use crate::{util, Krate, Registry, Source};
use anyhow::{Context, Error};
use futures::StreamExt;
use std::{io::Write, path::PathBuf};
use tracing::{debug, error, info, warn};
use tracing_futures::Instrument;

pub const INDEX_DIR: &str = "registry/index";
pub const CACHE_DIR: &str = "registry/cache";
pub const SRC_DIR: &str = "registry/src";
pub const GIT_DB_DIR: &str = "git/db";
pub const GIT_CO_DIR: &str = "git/checkouts";

pub async fn registry_indices(
    root_dir: PathBuf,
    backend: crate::Storage,
    registries: Vec<std::sync::Arc<Registry>>,
) -> Result<(), Error> {
    let root_dir = &root_dir;
    let resu = futures::stream::iter(registries)
        .map(|registry| {
            let backend = backend.clone();
            async move {
                let res: Result<(), Error> = registry_index(root_dir, backend, registry)
                    .instrument(tracing::debug_span!("download registry"))
                    .await;
                res
            }
            .instrument(tracing::debug_span!("sync registry"))
        })
        .buffer_unordered(32);

    resu.fold((), |u, res| async move {
        match res {
            Ok(a) => a,
            Err(e) => {
                error!("{:#}", e);
                u
            }
        }
    })
    .await;

    Ok(())
}

pub async fn registry_index(
    root_dir: &Path,
    backend: crate::Storage,
    registry: std::sync::Arc<Registry>,
) -> Result<(), Error> {
    let ident = registry.short_name();

    let index_path = root_dir.join(INDEX_DIR).join(ident.clone());
    std::fs::create_dir_all(&index_path).context("failed to create index dir")?;

    // Just skip the index if the git directory already exists,
    // as a patch on top of an existing repo via git fetch is
    // presumably faster
    if let Ok(repo) = git2::Repository::open(&index_path) {
        info!("registry index already exists, fetching instead");

        let url = registry.index.as_str().to_owned();

        // We need to ship off the fetching to a blocking thread so we don't anger tokio
        match tokio::task::spawn_blocking(move || -> Result<(), Error> {
            let git_config =
                git2::Config::open_default().context("Failed to open default git config")?;

            crate::git::with_fetch_options(&git_config, &url, &mut |mut opts| {
                repo.remote_anonymous(&url)?
                    .fetch(
                        &[
                            "refs/heads/master:refs/remotes/origin/master",
                            "HEAD:refs/remotes/origin/HEAD",
                        ],
                        Some(&mut opts),
                        None,
                    )
                    .context("Failed to fetch")
            })
        })
        .instrument(tracing::debug_span!("fetch"))
        .await?
        {
            Ok(_) => {
                // Write a file to the directory to let cargo know when it was updated
                std::fs::File::create(index_path.join(".last-updated"))
                    .context("failed to crate .last-updated")?;
                return Ok(());
            }
            Err(err_out) => {
                error!(
                    "failed to pull registry index, removing it and updating manually: {}",
                    err_out
                );
                remove_dir_all::remove_dir_all(&index_path)?;
            }
        }
    }

    let krate = Krate {
        name: ident.clone(),
        version: "1.0.0".to_owned(),
        source: Source::Git {
            url: registry.index.clone(),
            ident,
            rev: "feedc0de".to_owned(),
        },
    };

    let index_data = backend.fetch(&krate).await?;

    if let Err(e) = tokio::task::spawn_blocking(move || {
        util::unpack_tar(index_data, util::Encoding::Zstd, &index_path)
    })
    .instrument(tracing::debug_span!("unpack_tar"))
    .await
    {
        error!(err = ?e, "failed to unpack crates.io-index");
    }

    Ok(())
}

