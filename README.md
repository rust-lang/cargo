# Cargo

Cargo downloads your Rust project’s dependencies and compiles your project.

Learn more at https://doc.rust-lang.org/cargo/

## Code Status

[![CI](https://github.com/rust-lang/cargo/actions/workflows/main.yml/badge.svg?branch=auto-cargo)](https://github.com/rust-lang/cargo/actions/workflows/main.yml)

Code documentation: https://docs.rs/cargo/

## Installing Cargo

Cargo is distributed by default with Rust, so if you've got `rustc` installed
locally you probably also have `cargo` installed locally.

## Compiling from Source

Cargo requires the following tools and packages to build:

* `git`
* `curl` (on Unix)
* `pkg-config` (on Unix, used to figure out the `libssl` headers/libraries)
* OpenSSL headers (only for Unix, this is the `libssl-dev` package on ubuntu)
* `cargo` and `rustc`

First, you'll want to check out this repository

```
git clone https://github.com/rust-lang/cargo
cd cargo
```

With `cargo` already installed, you can simply run:

```
cargo build --release
```

## Adding new subcommands to Cargo

Cargo is designed to be extensible with new subcommands without having to modify
Cargo itself. See [the Wiki page][third-party-subcommands] for more details and
a list of known community-developed subcommands.

[third-party-subcommands]: https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands


## Releases

Cargo releases coincide with Rust releases.
High level release notes are available as part of [Rust's release notes][rel].
Detailed release notes are available in this repo at [CHANGELOG.md].

[rel]: https://github.com/rust-lang/rust/blob/master/RELEASES.md
[CHANGELOG.md]: CHANGELOG.md

## Reporting issues

Found a bug? We'd love to know about it!

Please report all issues on the GitHub [issue tracker][issues].

[issues]: https://github.com/rust-lang/cargo/issues

## Contributing

See the **[Cargo Contributor Guide]** for a complete introduction
to contributing to Cargo.

[Cargo Contributor Guide]: https://rust-lang.github.io/cargo/contrib/

## License

Cargo is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.

### Third party software

This product includes software developed by the OpenSSL Project
for use in the OpenSSL Toolkit (https://www.openssl.org/).

In binary form, this product includes software that is licensed under the
terms of the GNU General Public License, version 2, with a linking exception,
which can be obtained from the [upstream repository][1].

See [LICENSE-THIRD-PARTY](LICENSE-THIRD-PARTY) for details.

[1]: https://github.com/libgit2/libgit2

