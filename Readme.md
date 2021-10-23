# rustfix

The goal of this tool is to read and apply the suggestions made by rustc.

## Current status

Currently, rustfix is split into two crates:

- `rustfix`, a library for consuming and applying suggestions in the format that `rustc` outputs
- and `cargo-fix`, a binary that works as cargo subcommand and that end users will use to fix their code.

The magic of rustfix is entirely dependent on the diagnostics implemented in the Rust compiler (and external lints, like [clippy]).

[clippy]: https://github.com/rust-lang-nursery/rust-clippy

## Installation

To use the rustfix library, add it to your `Cargo.toml`.

To get the tool to automatically fix warnings in, run `cargo install cargo-fix`. This will give you `cargo fix`.

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
