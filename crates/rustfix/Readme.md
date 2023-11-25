# rustfix

[![Latest Version](https://img.shields.io/crates/v/rustfix.svg)](https://crates.io/crates/rustfix)
[![Rust Documentation](https://docs.rs/rustfix/badge.svg)](https://docs.rs/rustfix)

Rustfix is a library defining useful structures that represent fix suggestions from rustc.

This is a low-level library. You pass it the JSON output from `rustc`, and you can then use it to apply suggestions to in-memory strings. This library doesn't execute commands, or read or write from the filesystem.

If you are looking for the [`cargo fix`] implementation, the core of it is located in [`cargo::ops::fix`].

[`cargo fix`]: https://doc.rust-lang.org/cargo/commands/cargo-fix.html
[`cargo::ops::fix`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/ops/fix.rs

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
