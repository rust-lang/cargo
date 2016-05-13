Cargo downloads your Rust project’s dependencies and compiles your project.

Learn more at http://doc.crates.io/

## Code Status
[![Build Status](https://travis-ci.org/rust-lang/cargo.svg?branch=master)](https://travis-ci.org/rust-lang/cargo)
[![Build Status](https://ci.appveyor.com/api/projects/status/jnh54531mpidb2c2?svg=true)](https://ci.appveyor.com/project/alexcrichton/cargo)

## Installing Cargo

Cargo is distributed by default with Rust, so if you've got `rustc` installed
locally you probably also have `cargo` installed locally.

If, however, you would like to install Cargo from the nightly binaries that are
generated, you may also do so! Note that these nightlies are not official
binaries, so they are only provided in one format with one installation method.
Each tarball below contains a top-level `install.sh` script to install Cargo.

* [`x86_64-unknown-linux-gnu`](https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-unknown-linux-gnu.tar.gz)
* [`i686-unknown-linux-gnu`](https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-unknown-linux-gnu.tar.gz)
* [`x86_64-apple-darwin`](https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-apple-darwin.tar.gz)
* [`i686-apple-darwin`](https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-apple-darwin.tar.gz)
* [`x86_64-pc-windows-gnu`](https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-pc-windows-gnu.tar.gz)
* [`i686-pc-windows-gnu`](https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-pc-windows-gnu.tar.gz)
* [`x86_64-pc-windows-msvc`](https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-pc-windows-msvc.tar.gz)

Note that if you're on Windows you will have to run the `install.sh` script from
inside an MSYS shell, likely from a MinGW-64 installation.

## Compiling from Source

Cargo requires the following tools and packages to build:

* `python`
* `curl` (on Unix)
* `cmake`
* OpenSSL headers (only for Unix, this is the `libssl-dev` package on ubuntu)

First, you'll want to check out this repository

```
git clone --recursive https://github.com/rust-lang/cargo
cd cargo
```

If you already have `rustc` and `cargo` installed elsewhere, you can simply run

```
cargo build --release
```

Otherwise, if you have `rustc` installed and not Cargo, you can simply run:

```sh
./configure
make
make install
```

If, however, you have neither `rustc` nor `cargo` previously installed you can
run:

```sh
python -B src/etc/install-deps.py
./configure --local-rust-root="$PWD"/rustc
make
make install
```
Note: if building for 32 bit systems run `BITS=32 python -B ..`

More options can be discovered through `./configure`, such as compiling cargo
for more than one target. For example, if you'd like to compile both 32 and 64
bit versions of cargo on unix you would use:

```
$ ./configure --target=i686-unknown-linux-gnu,x86_64-unknown-linux-gnu
```

## Adding new subcommands to Cargo

Cargo is designed to be extensible with new subcommands without having to modify
Cargo itself. See [the Wiki page][third-party-subcommands] for more details and
a list of known community-developed subcommands.

[third-party-subcommands]: https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands

## Contributing to the Docs

To contribute to the docs, all you need to do is change the markdown files in
the `src/doc` directory. To view the rendered version of changes you have
made locally, run:

```sh
./configure
make doc
open target/doc/index.html
```

## Release notes

High level release notes are available as part of [Rust's release notes](https://github.com/rust-lang/rust/blob/master/RELEASES.md).

## Reporting Issues

Found a bug? We'd love to know about it!

Please report all issues on the github [issue tracker][issues].

[issues]: https://github.com/rust-lang/cargo/issues

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

[1]: https://github.com/libgit2/libgit2

