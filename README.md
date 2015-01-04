Cargo downloads your Rust project’s dependencies and compiles your project.

Learn more at http://doc.crates.io/.

## Installing cargo from nightlies

Cargo has nightlies available for use. The cargo source is not always guaranteed
to compile on rust master as it may lag behind by a day or two. Nightlies,
however, will run regardless of this fact!

```sh
triple=x86_64-unknown-linux-gnu
curl -O https://static.rust-lang.org/cargo-dist/cargo-nightly-$triple.tar.gz
tar xf cargo-nightly-$triple.tar.gz
./cargo-nightly-$triple/install.sh
```

Nightlies are available for the following triples:

* [`x86_64-unknown-linux-gnu`](https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-unknown-linux-gnu.tar.gz)
* [`i686-unknown-linux-gnu`](https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-unknown-linux-gnu.tar.gz)
* [`x86_64-apple-darwin`](https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-apple-darwin.tar.gz)
* [`i686-apple-darwin`](https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-apple-darwin.tar.gz)
* [`x86_64-pc-windows-gnu`](https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-pc-windows-gnu.tar.gz)
* [`i686-pc-windows-gnu`](https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-pc-windows-gnu.tar.gz)

Note that if you're using the windows snapshot you will need Mingw-w64 installed
as well as MSYS. The installation script needs to be run inside the MSYS shell.

## Compiling cargo

Cargo requires the following tools and packages to build:

* `rustc`
* `python`
* `curl`
* `cmake`
* `pkg-config`
* OpenSSL headers (`libssl-dev` package on ubuntu)

Cargo can then be compiled like many other standard unix-like projects:

```sh
git clone https://github.com/rust-lang/cargo
cd cargo
git submodule update --init
./.travis.install.deps.sh
./configure --local-rust-root="$PWD"/rustc
make
make install
```

More options can be discovered through `./configure`, such as compiling cargo
for more than one target. For example, if you'd like to compile both 32 and 64
bit versions of cargo on unix you would use:

```
$ ./configure --target=i686-unknown-linux-gnu,x86_64-unknown-linux-gnu
```

## Contributing to the Docs

To contribute to the docs, all you need to do is change the markdown files in
the `src/doc` directory.

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

