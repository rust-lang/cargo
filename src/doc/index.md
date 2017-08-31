% Cargo, Rust’s Package Manager

# Installing

The easiest way to get Cargo is to get the current stable release of Rust by
using the `rustup` script:

```shell
$ curl -sSf https://static.rust-lang.org/rustup.sh | sh
```

This will get you the current stable release of Rust for your platform along
with the latest Cargo.

If you are on Windows, you can directly download the latest 32bit ([Rust](https://static.rust-lang.org/dist/rust-1.0.0-i686-pc-windows-gnu.msi)
and [Cargo](https://static.rust-lang.org/cargo-dist/cargo-nightly-i686-pc-windows-gnu.tar.gz)) or 64bit ([Rust](https://static.rust-lang.org/dist/rust-1.0.0-x86_64-pc-windows-gnu.msi) and [Cargo](https://static.rust-lang.org/cargo-dist/cargo-nightly-x86_64-pc-windows-gnu.tar.gz)) Rust stable releases or Cargo nightlies.

Alternatively, you can build Cargo from source.

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
