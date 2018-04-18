# Cargo

Cargo downloads your Rust project’s dependencies and compiles your project.

Learn more at https://doc.rust-lang.org/cargo/

## Code Status

[![Build Status](https://travis-ci.org/rust-lang/cargo.svg?branch=master)](https://travis-ci.org/rust-lang/cargo)
[![Build Status](https://ci.appveyor.com/api/projects/status/github/rust-lang/cargo?branch=master&svg=true)](https://ci.appveyor.com/project/rust-lang-libs/cargo)

Code documentation: https://docs.rs/cargo/

## Installing Cargo

Cargo is distributed by default with Rust, so if you've got `rustc` installed
locally you probably also have `cargo` installed locally.

## Compiling from Source

Cargo requires the following tools and packages to build:

* `python`
* `curl` (on Unix)
* `cmake`
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

High level release notes are available as part of [Rust's release notes][rel].
Cargo releases coincide with Rust releases.

[rel]: https://github.com/rust-lang/rust/blob/master/RELEASES.md

## Reporting issues

Found a bug? We'd love to know about it!

Please report all issues on the GitHub [issue tracker][issues].

[issues]: https://github.com/rust-lang/cargo/issues

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). You may also find the architecture
documentation useful ([ARCHITECTURE.md](ARCHITECTURE.md)).

## License

Cargo is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See LICENSE-APACHE and LICENSE-MIT for details.

### Third party software

This product includes software developed by the OpenSSL Project
for use in the OpenSSL Toolkit (http://www.openssl.org/).

In binary form, this product includes software that is licensed under the
terms of the GNU General Public License, version 2, with a linking exception,
which can be obtained from the [upstream repository][1].

See LICENSE-THIRD-PARTY for details.

[1]: https://github.com/libgit2/libgit2

