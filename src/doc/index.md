% Cargo, Rust’s Package Manager

# Installing

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

# Let’s get started

To start a new project with Cargo, use `cargo new`:

```shell
$ cargo new hello_world --bin
```

We’re passing `--bin` because we’re making a binary program: if we
were making a library, we’d leave it off.

Let’s check out what Cargo has generated for us:

```shell
$ cd hello_world
$ tree .
.
├── Cargo.toml
└── src
    └── main.rs

1 directory, 2 files
```

This is all we need to get started. First, let’s check out `Cargo.toml`:

```toml
[package]
name = "hello_world"
version = "0.1.0"
authors = ["Your Name <you@example.com>"]
```

This is called a **manifest**, and it contains all of the metadata that Cargo
needs to compile your project.

Here’s what’s in `src/main.rs`:

```
fn main() {
    println!("Hello, world!");
}
```

Cargo generated a “hello world” for us. Let’s compile it:

```shell
$ cargo build
   Compiling hello_world v0.1.0 (file:///path/to/project/hello_world)
```

And then run it:

```shell
$ ./target/debug/hello_world
Hello, world!
```

We can also use `cargo run` to compile and then run it, all in one step:

```shell
$ cargo run
     Fresh hello_world v0.1.0 (file:///path/to/project/hello_world)
   Running `target/hello_world`
Hello, world!
```

# Going further

For more details on using Cargo, check out the [Cargo Guide](guide.html)
