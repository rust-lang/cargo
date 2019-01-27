## First Steps with Cargo

To start a new package with Cargo, use `cargo new`:

```console
$ cargo new hello_world
```

Cargo defaults to `--bin` to make a binary program. To make a library, we'd
pass `--lib`.

Let’s check out what Cargo has generated for us:

```console
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
edition = "2018"

[dependencies]
```

This is called a **manifest**, and it contains all of the metadata that Cargo
needs to compile your package.

Here’s what’s in `src/main.rs`:

```rust
fn main() {
    println!("Hello, world!");
}
```

Cargo generated a “hello world” for us. Let’s compile it:

```console
$ cargo build
   Compiling hello_world v0.1.0 (file:///path/to/package/hello_world)
```

And then run it:

```console
$ ./target/debug/hello_world
Hello, world!
```

We can also use `cargo run` to compile and then run it, all in one step:

```console
$ cargo run
     Fresh hello_world v0.1.0 (file:///path/to/package/hello_world)
   Running `target/hello_world`
Hello, world!
```

### Going further

For more details on using Cargo, check out the [Cargo Guide](guide/index.html)
