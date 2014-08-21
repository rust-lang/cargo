---
title: Building external code
---

Some packages need to compile third-party non-Rust code that you will
link into your Rust code using `#[link]` (more information on `#[link]`
can be found in [the Rust manual][1]).

Cargo does not aim to replace other tools that are well-optimized for
building C or C++ code, but it does integrate with them with the `build`
configuration option.

```toml
[package]

name = "hello-world-from-c"
version = "0.1.0"
authors = [ "wycats@gmail.com" ]
build = "make"
```

The `build` command will be invoked before `rustc`, allowing your Rust
code to depend on the built artifacts.

Here's what you need to know:

* Cargo passes your build script an environment variable named
  `OUT_DIR`, which is where you should put any compiled artifacts. It
  will be different for different Cargo commands, but Cargo will always
  pass that output directory as a lib directory to `rustc`.
* Cargo will retain all output in `OUT_DIR` for clean packages across
  builds (intelligently discarding the compiled artifacts for dirty
  dependencies). Do not put the output of a build command in any other
  directory.
* The actual location of `$OUT_DIR` is
  `/path/to/project/target/native/$your-out-dir`.

What this means is that the normal workflow for build dependencies is:

* The first time a user types `cargo build` for a project that contains
  your package, your `build` script will be invoked. Place any artifacts
  into the provided `$OUT_DIR`.
* The next time a user runs `cargo build`, if the dependency has not
  changed (via `cargo update <your-package>`), Cargo will reuse the
  output you provided before.
* If the user updates your package to a new version (or git revision),
  Cargo will wipe the old `$OUT_DIR` and re-invoke your build script.

In general, build scripts may not be as portable as we'd like today. We
encourage package authors to write build scripts that can work in both
Windows and Unix environments.

Several people who work on Cargo are also working on a project called
[link-config][2], which is a Rust syntax extension whose goal is to
enable portable external compilation and linkage against system
packages. We intend for it to eventually serve this purpose for Cargo
projects.

[1]: http://doc.rust-lang.org/rust.html#runtime-services,-linkage-and-debugging
[2]: https://github.com/alexcrichton/link-config
