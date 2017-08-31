## Installation

### Install Stable Rust and Cargo

The easiest way to get Cargo is to get the current stable release of [Rust] by
using the `rustup` script:

```shell
$ curl -sSf https://static.rust-lang.org/rustup.sh | sh
```

After this, you can use the `rustup` command to also install `beta` or `nightly`
channels for Rust and Cargo.

### Install Nightly Cargo

To install just Cargo, the current recommended installation method is through
the official nightly builds. Note that Cargo will also require that [Rust] is
already installed on the system.

| Platform         | 64-bit            | 32-bit            |
|------------------|-------------------|-------------------|
| Linux binaries   | [tar.gz][linux64] | [tar.gz][linux32] |
| MacOS binaries   | [tar.gz][mac64]   | [tar.gz][mac32]   |
| Windows binaries | [tar.gz][win64]   | [tar.gz][win32]   |

### Build and Install Cargo from Source

Alternatively, you can [build Cargo from source][compiling-from-source].

[rust]: https://www.rust-lang.org/
[linux64]: https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-unknown-linux-gnu.tar.gz
[linux32]: https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-unknown-linux-gnu.tar.gz
[mac64]: https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-apple-darwin.tar.gz
[mac32]: https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-apple-darwin.tar.gz
[win64]: https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-pc-windows-gnu.tar.gz
[win32]: https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-pc-windows-gnu.tar.gz
[compiling-from-source]: https://github.com/rust-lang/cargo#compiling-from-source