async fn sync_git(
    db_dir: PathBuf,
    co_dir: PathBuf,
    krate: &Krate,
    src: crate::git::GitSource,
    rev: &str,
) -> Result<(), Error> {
    let db_path = db_dir.join(format!("{}", krate.local_id()));

    // Always just blow away and do a sync from the remote tar
    if db_path.exists() {
        remove_dir_all::remove_dir_all(&db_path).context("failed to remove existing DB path")?;
    }

    let crate::git::GitSource { db, checkout } = src;

    let unpack_path = db_path.clone();
    tokio::task::spawn_blocking(move || util::unpack_tar(db, util::Encoding::Zstd, &unpack_path))
        .instrument(tracing::debug_span!("unpack_db_tar"))
        .await??;

    let co_path = co_dir.join(format!("{}/{}", krate.local_id(), rev));

    // If we get here, it means there wasn't a .cargo-ok in the dir, even if the
    // rest of it is checked out and ready, so blow it away just in case as we are
    // doing a clone/checkout from a local bare repository rather than a remote one
    if co_path.exists() {
        debug!("removing checkout dir {} for {}", co_path.display(), krate);
        remove_dir_all::remove_dir_all(&co_path)
            .with_context(|| format!("unable to remove {}", co_path.display()))?;
    }

    // If we have a checkout tarball, use that, as it will include submodules,
    // otherwise do a checkout
    match checkout {
        Some(checkout) => {
            let unpack_path = co_path.clone();
            tokio::task::spawn_blocking(move || {
                util::unpack_tar(checkout, util::Encoding::Zstd, &unpack_path)
            })
            .instrument(tracing::debug_span!("unpack_checkout_tar"))
            .await??;
        }
        None => {
            // Do a checkout of the bare clone
            crate::git::checkout(db_path, co_path.clone(), rev.to_owned())
                .instrument(tracing::debug_span!("checkout"))
                .await?;
        }
    }

    let ok = co_path.join(".cargo-ok");
    // The non-git .cargo-ok has "ok" in it, however the git ones do not
    std::fs::File::create(&ok).with_context(|| ok.display().to_string())?;

    Ok(())
}

use std::path::Path;

async fn sync_package(
    cache_dir: &Path,
    src_dir: &Path,
    krate: &Krate,
    data: bytes::Bytes,
    chksum: &str,
) -> Result<(), Error> {
    util::validate_checksum(&data, chksum)?;

    let packed_krate_path = cache_dir.join(format!("{}", krate.local_id()));

    let pack_data = data.clone();
    let packed_path = packed_krate_path.clone();

    // Spawn a worker thread to write the original pack file to disk as we don't
    // particularly care when it is done
    let pack_write = tokio::task::spawn_blocking(move || {
        let s = tracing::debug_span!("pack_write");
        let _ = s.enter();
        match std::fs::File::create(&packed_path) {
            Ok(mut f) => {
                let _ = f.set_len(pack_data.len() as u64);
                f.write_all(&pack_data)?;
                f.sync_all()?;

                debug!(bytes = pack_data.len(), path = ?packed_path, "wrote pack file to disk");

                Ok(())
            }
            Err(e) => Err(e),
        }
    });

    let mut src_path = src_dir.join(format!("{}", krate.local_id()));

    let unpack = tokio::task::spawn_blocking(move || {
        // Remove the .crate extension
        src_path.set_extension("");
        let ok = src_path.join(".cargo-ok");

        if !ok.exists() {
            if src_path.exists() {
                debug!("cleaning src/");
                if let Err(e) = remove_dir_all::remove_dir_all(&src_path) {
                    error!(err = ?e, "failed to remove src/");
                    return Err(e.into());
                }
            }

            // Crate tarballs already include the top level directory internally,
            // so unpack in the top-level source directory
            if let Err(e) = util::unpack_tar(data, util::Encoding::Gzip, src_path.parent().unwrap())
            {
                error!(err = ?e, "failed to unpack to src/");
                return Err(e);
            }

            // Create the .cargo-ok file so that cargo doesn't suspect a thing
            if let Err(e) = util::write_ok(&ok) {
                // If this happens, cargo will just resync and recheckout the repo most likely
                warn!(err = ?e, "failed to write .cargo-ok");
            }
        }

        Ok(())
    });

    let unpack = tokio::task::spawn(unpack);

    let (pack_write, unpack) = tokio::join!(pack_write, unpack);

    if let Err(err) = pack_write {
        error!(?err, path = ?packed_krate_path, "failed to write tarball to disk")
    }

    if let Err(err) = unpack {
        error!(?err, "failed to unpack tarball to disk")
    }

    Ok(())
}

fn get_missing_git_sources<'krate>(
    ctx: &'krate crate::Ctx,
    git_co_dir: &Path,
    to_sync: &mut Vec<&'krate Krate>,
) {
    for (rev, ident, krate) in ctx.krates.iter().filter_map(|k| match &k.source {
        Source::Git { rev, ident, .. } => Some((rev, ident, k)),
        _ => None,
    }) {
        let path = git_co_dir.join(format!("{}/{}/.cargo-ok", ident, rev));

        if !path.exists() {
            to_sync.push(krate);
        }
    }
}

