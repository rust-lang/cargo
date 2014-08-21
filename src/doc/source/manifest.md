---
title: The Manifest Format
---

# The `[package]` Section

The first section in a `Cargo.toml` is `[package]`.

```toml
[package]
name = "hello-world" # the name of the package
version = "1.0.0"    # the current version, obeying semver
authors = [ "wycats@example.com" ]
```

All three of these fields are mandatory. Cargo bakes in the concept of
[Semantic Versioning](http://semver.org/), so make sure you follow some
basic rules:

* Before you reach 1.0.0, anything goes.
* After 1.0.0, only make breaking changes when you increment the major
  version. In Rust, breaking changes include adding fields to structs or
  variants to enums. Don't break the build.
* After 1.0.0, don't add any new public API (no new `pub` anything) in
  tiny versions. Always increment the minor version if you add any new
  `pub` structs, traits, fields, types, functions, methods or anything else.
* Use version numbers with three numeric parts such as 1.0.0 rather than 1.0.

## The `build` Field (optional)

You can specify a script that Cargo should execute before invoking
`rustc`. You can use this to compile C code that you will [link][1] into
your Rust code, for example.

[1]: http://doc.rust-lang.org/rust.html#external-blocks

```toml
[package]
# ...
build = "make"
```

# The `[dependencies.*]` Sections

You list dependencies using `[dependencies.<name>]`. For example, if you
wanted to depend on both `hammer` and `color`:

```toml
[package]
# ...

[dependencies.hammer]
version = "0.5.0" # optional
git = "https://github.com/wycats/hammer.rs"

[dependencies.color]
git = "https://github.com/bjz/color-rs"
```

You can specify the source of a dependency in one of two ways at the moment:

* `git = "<git-url>"`: A git repository with a `Cargo.toml` in its root. The
  `rev`, `tag`, and `branch` options are also recognized to use something other
  than the `master` branch.
* `path = "<relative-path>"`: A path relative to the current `Cargo.toml`
  with a `Cargo.toml` in its root.

Soon, you will be able to load packages from the Cargo registry as well.

# The Project Layout

If your project is an executable, name the main source file `src/main.rs`.
If it is a library, name the main source file `src/lib.rs`.

Cargo will also treat any files located in `src/bin/*.rs` as
executables.

When you run `cargo build`, Cargo will compile all of these files into
the `target` directory.

```
▾ src/          # directory containing source files
  ▾ bin/        # (optional) directory containing executables
    *.rs
  lib.rs        # the main entry point for libraries and packages
  main.rs       # the main entry point for projects producing executables
▾ examples/     # (optional) examples
  *.rs
▾ tests/        # (optional) integration tests
  *.rs
```

# Examples

Files located under `examples` are example uses of the functionality
provided by the library.

They must compile as executables (with `main.rs`) and load in the
library by using `extern crate <library-name>`. They are compiled when
you run your tests to protect them from bitrotting.

# Tests

When you run `cargo test`, Cargo will:

* Compile your library's unit tests, which are in files reachable from
  `lib.rs`. Any sections marked with `#[cfg(test)]` will be included.
* Compile your library's integration tests, which are located in
  `tests`. Files in `tests` load in your library by using `extern crate
  <library-name>` like any other code that depends on it.
* Compile your library's examples.

# Building Dynamic Libraries

If your project produces a library, you can specify which kind of
library to build by explicitly listing the library in your `Cargo.toml`:

```toml
# ...

[[lib]]

name = "..."
crate-types = [ "dylib" ]
```

The available options are `dylib` and `rlib`. You should only use
this option in a project. Cargo will always compile **packages**
(dependencies) based on the requirements of the project that includes
them.
