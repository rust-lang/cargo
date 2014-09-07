---
title: Cargo, Rust's Package Manager
---

# Installing

The easiest way to get Cargo is to get the Rust nightly build by using
the `rustup` script:

```shell
$ curl https://static.rust-lang.org/rustup.sh | sudo bash
```

This will get you the latest Rust nightly for your platform along with
the latest Cargo. You should run this script almost every day to get the latest updates.

If you are on Windows, you can directly download the latest [Rust](http://static.rust-lang.org/dist/rust-nightly-install.exe)
and [Cargo](http://static.rust-lang.org/cargo-dist/cargo-nightly-i686-pc-mingw32.tar.gz) nightlies.

Alternatively, you can build Cargo from source.

# Let's Get Started

To start a new project with Cargo, use `cargo new`:

```shell
$ cargo new hello_world --bin
```

We're passing `--bin` because we're making a binary program: if we
were making a library, we'd leave it off.

Let's check out what Cargo has generated for us:

```shell
$ cd hello_world
$ tree .
.
├── Cargo.toml
└── src
    └── main.rs

1 directory, 2 files
```

This is all we need to get started. First, let's check out `Cargo.toml`:

```toml
[package]

name = "hello_world"
version = "0.0.1"
authors = ["Your Name <you@example.com>"]
```

This is called a **manifest**, and it contains all of the metadata that Cargo
needs to compile your project.

Here's what's in `src/main.rs`:

```rs
fn main() {
    println!("Hello, world!")
}
```

Cargo generated a 'hello world' for us. Let's compile it:

<pre><code class="highlight"><span class="gp">$</span> cargo build
<span style="font-weight: bold"
class="s1">   Compiling</span> hello_world v0.0.1 (file:///path/to/project/hello_world)</code></pre>

And then run it:

```shell
$ ./target/hello_world
Hello, world!
```

We can also use `cargo run` to compile and then run it, all in one step:

<pre><code class="highlight"><span class="gp">$</span> cargo run
<span style="font-weight: bold"
class="s1">     Fresh</span> hello_world v0.0.1 (file:///path/to/project/hello_world)
<span style="font-weight: bold"
class="s1">   Running</span> `target/hello_world`
Hello, world!</code></pre>

# Going Further

For more details on using Cargo, check out the [Cargo Guide](/guide.html)
