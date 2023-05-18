# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.5.6
- Fixed & enhanced documentation.
  [#12047](https://github.com/rust-lang/cargo/pull/12047)

## 0.5.5 - 2023-04-25
- The `home` crate has migrated to the <https://github.com/rust-lang/cargo/> repository.
  [#11359](https://github.com/rust-lang/cargo/pull/11359)
- Replaced the winapi dependency with windows-sys.
  [#11656](https://github.com/rust-lang/cargo/pull/11656)

## [0.5.4] - 2022-10-10
- Add `_with_env` variants of functions to support in-process threaded tests for
  rustup.

## [0.5.3] - 2020-01-07

Use Rust 1.36.0 as minimum Rust version.

## [0.5.2] - 2020-01-05

*YANKED since it cannot be built on Rust 1.36.0*

### Changed
- Check for emptiness of `CARGO_HOME` and `RUSTUP_HOME` environment variables.
- Windows: Use `SHGetFolderPath` to replace `GetUserProfileDirectory` syscall.
  * Remove `scopeguard` dependency.

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

[0.5.4]: https://github.com/brson/home/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/brson/home/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/brson/home/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/brson/home/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/brson/home/compare/0.4.2...v0.5.0
