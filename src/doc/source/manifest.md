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
your Rust code, for example. More information can be found in the building
non-rust code [guide][2]

[1]: http://doc.rust-lang.org/rust.html#external-blocks
[2]: native-build.html

```toml
[package]
# ...
build = "make"
```

```toml
[package]
# ...

# Specify two commands to be run sequentially
build = ["./configure", "make"]
```

## The `exclude` Field (optional)

You can explicitly specify to Cargo that a set of globs should be ignored for
the purposes of packaging and rebuilding a package. The globs specified in this
field identify a set of files that are not included when a package is published
as well as ignored for the purposes of detecting when to rebuild a package.

If a VCS is being used for a package, the `exclude` field will be seeded with
the VCS's ignore settings (`.gitignore` for git for example).

```toml
[package]
# ...
exclude = ["build/**/*.o", "doc/**/*.html"]
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

# The `[profile.*]` Sections

Cargo supports custom configuration of how rustc is invoked through **profiles**
at the top level. Any manifest may declare a profile, but only the **top level**
project's profiles are actually read. All dependencies' profiles will be
overridden. This is done so the top-level project has control over how its
dependencies are compiled.

There are five currently supported profile names, all of which have the same
configuration available to them. Listed below is the configuration available,
along with the defaults for each profile.

```toml
# The development profile, used for `cargo build`
[profile.dev]
opt-level = 0  # Controls the --opt-level the compiler builds with
debug = true   # Controls whether the compiler passes -g or `--cfg ndebug`

# The release profile, used for `cargo build --release`
[profile.release]
opt-level = 3
debug = false

# The testing profile, used for `cargo test`
[profile.test]
opt-level = 0
debug = true

# The benchmarking profile, used for `cargo bench`
[profile.bench]
opt-level = 3
debug = false

# The documentation profile, used for `cargo doc`
[profile.doc]
opt-level = 0
debug = true
```

# The `[dev-dependencies.*]` Sections

The format of this section is equivalent to `[dependencies.*]`. Dev-dependencies
are not used when compiling a package for building, but are used for compiling
tests and benchmarks.

These dependencies are *not* propagated to other packages which depend on this
package.

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
▾ benches/      # (optional) benchmarks
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

# Configuring a target

Both `[[bin]]` and `[lib]` sections support similar configuration for specifying
how a target should be built. The example below uses `[lib]`, but it also
applies to all `[[bin]]` sections as well. All values listed ar the defaults for
that option unless otherwise specified.

```toml
[package]
# ...

[lib]

# The name of a target is the name of the library that will be generated. This
# is defaulted to the name of the package or project.
name = "foo"

# This field points at where the crate is located, relative to the Cargo.toml.
path = "src/lib.rs"

# A flag for enabling unit tests for this target. This is used by `cargo test`.
test = true

# A flag for enabling documentation tests for this target. This is only
# relevant for libraries, it has no effect on [[bin]] sections. This is used by
# `cargo test`.
doctest = true

# A flag for enabling benchmarks for this target. This is used by `cargo bench`.
bench = true

# A flag for enabling documentation of this target. This is used by `cargo doc`.
doc = true

# If the target is meant to be a compiler plugin, this field must be set to true
# for cargo to correctly compile it and make it available for all dependencies.
plugin = false
```

# Building Dynamic or Static Libraries

If your project produces a library, you can specify which kind of
library to build by explicitly listing the library in your `Cargo.toml`:

```toml
# ...

[lib]

name = "..."
# this could be "staticlib" as well
crate-type = ["dylib"]
```

The available options are `dylib`, `rlib`, and `staticlib`. You should only use
this option in a project. Cargo will always compile **packages** (dependencies)
based on the requirements of the project that includes them.
