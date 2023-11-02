# Build Scripts

Some packages need to compile third-party non-Rust code, for example C
libraries. Other packages need to link to C libraries which can either be
located on the system or possibly need to be built from source. Others still
need facilities for functionality such as code generation before building (think
parser generators).

Cargo does not aim to replace other tools that are well-optimized for these
tasks, but it does integrate with them with custom build scripts. Placing a
file named `build.rs` in the root of a package will cause Cargo to compile
that script and execute it just before building the package.

```rust,ignore
// Example custom build script.
fn main() {
    // Tell Cargo that if the given file changes, to rerun this build script.
    println!("cargo:rerun-if-changed=src/hello.c");
    // Use the `cc` crate to build a C file and statically link it.
    cc::Build::new()
        .file("src/hello.c")
        .compile("hello");
}
```

Some example use cases of build scripts are:

* Building a bundled C library.
* Finding a C library on the host system.
* Generating a Rust module from a specification.
* Performing any platform-specific configuration needed for the crate.

The sections below describe how build scripts work, and the [examples
chapter](build-script-examples.md) shows a variety of examples on how to write
scripts.

> Note: The [`package.build` manifest key](manifest.md#the-build-field) can be
> used to change the name of the build script, or disable it entirely.

## Life Cycle of a Build Script

Just before a package is built, Cargo will compile a build script into an
executable (if it has not already been built). It will then run the script,
which may perform any number of tasks. The script may communicate with Cargo
by printing specially formatted commands prefixed with `cargo:` to stdout.

The build script will be rebuilt if any of its source files or dependencies
change.

By default, Cargo will re-run the build script if any of the files in the
package changes. Typically it is best to use the `rerun-if` commands,
described in the [change detection](#change-detection) section below, to
narrow the focus of what triggers a build script to run again.

Once the build script successfully finishes executing, the rest of the package
will be compiled. Scripts should exit with a non-zero exit code to halt the
build if there is an error, in which case the build script's output will be
displayed on the terminal.

## Inputs to the Build Script

When the build script is run, there are a number of inputs to the build script,
all passed in the form of [environment variables][build-env].

In addition to environment variables, the build script’s current directory is
the source directory of the build script’s package.

[build-env]: environment-variables.md#environment-variables-cargo-sets-for-build-scripts

## Outputs of the Build Script

Build scripts may save any output files or intermediate artifacts in the
directory specified in the [`OUT_DIR` environment variable][build-env]. Scripts
should not modify any files outside of that directory.

Build scripts communicate with Cargo by printing to stdout. Cargo will
interpret each line that starts with `cargo:` as an instruction that will
influence compilation of the package. All other lines are ignored.

> Note: The order of `cargo:` instructions printed by the build script *may*
> affect the order of arguments that `cargo` passes to `rustc`. In turn, the
> order of arguments passed to `rustc` may affect the order of arguments passed
> to the linker. Therefore, you will want to pay attention to the order of the
> build script's instructions. For example, if object `foo` needs to link against
> library `bar`, you may want to make sure that library `bar`'s
> [`cargo:rustc-link-lib`](#rustc-link-lib) instruction appears *after*
> instructions to link object `foo`.

The output of the script is hidden from the terminal during normal
compilation. If you would like to see the output directly in your terminal,
invoke Cargo as "very verbose" with the `-vv` flag. This only happens when the
build script is run. If Cargo determines nothing has changed, it will not
re-run the script, see [change detection](#change-detection) below for more.

All the lines printed to stdout by a build script are written to a file like
`target/debug/build/<pkg>/output` (the precise location may depend on your
configuration). The stderr output is also saved in that same directory.

The following is a summary of the instructions that Cargo recognizes, with each
one detailed below.

* [`cargo:rerun-if-changed=PATH`](#rerun-if-changed) --- Tells Cargo when to
  re-run the script.
* [`cargo:rerun-if-env-changed=VAR`](#rerun-if-env-changed) --- Tells Cargo when
  to re-run the script.
* [`cargo:rustc-link-arg=FLAG`](#rustc-link-arg) --- Passes custom flags to a
  linker for benchmarks, binaries, `cdylib` crates, examples, and tests.
* [`cargo:rustc-link-arg-bin=BIN=FLAG`](#rustc-link-arg-bin) --- Passes custom
  flags to a linker for the binary `BIN`.
* [`cargo:rustc-link-arg-bins=FLAG`](#rustc-link-arg-bins) --- Passes custom
  flags to a linker for binaries.
* [`cargo:rustc-link-arg-tests=FLAG`](#rustc-link-arg-tests) --- Passes custom
  flags to a linker for tests.
* [`cargo:rustc-link-arg-examples=FLAG`](#rustc-link-arg-examples) --- Passes custom
  flags to a linker for examples.
* [`cargo:rustc-link-arg-benches=FLAG`](#rustc-link-arg-benches) --- Passes custom
  flags to a linker for benchmarks.
* [`cargo:rustc-link-lib=LIB`](#rustc-link-lib) --- Adds a library to
  link.
* [`cargo:rustc-link-search=[KIND=]PATH`](#rustc-link-search) --- Adds to the
  library search path.
* [`cargo:rustc-flags=FLAGS`](#rustc-flags) --- Passes certain flags to the
  compiler.
* [`cargo:rustc-cfg=KEY[="VALUE"]`](#rustc-cfg) --- Enables compile-time `cfg`
  settings.
* [`cargo:rustc-env=VAR=VALUE`](#rustc-env) --- Sets an environment variable.
* [`cargo:rustc-cdylib-link-arg=FLAG`](#rustc-cdylib-link-arg) --- Passes custom
  flags to a linker for cdylib crates.
* [`cargo:warning=MESSAGE`](#cargo-warning) --- Displays a warning on the
  terminal.
* [`cargo:KEY=VALUE`](#the-links-manifest-key) --- Metadata, used by `links`
  scripts.


### `cargo:rustc-link-arg=FLAG` {#rustc-link-arg}

The `rustc-link-arg` instruction tells Cargo to pass the [`-C link-arg=FLAG`
option][link-arg] to the compiler, but only when building supported targets
(benchmarks, binaries, `cdylib` crates, examples, and tests). Its usage is
highly platform specific. It is useful to set the shared library version or
linker script.

[link-arg]: ../../rustc/codegen-options/index.md#link-arg

### `cargo:rustc-link-arg-bin=BIN=FLAG` {#rustc-link-arg-bin}

The `rustc-link-arg-bin` instruction tells Cargo to pass the [`-C
link-arg=FLAG` option][link-arg] to the compiler, but only when building
the binary target with name `BIN`. Its usage is highly platform specific. It is useful
to set a linker script or other linker options.


### `cargo:rustc-link-arg-bins=FLAG` {#rustc-link-arg-bins}

The `rustc-link-arg-bins` instruction tells Cargo to pass the [`-C
link-arg=FLAG` option][link-arg] to the compiler, but only when building a
binary target. Its usage is highly platform specific. It is useful
to set a linker script or other linker options.


### `cargo:rustc-link-lib=LIB` {#rustc-link-lib}

The `rustc-link-lib` instruction tells Cargo to link the given library using
the compiler's [`-l` flag][option-link]. This is typically used to link a
native library using [FFI].

The `LIB` string is passed directly to rustc, so it supports any syntax that
`-l` does. \
Currently the full supported syntax for `LIB` is `[KIND[:MODIFIERS]=]NAME[:RENAME]`.

The `-l` flag is only passed to the library target of the package, unless
there is no library target, in which case it is passed to all targets. This is
done because all other targets have an implicit dependency on the library
target, and the given library to link should only be included once. This means
that if a package has both a library and a binary target, the *library* has
access to the symbols from the given lib, and the binary should access them
through the library target's public API.

The optional `KIND` may be one of `dylib`, `static`, or `framework`. See the
[rustc book][option-link] for more detail.

[option-link]: ../../rustc/command-line-arguments.md#option-l-link-lib
[FFI]: ../../nomicon/ffi.md


### `cargo:rustc-link-arg-tests=FLAG` {#rustc-link-arg-tests}

The `rustc-link-arg-tests` instruction tells Cargo to pass the [`-C
link-arg=FLAG` option][link-arg] to the compiler, but only when building a
tests target.


### `cargo:rustc-link-arg-examples=FLAG` {#rustc-link-arg-examples}

The `rustc-link-arg-examples` instruction tells Cargo to pass the [`-C
link-arg=FLAG` option][link-arg] to the compiler, but only when building an examples
target.

### `cargo:rustc-link-arg-benches=FLAG` {#rustc-link-arg-benches}

The `rustc-link-arg-benches` instruction tells Cargo to pass the [`-C
link-arg=FLAG` option][link-arg] to the compiler, but only when building a benchmark
target.

### `cargo:rustc-link-search=[KIND=]PATH` {#rustc-link-search}

The `rustc-link-search` instruction tells Cargo to pass the [`-L`
flag][option-search] to the compiler to add a directory to the library search
path.

The optional `KIND` may be one of `dependency`, `crate`, `native`,
`framework`, or `all`. See the [rustc book][option-search] for more detail.

These paths are also added to the [dynamic library search path environment
variable](environment-variables.md#dynamic-library-paths) if they are within
the `OUT_DIR`. Depending on this behavior is discouraged since this makes it
difficult to use the resulting binary. In general, it is best to avoid
creating dynamic libraries in a build script (using existing system libraries
is fine).

[option-search]: ../../rustc/command-line-arguments.md#option-l-search-path

### `cargo:rustc-flags=FLAGS` {#rustc-flags}

The `rustc-flags` instruction tells Cargo to pass the given space-separated
flags to the compiler. This only allows the `-l` and `-L` flags, and is
equivalent to using [`rustc-link-lib`](#rustc-link-lib) and
[`rustc-link-search`](#rustc-link-search).

### `cargo:rustc-cfg=KEY[="VALUE"]` {#rustc-cfg}

The `rustc-cfg` instruction tells Cargo to pass the given value to the
[`--cfg` flag][option-cfg] to the compiler. This may be used for compile-time
detection of features to enable [conditional compilation].

Note that this does *not* affect Cargo's dependency resolution. This cannot be
used to enable an optional dependency, or enable other Cargo features.

Be aware that [Cargo features] use the form `feature="foo"`. `cfg` values
passed with this flag are not restricted to that form, and may provide just a
single identifier, or any arbitrary key/value pair. For example, emitting
`cargo:rustc-cfg=abc` will then allow code to use `#[cfg(abc)]` (note the lack
of `feature=`). Or an arbitrary key/value pair may be used with an `=` symbol
like `cargo:rustc-cfg=my_component="foo"`. The key should be a Rust
identifier, the value should be a string.

[cargo features]: features.md
[conditional compilation]: ../../reference/conditional-compilation.md
[option-cfg]: ../../rustc/command-line-arguments.md#option-cfg

### `cargo:rustc-env=VAR=VALUE` {#rustc-env}

The `rustc-env` instruction tells Cargo to set the given environment variable
when compiling the package. The value can be then retrieved by the [`env!`
macro][env-macro] in the compiled crate. This is useful for embedding
additional metadata in crate's code, such as the hash of git HEAD or the
unique identifier of a continuous integration server.

See also the [environment variables automatically included by
Cargo][env-cargo].

> **Note**: These environment variables are also set when running an
> executable with `cargo run` or `cargo test`. However, this usage is
> discouraged since it ties the executable to Cargo's execution environment.
> Normally, these environment variables should only be checked at compile-time
> with the `env!` macro.

[env-macro]: ../../std/macro.env.html
[env-cargo]: environment-variables.md#environment-variables-cargo-sets-for-crates

### `cargo:rustc-cdylib-link-arg=FLAG` {#rustc-cdylib-link-arg}

The `rustc-cdylib-link-arg` instruction tells Cargo to pass the [`-C
link-arg=FLAG` option][link-arg] to the compiler, but only when building a
`cdylib` library target. Its usage is highly platform specific. It is useful
to set the shared library version or the runtime-path.


### `cargo:warning=MESSAGE` {#cargo-warning}

The `warning` instruction tells Cargo to display a warning after the build
script has finished running. Warnings are only shown for `path` dependencies
(that is, those you're working on locally), so for example warnings printed
out in [crates.io] crates are not emitted by default. The `-vv` "very verbose"
flag may be used to have Cargo display warnings for all crates.

## Build Dependencies

Build scripts are also allowed to have dependencies on other Cargo-based crates.
Dependencies are declared through the `build-dependencies` section of the
manifest.

```toml
[build-dependencies]
cc = "1.0.46"
```

The build script **does not** have access to the dependencies listed in the
`dependencies` or `dev-dependencies` section (they’re not built yet!). Also,
build dependencies are not available to the package itself unless also
explicitly added in the `[dependencies]` table.

It is recommended to carefully consider each dependency you add, weighing
against the impact on compile time, licensing, maintenance, etc. Cargo will
attempt to reuse a dependency if it is shared between build dependencies and
normal dependencies. However, this is not always possible, for example when
cross-compiling, so keep that in consideration of the impact on compile time.

## Change Detection

When rebuilding a package, Cargo does not necessarily know if the build script
needs to be run again. By default, it takes a conservative approach of always
re-running the build script if any file within the package is changed (or the
list of files controlled by the [`exclude` and `include` fields]). For most
cases, this is not a good choice, so it is recommended that every build script
emit at least one of the `rerun-if` instructions (described below). If these
are emitted, then Cargo will only re-run the script if the given value has
changed. If Cargo is re-running the build scripts of your own crate or a
dependency and you don't know why, see ["Why is Cargo rebuilding my code?" in the
FAQ](../faq.md#why-is-cargo-rebuilding-my-code).

[`exclude` and `include` fields]: manifest.md#the-exclude-and-include-fields

### `cargo:rerun-if-changed=PATH` {#rerun-if-changed}

The `rerun-if-changed` instruction tells Cargo to re-run the build script if
the file at the given path has changed. Currently, Cargo only uses the
filesystem last-modified "mtime" timestamp to determine if the file has
changed. It compares against an internal cached timestamp of when the build
script last ran.

If the path points to a directory, it will scan the entire directory for
any modifications.

If the build script inherently does not need to re-run under any circumstance,
then emitting `cargo:rerun-if-changed=build.rs` is a simple way to prevent it
from being re-run (otherwise, the default if no `rerun-if` instructions are
emitted is to scan the entire package directory for changes). Cargo
automatically handles whether or not the script itself needs to be recompiled,
and of course the script will be re-run after it has been recompiled.
Otherwise, specifying `build.rs` is redundant and unnecessary.

### `cargo:rerun-if-env-changed=NAME` {#rerun-if-env-changed}

The `rerun-if-env-changed` instruction tells Cargo to re-run the build script
if the value of an environment variable of the given name has changed.

Note that the environment variables here are intended for global environment
variables like `CC` and such, it is not possible to use this for environment
variables like `TARGET` that [Cargo sets for build scripts][build-env]. The
environment variables in use are those received by `cargo` invocations, not
those received by the executable of the build script.


## The `links` Manifest Key

The `package.links` key may be set in the `Cargo.toml` manifest to declare
that the package links with the given native library. The purpose of this
manifest key is to give Cargo an understanding about the set of native
dependencies that a package has, as well as providing a principled system of
passing metadata between package build scripts.

```toml
[package]
# ...
links = "foo"
```

This manifest states that the package links to the `libfoo` native library.
When using the `links` key, the package must have a build script, and the
build script should use the [`rustc-link-lib` instruction](#rustc-link-lib) to
link the library.

Primarily, Cargo requires that there is at most one package per `links` value.
In other words, it is forbidden to have two packages link to the same native
library. This helps prevent duplicate symbols between crates. Note, however,
that there are [conventions in place](#-sys-packages) to alleviate this.

As mentioned above in the output format, each build script can generate an
arbitrary set of metadata in the form of key-value pairs. This metadata is
passed to the build scripts of **dependent** packages. For example, if the
package `bar` depends on `foo`, then if `foo` generates `key=value` as part of
its build script metadata, then the build script of `bar` will have the
environment variables `DEP_FOO_KEY=value`. See the ["Using another `sys`
crate"][using-another-sys] for an example of
how this can be used.

Note that metadata is only passed to immediate dependents, not transitive
dependents.

[using-another-sys]: build-script-examples.md#using-another-sys-crate

## `*-sys` Packages

Some Cargo packages that link to system libraries have a naming convention of
having a `-sys` suffix. Any package named `foo-sys` should provide two major
pieces of functionality:

* The library crate should link to the native library `libfoo`. This will often
  probe the current system for `libfoo` before resorting to building from
  source.
* The library crate should provide **declarations** for types and functions in
  `libfoo`, but **not** higher-level abstractions.

The set of `*-sys` packages provides a common set of dependencies for linking
to native libraries. There are a number of benefits earned from having this
convention of native-library-related packages:

* Common dependencies on `foo-sys` alleviates the rule about one package per
  value of `links`.
* Other `-sys` packages can take advantage of the `DEP_NAME_KEY=value`
  environment variables to better integrate with other packages. See the
  ["Using another `sys` crate"][using-another-sys] example.
* A common dependency allows centralizing logic on discovering `libfoo` itself
  (or building it from source).
* These dependencies are easily [overridable](#overriding-build-scripts).

It is common to have a companion package without the `-sys` suffix that
provides a safe, high-level abstractions on top of the sys package. For
example, the [`git2` crate] provides a high-level interface to the
[`libgit2-sys` crate].

[`git2` crate]: https://crates.io/crates/git2
[`libgit2-sys` crate]: https://crates.io/crates/libgit2-sys

## Overriding Build Scripts

If a manifest contains a `links` key, then Cargo supports overriding the build
script specified with a custom library. The purpose of this functionality is to
prevent running the build script in question altogether and instead supply the
metadata ahead of time.

To override a build script, place the following configuration in any acceptable [`config.toml`](config.md) file.

```toml
[target.x86_64-unknown-linux-gnu.foo]
rustc-link-lib = ["foo"]
rustc-link-search = ["/path/to/foo"]
rustc-flags = "-L /some/path"
rustc-cfg = ['key="value"']
rustc-env = {key = "value"}
rustc-cdylib-link-arg = ["…"]
metadata_key1 = "value"
metadata_key2 = "value"
```

With this configuration, if a package declares that it links to `foo` then the
build script will **not** be compiled or run, and the metadata specified will
be used instead.

The `warning`, `rerun-if-changed`, and `rerun-if-env-changed` keys should not
be used and will be ignored.

## Jobserver

Cargo and `rustc` use the [jobserver protocol], developed for GNU make, to
coordinate concurrency across processes. It is essentially a semaphore that
controls the number of jobs running concurrently. The concurrency may be set
with the `--jobs` flag, which defaults to the number of logical CPUs.

Each build script inherits one job slot from Cargo, and should endeavor to
only use one CPU while it runs. If the script wants to use more CPUs in
parallel, it should use the [`jobserver` crate] to coordinate with Cargo.

As an example, the [`cc` crate] may enable the optional `parallel` feature
which will use the jobserver protocol to attempt to build multiple C files
at the same time.

[`cc` crate]: https://crates.io/crates/cc
[`jobserver` crate]: https://crates.io/crates/jobserver
[jobserver protocol]: http://make.mad-scientist.net/papers/jobserver-implementation/
[crates.io]: https://crates.io/
