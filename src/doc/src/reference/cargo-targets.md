# Cargo Targets

Cargo packages consist of *targets* which correspond to source files which can
be compiled into a crate. Packages can have [library](#library),
[binary](#binaries), [example](#examples), [test](#tests), and
[benchmark](#benchmarks) targets. The list of targets can be configured in the
`Cargo.toml` manifest, often [inferred automatically](#target-auto-discovery)
by the [directory layout][package layout] of the source files.

See [Configuring a target](#configuring-a-target) below for details on
configuring the settings for a target.

## Library

The library target defines a "library" that can be used and linked by other
libraries and executables. The filename defaults to `src/lib.rs`, and the name
of the library defaults to the name of the package. A package can have only
one library. The settings for the library can be [customized] in the `[lib]`
table in `Cargo.toml`.

```toml
# Example of customizing the library in Cargo.toml.
[lib]
crate-type = ["cdylib"]
bench = false
```

## Binaries

Binary targets are executable programs that can be run after being compiled.
The default binary filename is `src/main.rs`, which defaults to the name of
the package. Additional binaries are stored in the [`src/bin/`
directory][package layout]. The settings for each binary can be [customized]
in the `[[bin]]` tables in `Cargo.toml`.

Binaries can use the public API of the package's library. They are also linked
with the [`[dependencies]`][dependencies] defined in `Cargo.toml`.

You can run individual binaries with the [`cargo run`] command with the `--bin
<bin-name>` option. [`cargo install`] can be used to copy the executable to a
common location.

```toml
# Example of customizing binaries in Cargo.toml.
[[bin]]
name = "cool-tool"
test = false
bench = false

[[bin]]
name = "frobnicator"
required-features = ["frobnicate"]
```

## Examples

Files located under the [`examples` directory][package layout] are example
uses of the functionality provided by the library. When compiled, they are
placed in the [`target/debug/examples` directory][build cache].

Examples can use the public API of the package's library. They are also linked
with the [`[dependencies]`][dependencies] and
[`[dev-dependencies]`][dev-dependencies] defined in `Cargo.toml`.

By default, examples are executable binaries (with a `main()` function). You
can specify the [`crate-type` field](#the-crate-type-field) to make an example
be compiled as a library:

```toml
[[example]]
name = "foo"
crate-type = ["staticlib"]
```

You can run individual executable examples with the [`cargo run`] command with
the `--example <example-name>` option. Library examples can be built with
[`cargo build`] with the `--example <example-name>` option. [`cargo install`]
with the `--example <example-name>` option can be used to copy executable
binaries to a common location. Examples are compiled by [`cargo test`] by
default to protect them from bit-rotting. Set [the `test`
field](#the-test-field) to `true` if you have `#[test]` functions in the
example that you want to run with [`cargo test`].

## Tests

There are two styles of tests within a Cargo project:

* *Unit tests* which are functions marked with the [`#[test]`
  attribute][test-attribute] located within your library or binaries (or any
  target enabled with [the `test` field](#the-test-field)). These tests have
  access to private APIs located within the target they are defined in.
* *Integration tests* which is a separate executable binary, also containing
  `#[test]` functions, which is linked with the project's library and has
  access to its *public* API.

Tests are run with the [`cargo test`] command. By default, Cargo and `rustc`
use the [libtest harness] which is responsible for collecting functions
annotated with the [`#[test]` attribute][test-attribute] and executing them in
parallel, reporting the success and failure of each test. See [the `harness`
field](#the-harness-field) if you want to use a different harness or test
strategy.

> **Note**: There is another special style of test in Cargo:
> [documentation tests][documentation examples].
> They are handled by `rustdoc` and have a slightly different execution model.
> For more information, please see [`cargo test`][cargo-test-documentation-tests].

[libtest harness]: ../../rustc/tests/index.html
[cargo-test-documentation-tests]: ../commands/cargo-test.md#documentation-tests

### Integration tests

Files located under the [`tests` directory][package layout] are integration
tests. When you run [`cargo test`], Cargo will compile each of these files as
a separate crate, and execute them.

Integration tests can use the public API of the package's library. They are
also linked with the [`[dependencies]`][dependencies] and
[`[dev-dependencies]`][dev-dependencies] defined in `Cargo.toml`.

If you want to share code among multiple integration tests, you can place it
in a separate module such as `tests/common/mod.rs` and then put `mod common;`
in each test to import it.

Each integration test results in a separate executable binary, and [`cargo
test`] will run them serially. In some cases this can be inefficient, as it
can take longer to compile, and may not make full use of multiple CPUs when
running the tests. If you have a lot of integration tests, you may want to
consider creating a single integration test, and split the tests into multiple
modules. The libtest harness will automatically find all of the `#[test]`
annotated functions and run them in parallel. You can pass module names to
[`cargo test`] to only run the tests within that module.

Binary targets are automatically built if there is an integration test. This
allows an integration test to execute the binary to exercise and test its
behavior. The `CARGO_BIN_EXE_<name>` [environment variable] is set when the
integration test is built so that it can use the [`env` macro] to locate the
executable.

[environment variable]: environment-variables.md#environment-variables-cargo-sets-for-crates
[`env` macro]: ../../std/macro.env.html

## Benchmarks

Benchmarks provide a way to test the performance of your code using the
[`cargo bench`] command. They follow the same structure as [tests](#tests),
with each benchmark function annotated with the `#[bench]` attribute.
Similarly to tests:

* Benchmarks are placed in the [`benches` directory][package layout].
* Benchmark functions defined in libraries and binaries have access to the
  *private* API within the target they are defined in. Benchmarks in the
  `benches` directory may use the *public* API.
* [The `bench` field](#the-bench-field) can be used to define which targets
  are benchmarked by default.
* [The `harness` field](#the-harness-field) can be used to disable the
  built-in harness.

> **Note**: The [`#[bench]`
> attribute](../../unstable-book/library-features/test.html) is currently
> unstable and only available on the [nightly channel]. There are some
> packages available on [crates.io](https://crates.io/keywords/benchmark) that
> may help with running benchmarks on the stable channel, such as
> [Criterion](https://crates.io/crates/criterion).

## Configuring a target

All of the  `[lib]`, `[[bin]]`, `[[example]]`, `[[test]]`, and `[[bench]]`
sections in `Cargo.toml` support similar configuration for specifying how a
target should be built. The double-bracket sections like `[[bin]]` are
[array-of-table of TOML](https://toml.io/en/v1.0.0-rc.3#array-of-tables),
which means you can write more than one `[[bin]]` section to make several
executables in your crate. You can only specify one library, so `[lib]` is a
normal TOML table.

The following is an overview of the TOML settings for each target, with each
field described in detail below.

```toml
[lib]
name = "foo"           # The name of the target.
path = "src/lib.rs"    # The source file of the target.
test = true            # Is tested by default.
doctest = true         # Documentation examples are tested by default.
bench = true           # Is benchmarked by default.
doc = true             # Is documented by default.
plugin = false         # Used as a compiler plugin (deprecated).
proc-macro = false     # Set to `true` for a proc-macro library.
harness = true         # Use libtest harness.
edition = "2015"       # The edition of the target.
crate-type = ["lib"]   # The crate types to generate.
required-features = [] # Features required to build this target (N/A for lib).
```

### The `name` field

The `name` field specifies the name of the target, which corresponds to the
filename of the artifact that will be generated. For a library, this is the
crate name that dependencies will use to reference it.

For the `[lib]` and the default binary (`src/main.rs`), this defaults to the
name of the package, with any dashes replaced with underscores. For other
[auto discovered](#target-auto-discovery) targets, it defaults to the
directory or file name.

This is required for all targets except `[lib]`.

### The `path` field

The `path` field specifies where the source for the crate is located, relative
to the `Cargo.toml` file.

If not specified, the [inferred path](#target-auto-discovery) is used based on
the target name.

### The `test` field

The `test` field indicates whether or not the target is tested by default by
[`cargo test`]. The default is `true` for lib, bins, and tests.

> **Note**: Examples are built by [`cargo test`] by default to ensure they
> continue to compile, but they are not *tested* by default. Setting `test =
> true` for an example will also build it as a test and run any
> [`#[test]`][test-attribute] functions defined in the example.

### The `doctest` field

The `doctest` field indicates whether or not [documentation examples] are
tested by default by [`cargo test`]. This is only relevant for libraries, it
has no effect on other sections. The default is `true` for the library.

### The `bench` field

The `bench` field indicates whether or not the target is benchmarked by
default by [`cargo bench`]. The default is `true` for lib, bins, and
benchmarks.

### The `doc` field

The `doc` field indicates whether or not the target is included in the
documentation generated by [`cargo doc`] by default. The default is `true` for
libraries and binaries.

> **Note**: The binary will be skipped if its name is the same as the lib
> target.

### The `plugin` field

This field is used for `rustc` plugins, which are being deprecated.

### The `proc-macro` field

The `proc-macro` field indicates that the library is a [procedural macro]
([reference][proc-macro-reference]). This is only valid for the `[lib]`
target.

### The `harness` field

The `harness` field indicates that the [`--test` flag] will be passed to
`rustc` which will automatically include the libtest library which is the
driver for collecting and running tests marked with the [`#[test]`
attribute][test-attribute] or benchmarks with the `#[bench]` attribute. The
default is `true` for all targets.

If set to `false`, then you are responsible for defining a `main()` function
to run tests and benchmarks.

Tests have the [`cfg(test)` conditional expression][cfg-test] enabled whether
or not the harness is enabled.

### The `edition` field

The `edition` field defines the [Rust edition] the target will use. If not
specified, it defaults to the [`edition` field][package-edition] for the
`[package]`. This field should usually not be set, and is only intended for
advanced scenarios such as incrementally transitioning a large package to a
new edition.

### The `crate-type` field

The `crate-type` field defines the [crate types] that will be generated by the
target. It is an array of strings, allowing you to specify multiple crate
types for a single target. This can only be specified for libraries and
examples. Binaries, tests, and benchmarks are always the "bin" crate type. The
defaults are:

Target | Crate Type
-------|-----------
Normal library | `"lib"`
Proc-macro library | `"proc-macro"`
Example | `"bin"`

The available options are `bin`, `lib`, `rlib`, `dylib`, `cdylib`,
`staticlib`, and `proc-macro`. You can read more about the different crate
types in the [Rust Reference Manual][crate types].

### The `required-features` field

The `required-features` field specifies which [features] the target needs in
order to be built. If any of the required features are not enabled, the
target will be skipped. This is only relevant for the `[[bin]]`, `[[bench]]`,
`[[test]]`, and `[[example]]` sections, it has no effect on `[lib]`.

```toml
[features]
# ...
postgres = []
sqlite = []
tools = []

[[bin]]
name = "my-pg-tool"
required-features = ["postgres", "tools"]
```


## Target auto-discovery

By default, Cargo automatically determines the targets to build based on the
[layout of the files][package layout] on the filesystem. The target
configuration tables, such as `[lib]`, `[[bin]]`, `[[test]]`, `[[bench]]`, or
`[[example]]`, can be used to add additional targets that don't follow the
standard directory layout.

The automatic target discovery can be disabled so that only manually
configured targets will be built. Setting the keys `autobins`, `autoexamples`,
`autotests`, or `autobenches` to `false` in the `[package]` section will
disable auto-discovery of the corresponding target type.

```toml
[package]
# ...
autobins = false
autoexamples = false
autotests = false
autobenches = false
```

Disabling automatic discovery should only be needed for specialized
situations. For example, if you have a library where you want a *module* named
`bin`, this would present a problem because Cargo would usually attempt to
compile anything in the `bin` directory as an executable. Here is a sample
layout of this scenario:

```text
├── Cargo.toml
└── src
    ├── lib.rs
    └── bin
        └── mod.rs
```

To prevent Cargo from inferring `src/bin/mod.rs` as an executable, set
`autobins = false` in `Cargo.toml` to disable auto-discovery:

```toml
[package]
# …
autobins = false
```

> **Note**: For packages with the 2015 edition, the default for auto-discovery
> is `false` if at least one target is manually defined in `Cargo.toml`.
> Beginning with the 2018 edition, the default is always `true`.


[Build cache]: ../guide/build-cache.md
[Rust Edition]: ../../edition-guide/index.html
[`--test` flag]: ../../rustc/command-line-arguments.html#option-test
[`cargo bench`]: ../commands/cargo-bench.md
[`cargo build`]: ../commands/cargo-build.md
[`cargo doc`]: ../commands/cargo-doc.md
[`cargo install`]: ../commands/cargo-install.md
[`cargo run`]: ../commands/cargo-run.md
[`cargo test`]: ../commands/cargo-test.md
[cfg-test]: ../../reference/conditional-compilation.html#test
[crate types]: ../../reference/linkage.html
[crates.io]: https://crates.io/
[customized]: #configuring-a-target
[dependencies]: specifying-dependencies.md
[dev-dependencies]: specifying-dependencies.md#development-dependencies
[documentation examples]: ../../rustdoc/documentation-tests.html
[features]: features.md
[nightly channel]: ../../book/appendix-07-nightly-rust.html
[package layout]: ../guide/project-layout.md
[package-edition]: manifest.md#the-edition-field
[proc-macro-reference]: ../../reference/procedural-macros.html
[procedural macro]: ../../book/ch19-06-macros.html
[test-attribute]: ../../reference/attributes/testing.html#the-test-attribute