fn get_missing_registry_sources<'krate>(
    ctx: &'krate crate::Ctx,
    registry: &Registry,
    cache_dir: &Path,
    to_sync: &mut Vec<&'krate Krate>,
) -> Result<(), Error> {
    let cache_iter = std::fs::read_dir(&cache_dir)?;

    let mut cached_crates: Vec<String> = cache_iter
        .filter_map(|entry| {
            entry
                .ok()
                .and_then(|entry| entry.file_name().to_str().map(|s| s.to_owned()))
        })
        .collect();

    cached_crates.sort();

    let mut krate_name = String::with_capacity(128);

    for krate in ctx.krates.iter().filter(|k| *k == registry) {
        use std::fmt::Write;
        write!(&mut krate_name, "{}", krate.local_id()).unwrap();

        if cached_crates.binary_search(&krate_name).is_err() {
            to_sync.push(krate);
        }

        krate_name.clear();
    }

    Ok(())
}

#[derive(Debug)]
pub struct Summary {
    pub total_bytes: usize,
    pub bad: u32,
    pub good: u32,
}

pub async fn crates(ctx: &crate::Ctx) -> Result<Summary, Error> {
    info!("synchronizing {} crates...", ctx.krates.len());

    let root_dir = &ctx.root_dir;
    let git_db_dir = root_dir.join(GIT_DB_DIR);
    let git_co_dir = root_dir.join(GIT_CO_DIR);

    std::fs::create_dir_all(&git_db_dir).context("failed to create git/db/")?;
    std::fs::create_dir_all(&git_co_dir).context("failed to create git/checkouts/")?;

    info!("checking local cache for missing crates...");
    let mut to_sync = Vec::with_capacity(ctx.krates.len());
    get_missing_git_sources(ctx, &git_co_dir, &mut to_sync);

    for registry in &ctx.registries {
        let (cache_dir, src_dir) = registry.sync_dirs(root_dir);
        std::fs::create_dir_all(&cache_dir).context("failed to create registry/cache")?;
        std::fs::create_dir_all(&src_dir).context("failed to create registry/src")?;

        get_missing_registry_sources(ctx, &registry, &cache_dir, &mut to_sync)?;
    }

    // Remove duplicates, eg. when 2 crates are sourced from the same git repository
    to_sync.sort();
    to_sync.dedup();

    if to_sync.is_empty() {
        info!("all crates already available on local disk");
        return Ok(Summary {
            total_bytes: 0,
            good: 0,
            bad: 0,
        });
    }

    info!("synchronizing {} missing crates...", to_sync.len());

    let bodies = futures::stream::iter(to_sync)
        .map(|krate| {
            let backend = ctx.backend.clone();

            let git_db_dir = git_db_dir.clone();
            let git_co_dir = git_co_dir.clone();

            #[allow(clippy::cognitive_complexity)]
            async move {
                match backend
                    .fetch(krate)
                    .instrument(tracing::debug_span!("download"))
                    .await
                {
                    Err(e) => {
                        error!(err = ?e, "failed to download");
                        Err(e)
                    }
                    Ok(krate_data) => {
                        let len = krate_data.len();
                        match &krate.source {
                            Source::Registry { registry, chksum } => {
                                let (cache_dir, src_dir) = registry.sync_dirs(&root_dir);
                                if let Err(e) =
                                    sync_package(&cache_dir, &src_dir, krate, krate_data, chksum)
                                        .instrument(tracing::debug_span!("package"))
                                        .await
                                {
                                    error!(err = ?e, "failed to splat package");
                                    return Err(e);
                                }
                            }
                            Source::Git { rev, .. } => {
                                let checkout = {
                                    let mut checkout_id = krate.clone();

                                    if let Source::Git { rev, .. } = &mut checkout_id.source {
                                        rev.push_str("-checkout");
                                    }

                                    backend
                                        .fetch(&checkout_id)
                                        .instrument(tracing::debug_span!("download_checkout"))
                                        .await
                                        .ok()
                                };

                                let git_source = crate::git::GitSource {
                                    db: krate_data,
                                    checkout,
                                };

                                if let Err(e) =
                                    sync_git(git_db_dir, git_co_dir, krate, git_source, rev)
                                        .instrument(tracing::debug_span!("git"))
                                        .await
                                {
                                    error!(err = ?e, "failed to splat git repo");
                                    return Err(e);
                                }
                            }
                        };

                        Ok(len)
                    }
                }
            }
            .instrument(tracing::debug_span!("sync", %krate))
        })
        .buffer_unordered(32);

    let summary = bodies
        .fold(
            Summary {
                total_bytes: 0,
                bad: 0,
                good: 0,
            },
            |mut acc, res| async move {
                match res {
                    Ok(len) => {
                        acc.good += 1;
                        acc.total_bytes += len;
                    }
                    Err(_) => {
                        acc.bad += 1;
                    }
                }

                acc
            },
        )
        .await;

    Ok(summary)
}
