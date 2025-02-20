# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->
## [Unreleased] - ReleaseDate
## [0.10.0] - 2020-12-14
### Added
- [PR#131](https://github.com/EmbarkStudios/cargo-fetcher/pull/131) and [PR#151](https://github.com/EmbarkStudios/cargo-fetcher/pull/150) added support for registries other than crates.io, resolving [#118](https://github.com/EmbarkStudios/cargo-fetcher/issues/118). Thanks [@m0ssc0de](https://github.com/m0ssc0de)!
- [PR#152](https://github.com/EmbarkStudios/cargo-fetcher/pull/152) added support for creating `.cache` entries when mirroring/syncing registry indices, resolving [#16](https://github.com/EmbarkStudios/cargo-fetcher/issues/16) and [#117](https://github.com/EmbarkStudios/cargo-fetcher/issues/117).
- [PR#154](https://github.com/EmbarkStudios/cargo-fetcher/pull/154) added support for mirroring and syncing git submodules, which was the final missing piece for having "perfect" copying of cargo's behavior when fetching crates and registries, resolving [#141](https://github.com/EmbarkStudios/cargo-fetcher/issues/141).

## [0.9.0] - 2020-07-28
### Added
- [PR#109](https://github.com/EmbarkStudios/cargo-fetcher/pull/109) added support for Azure Blob storage, under the `blob` feature flag. Thanks [@m0ssc0de](https://github.com/m0ssc0de)!

## [0.8.0] - 2020-06-05
### Added
- [PR#92](https://github.com/EmbarkStudios/cargo-fetcher/pull/92) added support for a local filesystem backend. Thanks [@cosmicexplorer](https://github.com/cosmicexplorer)!

## [0.7.0] - 2020-02-21
### Added
- Cargo's v2 Cargo.lock format is now supported, in addition to the v1 format.

### Changed
- Async (almost) all the things!
- Replaced log/env_logger with [tracing](https://github.com/tokio-rs/tracing)

## [0.6.1] - 2019-11-14
### Fixed
- Fetch registry index instead of pull

## [0.6.0] - 2019-11-14
### Added
- Added support for S3 storage behind the `s3` feature
- Integration tests using s3 via minio are now run in CI
- Git dependencies are now checked out to the git/checkouts folder
- Git dependencies now also recursively download submodules

### Changed
- Updated dependencies
- Place all GCS specific code/dependencies behind a `gcs` feature
- The url for the storage location is now supplied via `-u | --url`

### Fixed
- Replaced `failure` with `anyhow`
- Fixed issue where **all** crates were synced every time due to pruning and 
removing duplicates only to then completely ignore them and use the original crate list :facepalm:
- Fixed issue where crates.io packages were being unpacked with an extra parent directory

## [0.5.1] - 2019-10-27
### Fixed
- Allow using as `cargo fetcher` instead of only `cargo-fetcher`

## [0.5.0] - 2019-10-26
### Added
- Validate crate checksums after download

### Fixed
- Ensure duplicates are only downloaded once eg. same git source for multiple crates

## [0.4.1] - 2019-10-25
### Added
- Add support for only updating the registry index after it hasn't been updated
for a user specified amount of time, rather than always

## [0.4.0] - 2019-10-25
### Added
- Add support for retrieving and uploading the crates.io index

## [0.3.0] - 2019-10-25
### Added
- Add support for unpacking compressed crate tarballs into registry/src

## [0.2.0] - 2019-07-24
### Added
- Add crate retrieval and uploading for `git` sources

## [0.1.1] - 2019-07-23
### Fixed
- Travis config

## [0.1.0] - 2019-07-23
### Added
- Initial add of `cargo-fetcher`

<!-- next-url -->
[Unreleased]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.10.0...HEAD
[0.10.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.9.0...0.10.0
[0.9.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.8.0...0.9.0
[0.8.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.7.0...0.8.0
[0.7.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.6.1...0.7.0
[0.6.1]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.6.0...0.6.1
[0.6.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.5.1...0.6.0
[0.5.1]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.5.0...0.5.1
[0.5.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.4.1...0.5.0
[0.4.1]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.1.1...0.2.0
[0.1.1]: https://github.com/EmbarkStudios/cargo-fetcher/compare/0.1.0...0.1.1
[0.1.0]: https://github.com/EmbarkStudios/cargo-fetcher/releases/tag/0.1.0
