# rustfix

[![Latest Version](https://img.shields.io/crates/v/rustfix.svg)](https://crates.io/crates/rustfix)
[![Rust Documentation](https://docs.rs/rustfix/badge.svg)](https://docs.rs/rustfix)

Rustfix is a library defining useful structures that represent fix suggestions from rustc

## Current status

Currently, rustfix is split into two crates:

- `rustfix`, a library for consuming and applying suggestions in the format that `rustc` outputs (this crate)
- `cargo-fix`, a binary that works as cargo subcommand and that end users will use to fix their code (maintained in the [cargo](https://github.com/rust-lang/cargo/blob/master/src/cargo/ops/fix.rs) repo).


The library (and therefore this repo) is considered largely feature-complete. This is because:
* There is no compiler or even rust-specific logic here
* New lints and suggestions come from the Rust compiler (and external lints, like [clippy]).
* `rustfix` doesn't touch the filesystem to implement fixes, or read from disk

[clippy]: https://github.com/rust-lang-nursery/rust-clippy

## Installation

To get the tool to automatically fix warnings in, run `cargo install cargo-fix`. This will give you `cargo fix`.

To use the rustfix library for use in your own fix project, add it to your `Cargo.toml`.

## Using `cargo fix --edition` to transition to Rust 2021

Instructions on how to use this tool to transition a crate to Rust 2021 can be
found [in the Rust Edition Guide.](https://rust-lang-nursery.github.io/edition-guide/editions/transitioning-an-existing-project-to-a-new-edition.html)

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
