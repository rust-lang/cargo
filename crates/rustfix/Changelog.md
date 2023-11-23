# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.6] - 2019-07-16

### Changed

Internal changes:

- Change example to automatically determine filename
- Migrate to Rust 2018
- use `derive` feature over `serde_derive` crate

## [0.4.5] - 2019-03-26

### Added

- Implement common traits for Diagnostic and related types

### Fixed

- Fix out of bounds access in parse_snippet

## [0.4.4] - 2018-12-13

### Added

- Make Diagnostic::rendered public.

### Changed

- Revert faulty "Allow multiple solutions in a suggestion"

## [0.4.3] - 2018-12-09 - *yanked!*

### Added

- Allow multiple solutions in a suggestion

### Changed

- use `RUSTC` environment var if present

## [0.4.2] - 2018-07-31

### Added

- Expose an interface to apply fixes on-by-one

### Changed

- Handle invalid snippets instead of panicking

## [0.4.1] - 2018-07-26

### Changed

- Ignore duplicate replacements

## [0.4.0] - 2018-05-23

### Changed

- Filter by machine applicability by default

[Unreleased]: https://github.com/rust-lang-nursery/rustfix/compare/rustfix-0.4.6...HEAD
[0.4.6]: https://github.com/rust-lang-nursery/rustfix/compare/rustfix-0.4.5...rustfix-0.4.6
[0.4.5]: https://github.com/rust-lang-nursery/rustfix/compare/rustfix-0.4.4...rustfix-0.4.5
[0.4.4]: https://github.com/rust-lang-nursery/rustfix/compare/rustfix-0.4.3...rustfix-0.4.4
[0.4.3]: https://github.com/rust-lang-nursery/rustfix/compare/rustfix-0.4.2...rustfix-0.4.3
[0.4.2]: https://github.com/rust-lang-nursery/rustfix/compare/rustfix-0.4.1...rustfix-0.4.2
[0.4.1]: https://github.com/rust-lang-nursery/rustfix/compare/rustfix-0.4.0...rustfix-0.4.1
[0.4.0]: https://github.com/rust-lang-nursery/rustfix/compare/rustfix-0.4.0
