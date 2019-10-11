# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- ## [Unreleased] -->

## [0.5.1] - 2019-10-12
### Changed
- Disable unnecessary features for `scopeguard`. Thanks @mati865.

## [0.5.0] - 2019-08-21
### Added
- Add `home_dir` implementation for Windows UWP platforms.

### Fixed
- Fix `rustup_home` implementation when `RUSTUP_HOME` is an absolute directory.
- Fix `cargo_home` implementation when `CARGO_HOME` is an absolute directory.

### Removed
- Remove support for `multirust` folder used in old version of `rustup`.

[Unreleased]: https://github.com/brson/home/compare/v0.5.0...HEAD
[0.5.1]: https://github.com/brson/home/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/brson/home/compare/0.4.2...v0.5.0
