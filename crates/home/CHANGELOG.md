# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.5.11 - 2024-12-16

Note: 0.5.10 was not published.

- Updated package metadata.
  [#13184](https://github.com/rust-lang/cargo/pull/13184)
- Updated minimum Rust version to 1.81.
  [#13266](https://github.com/rust-lang/cargo/pull/13266)
  [#13324](https://github.com/rust-lang/cargo/pull/13324)
  [#14871](https://github.com/rust-lang/cargo/pull/14871)
- Updated windows-sys to 0.59.
  [#14335](https://github.com/rust-lang/cargo/pull/14335)
- Clarified support level of this crate (not intended for external use).
  [#14600](https://github.com/rust-lang/cargo/pull/14600)
- Docs cleanup.
  [#14823]()
- Add notice that this crate should not be used, and to use the standard library's `home_dir` instead.
  [#14939](https://github.com/rust-lang/cargo/pull/14939)

## 0.5.9 - 2023-12-15

- Replace SHGetFolderPathW with SHGetKnownFolderPath
  [#13173](https://github.com/rust-lang/cargo/pull/13173)
- Update windows-sys to 0.52
  [#13089](https://github.com/rust-lang/cargo/pull/13089)
- Set MSRV to 1.70.0
  [#12654](https://github.com/rust-lang/cargo/pull/12654)
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
