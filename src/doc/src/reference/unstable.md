# Unstable Features

Experimental Cargo features are only available on the [nightly channel]. You
are encouraged to experiment with these features to see if they meet your
needs, and if there are any issues or problems. Check the linked tracking
issues listed below for more information on the feature, and click the GitHub
subscribe button if you want future updates.

After some period of time, if the feature does not have any major concerns, it
can be [stabilized], which will make it available on stable once the current
nightly release reaches the stable channel (anywhere from 6 to 12 weeks).

There are three different ways that unstable features can be enabled based on
how the feature works:

* New syntax in `Cargo.toml` requires a `cargo-features` key at the top of
  `Cargo.toml`, before any tables. For example:

  ```toml
  # This specifies which new Cargo.toml features are enabled.
  cargo-features = ["test-dummy-unstable"]

  [package]
  name = "my-package"
  version = "0.1.0"
  im-a-teapot = true  # This is a new option enabled by test-dummy-unstable.
  ```

* New command-line flags, options, and subcommands require the `-Z
  unstable-options` CLI option to also be included. For example, the new
  `--out-dir` option is only available on nightly:

  ```cargo +nightly build --out-dir=out -Z unstable-options```

* `-Z` command-line flags are used to enable new functionality that may not
  have an interface, or the interface has not yet been designed, or for more
  complex features that affect multiple parts of Cargo. For example, the
  [mtime-on-use](#mtime-on-use) feature can be enabled with:

  ```cargo +nightly build -Z mtime-on-use```

  Run `cargo -Z help` to see a list of flags available.

  Anything which can be configured with a `-Z` flag can also be set in the
  cargo [config file] (`.cargo/config.toml`) in the `unstable` table. For
  example:

  ```toml
  [unstable]
  mtime-on-use = true
  build-std = ["core", "alloc"]
  ```

Each new feature described below should explain how to use it.
For the latest nightly, see the [nightly version] of this page.

[config file]: config.md
[nightly channel]: ../../book/appendix-07-nightly-rust.html
[stabilized]: https://doc.crates.io/contrib/process/unstable.html#stabilization
[nightly version]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#script

## List of unstable features

* Unstable-specific features
    * [-Z allow-features](#allow-features) --- Provides a way to restrict which unstable features are used.
* Build scripts and linking
    * [Metabuild](#metabuild) --- Provides declarative build scripts.
* Resolver and features
    * [no-index-update](#no-index-update) --- Prevents cargo from updating the index cache.
    * [avoid-dev-deps](#avoid-dev-deps) --- Prevents the resolver from including dev-dependencies during resolution.
    * [minimal-versions](#minimal-versions) --- Forces the resolver to use the lowest compatible version instead of the highest.
    * [direct-minimal-versions](#direct-minimal-versions) — Forces the resolver to use the lowest compatible version instead of the highest.
    * [public-dependency](#public-dependency) --- Allows dependencies to be classified as either public or private.
    * [msrv-policy](#msrv-policy) --- MSRV-aware resolver and version selection
* Output behavior
    * [out-dir](#out-dir) --- Adds a directory where artifacts are copied to.
    * [Different binary name](#different-binary-name) --- Assign a name to the built binary that is separate from the crate name.
* Compile behavior
    * [mtime-on-use](#mtime-on-use) --- Updates the last-modified timestamp on every dependency every time it is used, to provide a mechanism to delete unused artifacts.
    * [doctest-xcompile](#doctest-xcompile) --- Supports running doctests with the `--target` flag.
    * [build-std](#build-std) --- Builds the standard library instead of using pre-built binaries.
    * [build-std-features](#build-std-features) --- Sets features to use with the standard library.
    * [binary-dep-depinfo](#binary-dep-depinfo) --- Causes the dep-info file to track binary dependencies.
    * [panic-abort-tests](#panic-abort-tests) --- Allows running tests with the "abort" panic strategy.
    * [check-cfg](#check-cfg) --- Compile-time validation of `cfg` expressions.
    * [host-config](#host-config) --- Allows setting `[target]`-like configuration settings for host build targets.
    * [target-applies-to-host](#target-applies-to-host) --- Alters whether certain flags will be passed to host build targets.
* rustdoc
    * [rustdoc-map](#rustdoc-map) --- Provides mappings for documentation to link to external sites like [docs.rs](https://docs.rs/).
    * [scrape-examples](#scrape-examples) --- Shows examples within documentation.
* `Cargo.toml` extensions
    * [Profile `rustflags` option](#profile-rustflags-option) --- Passed directly to rustc.
    * [codegen-backend](#codegen-backend) --- Select the codegen backend used by rustc.
    * [per-package-target](#per-package-target) --- Sets the `--target` to use for each individual package.
    * [artifact dependencies](#artifact-dependencies) --- Allow build artifacts to be included into other build artifacts and build them for different targets.
    * [Edition 2024](#edition-2024) — Adds support for the 2024 Edition.
* Information and metadata
    * [Build-plan](#build-plan) --- Emits JSON information on which commands will be run.
    * [unit-graph](#unit-graph) --- Emits JSON for Cargo's internal graph structure.
    * [`cargo rustc --print`](#rustc---print) --- Calls rustc with `--print` to display information from rustc.
* Configuration
    * [config-include](#config-include) --- Adds the ability for config files to include other files.
    * [`cargo config`](#cargo-config) --- Adds a new subcommand for viewing config files.
* Registries
    * [publish-timeout](#publish-timeout) --- Controls the timeout between uploading the crate and being available in the index
    * [asymmetric-token](#asymmetric-token) --- Adds support for authentication tokens using asymmetric cryptography (`cargo:paseto` provider).
* Other
    * [gitoxide](#gitoxide) --- Use `gitoxide` instead of `git2` for a set of operations.
    * [script](#script) --- Enable support for single-file `.rs` packages.

## allow-features

This permanently-unstable flag makes it so that only a listed set of
unstable features can be used. Specifically, if you pass
`-Zallow-features=foo,bar`, you'll continue to be able to pass `-Zfoo`
and `-Zbar` to `cargo`, but you will be unable to pass `-Zbaz`. You can
pass an empty string (`-Zallow-features=`) to disallow all unstable
features.

`-Zallow-features` also restricts which unstable features can be passed
to the `cargo-features` entry in `Cargo.toml`. If, for example, you want
to allow

```toml
cargo-features = ["test-dummy-unstable"]
```

where `test-dummy-unstable` is unstable, that features would also be
disallowed by `-Zallow-features=`, and allowed with
`-Zallow-features=test-dummy-unstable`.

The list of features passed to cargo's `-Zallow-features` is also passed
to any Rust tools that cargo ends up calling (like `rustc` or
`rustdoc`). Thus, if you run `cargo -Zallow-features=`, no unstable
Cargo _or_ Rust features can be used.

## no-index-update
* Original Issue: [#3479](https://github.com/rust-lang/cargo/issues/3479)
* Tracking Issue: [#7404](https://github.com/rust-lang/cargo/issues/7404)

The `-Z no-index-update` flag ensures that Cargo does not attempt to update
the registry index. This is intended for tools such as Crater that issue many
Cargo commands, and you want to avoid the network latency for updating the
index each time.

## mtime-on-use
* Original Issue: [#6477](https://github.com/rust-lang/cargo/pull/6477)
* Cache usage meta tracking issue: [#7150](https://github.com/rust-lang/cargo/issues/7150)

The `-Z mtime-on-use` flag is an experiment to have Cargo update the mtime of
used files to make it easier for tools like cargo-sweep to detect which files
are stale. For many workflows this needs to be set on *all* invocations of cargo.
To make this more practical setting the `unstable.mtime_on_use` flag in `.cargo/config.toml`
or the corresponding ENV variable will apply the `-Z mtime-on-use` to all
invocations of nightly cargo. (the config flag is ignored by stable)

## avoid-dev-deps
* Original Issue: [#4988](https://github.com/rust-lang/cargo/issues/4988)
* Tracking Issue: [#5133](https://github.com/rust-lang/cargo/issues/5133)

When running commands such as `cargo install` or `cargo build`, Cargo
currently requires dev-dependencies to be downloaded, even if they are not
used. The `-Z avoid-dev-deps` flag allows Cargo to avoid downloading
dev-dependencies if they are not needed. The `Cargo.lock` file will not be
generated if dev-dependencies are skipped.

## minimal-versions
* Original Issue: [#4100](https://github.com/rust-lang/cargo/issues/4100)
* Tracking Issue: [#5657](https://github.com/rust-lang/cargo/issues/5657)

> Note: It is not recommended to use this feature. Because it enforces minimal
> versions for all transitive dependencies, its usefulness is limited since
> not all external dependencies declare proper lower version bounds. It is
> intended that it will be changed in the future to only enforce minimal
> versions for direct dependencies.

When a `Cargo.lock` file is generated, the `-Z minimal-versions` flag will
resolve the dependencies to the minimum SemVer version that will satisfy the
requirements (instead of the greatest version).

The intended use-case of this flag is to check, during continuous integration,
that the versions specified in Cargo.toml are a correct reflection of the
minimum versions that you are actually using. That is, if Cargo.toml says
`foo = "1.0.0"` that you don't accidentally depend on features added only in
`foo 1.5.0`.

## direct-minimal-versions
* Original Issue: [#4100](https://github.com/rust-lang/cargo/issues/4100)
* Tracking Issue: [#5657](https://github.com/rust-lang/cargo/issues/5657)

When a `Cargo.lock` file is generated, the `-Z direct-minimal-versions` flag will
resolve the dependencies to the minimum SemVer version that will satisfy the
requirements (instead of the greatest version) for direct dependencies only.

The intended use-case of this flag is to check, during continuous integration,
that the versions specified in Cargo.toml are a correct reflection of the
minimum versions that you are actually using. That is, if Cargo.toml says
`foo = "1.0.0"` that you don't accidentally depend on features added only in
`foo 1.5.0`.

Indirect dependencies are resolved as normal so as not to be blocked on their
minimal version validation.

## out-dir
* Original Issue: [#4875](https://github.com/rust-lang/cargo/issues/4875)
* Tracking Issue: [#6790](https://github.com/rust-lang/cargo/issues/6790)

This feature allows you to specify the directory where artifacts will be
copied to after they are built. Typically artifacts are only written to the
`target/release` or `target/debug` directories. However, determining the
exact filename can be tricky since you need to parse JSON output. The
`--out-dir` flag makes it easier to predictably access the artifacts. Note
that the artifacts are copied, so the originals are still in the `target`
directory. Example:

```sh
cargo +nightly build --out-dir=out -Z unstable-options
```

This can also be specified in `.cargo/config.toml` files.

```toml
[build]
out-dir = "out"
```

## doctest-xcompile
* Tracking Issue: [#7040](https://github.com/rust-lang/cargo/issues/7040)
* Tracking Rustc Issue: [#64245](https://github.com/rust-lang/rust/issues/64245)

This flag changes `cargo test`'s behavior when handling doctests when
a target is passed. Currently, if a target is passed that is different
from the host cargo will simply skip testing doctests. If this flag is
present, cargo will continue as normal, passing the tests to doctest,
while also passing it a `--target` option, as well as enabling
`-Zunstable-features --enable-per-target-ignores` and passing along
information from `.cargo/config.toml`. See the rustc issue for more information.

```sh
cargo test --target foo -Zdoctest-xcompile
```

## Build-plan
* Tracking Issue: [#5579](https://github.com/rust-lang/cargo/issues/5579)

The `--build-plan` argument for the `build` command will output JSON with
information about which commands would be run without actually executing
anything. This can be useful when integrating with another build tool.
Example:

```sh
cargo +nightly build --build-plan -Z unstable-options
```

## Metabuild
* Tracking Issue: [rust-lang/rust#49803](https://github.com/rust-lang/rust/issues/49803)
* RFC: [#2196](https://github.com/rust-lang/rfcs/blob/master/text/2196-metabuild.md)

Metabuild is a feature to have declarative build scripts. Instead of writing
a `build.rs` script, you specify a list of build dependencies in the
`metabuild` key in `Cargo.toml`. A build script is automatically generated
that runs each build dependency in order. Metabuild packages can then read
metadata from `Cargo.toml` to specify their behavior.

Include `cargo-features` at the top of `Cargo.toml`, a `metabuild` key in the
`package`, list the dependencies in `build-dependencies`, and add any metadata
that the metabuild packages require under `package.metadata`. Example:

```toml
cargo-features = ["metabuild"]

[package]
name = "mypackage"
version = "0.0.1"
metabuild = ["foo", "bar"]

[build-dependencies]
foo = "1.0"
bar = "1.0"

[package.metadata.foo]
extra-info = "qwerty"
```

Metabuild packages should have a public function called `metabuild` that
performs the same actions as a regular `build.rs` script would perform.

## public-dependency
* Tracking Issue: [#44663](https://github.com/rust-lang/rust/issues/44663)

The 'public-dependency' feature allows marking dependencies as 'public'
or 'private'. When this feature is enabled, additional information is passed to rustc to allow
the 'exported_private_dependencies' lint to function properly.

This requires the appropriate key to be set in `cargo-features`:

```toml
cargo-features = ["public-dependency"]

[dependencies]
my_dep = { version = "1.2.3", public = true }
private_dep = "2.0.0" # Will be 'private' by default
```

## msrv-policy
- [#9930](https://github.com/rust-lang/cargo/issues/9930) (MSRV-aware resolver)
- [#10653](https://github.com/rust-lang/cargo/issues/10653) (MSRV-aware cargo-add)
- [#10903](https://github.com/rust-lang/cargo/issues/10903) (MSRV-aware cargo-install)

The `msrv-policy` feature enables experiments in MSRV-aware policy for cargo in
preparation for an upcoming RFC.

## build-std
* Tracking Repository: <https://github.com/rust-lang/wg-cargo-std-aware>

The `build-std` feature enables Cargo to compile the standard library itself as
part of a crate graph compilation. This feature has also historically been known
as "std-aware Cargo". This feature is still in very early stages of development,
and is also a possible massive feature addition to Cargo. This is a very large
feature to document, even in the minimal form that it exists in today, so if
you're curious to stay up to date you'll want to follow the [tracking
repository](https://github.com/rust-lang/wg-cargo-std-aware) and its set of
issues.

The functionality implemented today is behind a flag called `-Z build-std`. This
flag indicates that Cargo should compile the standard library from source code
using the same profile as the main build itself. Note that for this to work you
need to have the source code for the standard library available, and at this
time the only supported method of doing so is to add the `rust-src` rust rustup
component:

```console
$ rustup component add rust-src --toolchain nightly
```

It is also required today that the `-Z build-std` flag is combined with the
`--target` flag. Note that you're not forced to do a cross compilation, you're
just forced to pass `--target` in one form or another.

Usage looks like:

```console
$ cargo new foo
$ cd foo
$ cargo +nightly run -Z build-std --target x86_64-unknown-linux-gnu
   Compiling core v0.0.0 (...)
   ...
   Compiling foo v0.1.0 (...)
    Finished dev [unoptimized + debuginfo] target(s) in 21.00s
     Running `target/x86_64-unknown-linux-gnu/debug/foo`
Hello, world!
```

Here we recompiled the standard library in debug mode with debug assertions
(like `src/main.rs` is compiled) and everything was linked together at the end.

Using `-Z build-std` will implicitly compile the stable crates `core`, `std`,
`alloc`, and `proc_macro`. If you're using `cargo test` it will also compile the
`test` crate. If you're working with an environment which does not support some
of these crates, then you can pass an argument to `-Zbuild-std` as well:

```console
$ cargo +nightly build -Z build-std=core,alloc
```

The value here is a comma-separated list of standard library crates to build.

### Requirements

As a summary, a list of requirements today to use `-Z build-std` are:

* You must install libstd's source code through `rustup component add rust-src`
* You must pass `--target`
* You must use both a nightly Cargo and a nightly rustc
* The `-Z build-std` flag must be passed to all `cargo` invocations.

### Reporting bugs and helping out

The `-Z build-std` feature is in the very early stages of development! This
feature for Cargo has an extremely long history and is very large in scope, and
this is just the beginning. If you'd like to report bugs please either report
them to:

* Cargo --- <https://github.com/rust-lang/cargo/issues/new> --- for implementation bugs
* The tracking repository ---
  <https://github.com/rust-lang/wg-cargo-std-aware/issues/new> --- for larger design
  questions.

Also if you'd like to see a feature that's not yet implemented and/or if
something doesn't quite work the way you'd like it to, feel free to check out
the [issue tracker](https://github.com/rust-lang/wg-cargo-std-aware/issues) of
the tracking repository, and if it's not there please file a new issue!

## build-std-features
* Tracking Repository: <https://github.com/rust-lang/wg-cargo-std-aware>

This flag is a sibling to the `-Zbuild-std` feature flag. This will configure
the features enabled for the standard library itself when building the standard
library. The default enabled features, at this time, are `backtrace` and
`panic-unwind`. This flag expects a comma-separated list and, if provided, will
override the default list of features enabled.

## binary-dep-depinfo
* Tracking rustc issue: [#63012](https://github.com/rust-lang/rust/issues/63012)

The `-Z binary-dep-depinfo` flag causes Cargo to forward the same flag to
`rustc` which will then cause `rustc` to include the paths of all binary
dependencies in the "dep info" file (with the `.d` extension). Cargo then uses
that information for change-detection (if any binary dependency changes, then
the crate will be rebuilt). The primary use case is for building the compiler
itself, which has implicit dependencies on the standard library that would
otherwise be untracked for change-detection.

## panic-abort-tests
* Tracking Issue: [#67650](https://github.com/rust-lang/rust/issues/67650)
* Original Pull Request: [#7460](https://github.com/rust-lang/cargo/pull/7460)

The `-Z panic-abort-tests` flag will enable nightly support to compile test
harness crates with `-Cpanic=abort`. Without this flag Cargo will compile tests,
and everything they depend on, with `-Cpanic=unwind` because it's the only way
`test`-the-crate knows how to operate. As of [rust-lang/rust#64158], however,
the `test` crate supports `-C panic=abort` with a test-per-process, and can help
avoid compiling crate graphs multiple times.

It's currently unclear how this feature will be stabilized in Cargo, but we'd
like to stabilize it somehow!

[rust-lang/rust#64158]: https://github.com/rust-lang/rust/pull/64158

## config-include
* Tracking Issue: [#7723](https://github.com/rust-lang/cargo/issues/7723)

This feature requires the `-Zconfig-include` command-line option.

The `include` key in a config file can be used to load another config file. It
takes a string for a path to another file relative to the config file, or an
array of config file paths. Only path ending with `.toml` is accepted.

```toml
# a path ending with `.toml`
include = "path/to/mordor.toml"

# or an array of paths
include = ["frodo.toml", "samwise.toml"]
```

Unlike other config values, the merge behavior of the `include` key is
different. When a config file contains an `include` key:

1. The config values are first loaded from the `include` path.
    * If the value of the `include` key is an array of paths, the config values
      are loaded and merged from left to right for each path.
    * Recurse this step if the config values from the `include` path also
      contain an `include` key.
2. Then, the config file's own values are merged on top of the config
   from the `include` path.

## target-applies-to-host
* Original Pull Request: [#9322](https://github.com/rust-lang/cargo/pull/9322)
* Tracking Issue: [#9453](https://github.com/rust-lang/cargo/issues/9453)

Historically, Cargo's behavior for whether the `linker` and `rustflags`
configuration options from environment variables and
[`[target]`](config.md#target) are respected for build scripts, plugins,
and other artifacts that are _always_ built for the host platform has
been somewhat inconsistent.
When `--target` is _not_ passed, Cargo respects the same `linker` and
`rustflags` for build scripts as for all other compile artifacts. When
`--target` _is_ passed, however, Cargo respects `linker` from
[`[target.<host triple>]`](config.md#targettriplelinker), and does not
pick up any `rustflags` configuration.
This dual behavior is confusing, but also makes it difficult to correctly
configure builds where the host triple and the [target triple] happen to
be the same, but artifacts intended to run on the build host should still
be configured differently.

`-Ztarget-applies-to-host` enables the top-level
`target-applies-to-host` setting in Cargo configuration files which
allows users to opt into different (and more consistent) behavior for
these properties. When `target-applies-to-host` is unset, or set to
`true`, in the configuration file, the existing Cargo behavior is
preserved (though see `-Zhost-config`, which changes that default). When
it is set to `false`, no options from `[target.<host triple>]`,
`RUSTFLAGS`, or `[build]` are respected for host artifacts regardless of
whether `--target` is passed to Cargo. To customize artifacts intended
to be run on the host, use `[host]` ([`host-config`](#host-config)).

In the future, `target-applies-to-host` may end up defaulting to `false`
to provide more sane and consistent default behavior.

```toml
# config.toml
target-applies-to-host = false
```

```console
cargo +nightly -Ztarget-applies-to-host build --target x86_64-unknown-linux-gnu
```

## host-config
* Original Pull Request: [#9322](https://github.com/rust-lang/cargo/pull/9322)
* Tracking Issue: [#9452](https://github.com/rust-lang/cargo/issues/9452)

The `host` key in a config file can be used pass flags to host build targets
such as build scripts that must run on the host system instead of the target
system when cross compiling. It supports both generic and host arch specific
tables. Matching host arch tables take precedence over generic host tables.

It requires the `-Zhost-config` and `-Ztarget-applies-to-host`
command-line options to be set, and that `target-applies-to-host =
false` is set in the Cargo configuration file.

```toml
# config.toml
[host]
linker = "/path/to/host/linker"
[host.x86_64-unknown-linux-gnu]
linker = "/path/to/host/arch/linker"
rustflags = ["-Clink-arg=--verbose"]
[target.x86_64-unknown-linux-gnu]
linker = "/path/to/target/linker"
```

The generic `host` table above will be entirely ignored when building on a
`x86_64-unknown-linux-gnu` host as the `host.x86_64-unknown-linux-gnu` table
takes precedence.

Setting `-Zhost-config` changes the default for `target-applies-to-host` to
`false` from `true`.

```console
cargo +nightly -Ztarget-applies-to-host -Zhost-config build --target x86_64-unknown-linux-gnu
```

## unit-graph
* Tracking Issue: [#8002](https://github.com/rust-lang/cargo/issues/8002)

The `--unit-graph` flag can be passed to any build command (`build`, `check`,
`run`, `test`, `bench`, `doc`, etc.) to emit a JSON object to stdout which
represents Cargo's internal unit graph. Nothing is actually built, and the
command returns immediately after printing. Each "unit" corresponds to an
execution of the compiler. These objects also include which unit each unit
depends on.

```
cargo +nightly build --unit-graph -Z unstable-options
```

This structure provides a more complete view of the dependency relationship as
Cargo sees it. In particular, the "features" field supports the new feature
resolver where a dependency can be built multiple times with different
features. `cargo metadata` fundamentally cannot represent the relationship of
features between different dependency kinds, and features now depend on which
command is run and which packages and targets are selected. Additionally it
can provide details about intra-package dependencies like build scripts or
tests.

The following is a description of the JSON structure:

```javascript
{
  /* Version of the JSON output structure. If any backwards incompatible
     changes are made, this value will be increased.
  */
  "version": 1,
  /* Array of all build units. */
  "units": [
    {
      /* An opaque string which indicates the package.
         Information about the package can be obtained from `cargo metadata`.
      */
      "pkg_id": "my-package 0.1.0 (path+file:///path/to/my-package)",
      /* The Cargo target. See the `cargo metadata` documentation for more
         information about these fields.
         https://doc.rust-lang.org/cargo/commands/cargo-metadata.html
      */
      "target": {
        "kind": ["lib"],
        "crate_types": ["lib"],
        "name": "my-package",
        "src_path": "/path/to/my-package/src/lib.rs",
        "edition": "2018",
        "test": true,
        "doctest": true
      },
      /* The profile settings for this unit.
         These values may not match the profile defined in the manifest.
         Units can use modified profile settings. For example, the "panic"
         setting can be overridden for tests to force it to "unwind".
      */
      "profile": {
        /* The profile name these settings are derived from. */
        "name": "dev",
        /* The optimization level as a string. */
        "opt_level": "0",
        /* The LTO setting as a string. */
        "lto": "false",
        /* The codegen units as an integer.
           `null` if it should use the compiler's default.
        */
        "codegen_units": null,
        /* The debug information level as an integer.
           `null` if it should use the compiler's default (0).
        */
        "debuginfo": 2,
        /* Whether or not debug-assertions are enabled. */
        "debug_assertions": true,
        /* Whether or not overflow-checks are enabled. */
        "overflow_checks": true,
        /* Whether or not rpath is enabled. */
        "rpath": false,
        /* Whether or not incremental is enabled. */
        "incremental": true,
        /* The panic strategy, "unwind" or "abort". */
        "panic": "unwind"
      },
      /* Which platform this target is being built for.
         A value of `null` indicates it is for the host.
         Otherwise it is a string of the target triple (such as
         "x86_64-unknown-linux-gnu").
      */
      "platform": null,
      /* The "mode" for this unit. Valid values:

         * "test" --- Build using `rustc` as a test.
         * "build" --- Build using `rustc`.
         * "check" --- Build using `rustc` in "check" mode.
         * "doc" --- Build using `rustdoc`.
         * "doctest" --- Test using `rustdoc`.
         * "run-custom-build" --- Represents the execution of a build script.
      */
      "mode": "build",
      /* Array of features enabled on this unit as strings. */
      "features": ["somefeat"],
      /* Whether or not this is a standard-library unit,
         part of the unstable build-std feature.
         If not set, treat as `false`.
      */
      "is_std": false,
      /* Array of dependencies of this unit. */
      "dependencies": [
        {
          /* Index in the "units" array for the dependency. */
          "index": 1,
          /* The name that this dependency will be referred as. */
          "extern_crate_name": "unicode_xid",
          /* Whether or not this dependency is "public",
             part of the unstable public-dependency feature.
             If not set, the public-dependency feature is not enabled.
          */
          "public": false,
          /* Whether or not this dependency is injected into the prelude,
             currently used by the build-std feature.
             If not set, treat as `false`.
          */
          "noprelude": false
        }
      ]
    },
    // ...
  ],
  /* Array of indices in the "units" array that are the "roots" of the
     dependency graph.
  */
  "roots": [0],
}
```

## Profile `rustflags` option
* Original Issue: [rust-lang/cargo#7878](https://github.com/rust-lang/cargo/issues/7878)
* Tracking Issue: [rust-lang/cargo#10271](https://github.com/rust-lang/cargo/issues/10271)

This feature provides a new option in the `[profile]` section to specify flags
that are passed directly to rustc.
This can be enabled like so:

```toml
cargo-features = ["profile-rustflags"]

[package]
# ...

[profile.release]
rustflags = [ "-C", "..." ]
```

To set this in a profile in Cargo configuration, you need to use either
`-Z profile-rustflags` or `[unstable]` table to enable it. For example,

```toml
# .cargo/config.toml
[unstable]
profile-rustflags = true

[profile.release]
rustflags = [ "-C", "..." ]
```

## rustdoc-map
* Tracking Issue: [#8296](https://github.com/rust-lang/cargo/issues/8296)

This feature adds configuration settings that are passed to `rustdoc` so that
it can generate links to dependencies whose documentation is hosted elsewhere
when the dependency is not documented. First, add this to `.cargo/config`:

```toml
[doc.extern-map.registries]
crates-io = "https://docs.rs/"
```

Then, when building documentation, use the following flags to cause links
to dependencies to link to [docs.rs](https://docs.rs/):

```
cargo +nightly doc --no-deps -Zrustdoc-map
```

The `registries` table contains a mapping of registry name to the URL to link
to. The URL may have the markers `{pkg_name}` and `{version}` which will get
replaced with the corresponding values. If neither are specified, then Cargo
defaults to appending `{pkg_name}/{version}/` to the end of the URL.

Another config setting is available to redirect standard library links. By
default, rustdoc creates links to <https://doc.rust-lang.org/nightly/>. To
change this behavior, use the `doc.extern-map.std` setting:

```toml
[doc.extern-map]
std = "local"
```

A value of `"local"` means to link to the documentation found in the `rustc`
sysroot. If you are using rustup, this documentation can be installed with
`rustup component add rust-docs`.

The default value is `"remote"`.

The value may also take a URL for a custom location.

## per-package-target
* Tracking Issue: [#9406](https://github.com/rust-lang/cargo/pull/9406)
* Original Pull Request: [#9030](https://github.com/rust-lang/cargo/pull/9030)
* Original Issue: [#7004](https://github.com/rust-lang/cargo/pull/7004)

The `per-package-target` feature adds two keys to the manifest:
`package.default-target` and `package.forced-target`. The first makes
the package be compiled by default (ie. when no `--target` argument is
passed) for some target. The second one makes the package always be
compiled for the target.

Example:

```toml
[package]
forced-target = "wasm32-unknown-unknown"
```

In this example, the crate is always built for
`wasm32-unknown-unknown`, for instance because it is going to be used
as a plugin for a main program that runs on the host (or provided on
the command line) target.

## artifact-dependencies

* Tracking Issue: [#9096](https://github.com/rust-lang/cargo/pull/9096)
* Original Pull Request: [#9992](https://github.com/rust-lang/cargo/pull/9992)

Artifact dependencies allow Cargo packages to depend on `bin`, `cdylib`, and `staticlib` crates,
and use the artifacts built by those crates at compile time.

Run `cargo` with `-Z bindeps` to enable this functionality.

### artifact-dependencies: Dependency declarations

Artifact-dependencies adds the following keys to a dependency declaration in `Cargo.toml`:

- `artifact` --- This specifies the [Cargo Target](cargo-targets.md) to build.
  Normally without this field, Cargo will only build the `[lib]` target from a dependency.
  This field allows specifying which target will be built, and made available as a binary at build time:

  * `"bin"` --- Compiled executable binaries, corresponding to all of the `[[bin]]` sections in the dependency's manifest.
  * `"bin:<bin-name>"` --- Compiled executable binary, corresponding to a specific binary target specified by the given `<bin-name>`.
  * `"cdylib"` --- A C-compatible dynamic library, corresponding to a `[lib]` section with `crate-type = ["cdylib"]` in the dependency's manifest.
  * `"staticlib"` --- A C-compatible static library, corresponding to a `[lib]` section with `crate-type = ["staticlib"]` in the dependency's manifest.

  The `artifact` value can be a string, or it can be an array of strings to specify multiple targets.

  Example:

  ```toml
  [dependencies]
  bar = { version = "1.0", artifact = "staticlib" }
  zoo = { version = "1.0", artifact = ["bin:cat", "bin:dog"]}
  ```

- `lib` --- This is a Boolean value which indicates whether or not to also build the dependency's library as a normal Rust `lib` dependency.
  This field can only be specified when `artifact` is specified.

  The default for this field is `false` when `artifact` is specified.
  If this is set to `true`, then the dependency's `[lib]` target will also be built for the platform target the declaring package is being built for.
  This allows the package to use the dependency from Rust code like a normal dependency in addition to an artifact dependency.

  Example:

  ```toml
  [dependencies]
  bar = { version = "1.0", artifact = "bin", lib = true }
  ```

- `target` --- The platform target to build the dependency for.
  This field can only be specified when `artifact` is specified.

  The default if this is not specified depends on the dependency kind.
  For build dependencies, it will be built for the host target.
  For all other dependencies, it will be built for the same targets the declaring package is built for.

  For a build dependency, this can also take the special value of `"target"` which means to build the dependency for the same targets that the package is being built for.

  ```toml
  [build-dependencies]
  bar = { version = "1.0", artifact = "cdylib", target = "wasm32-unknown-unknown"}
  same-target = { version = "1.0", artifact = "bin", target = "target" }
  ```

### artifact-dependencies: Environment variables

After building an artifact dependency, Cargo provides the following environment variables that you can use to access the artifact:

- `CARGO_<ARTIFACT-TYPE>_DIR_<DEP>` --- This is the directory containing all the artifacts from the dependency.

  `<ARTIFACT-TYPE>` is the `artifact` specified for the dependency (uppercased as in `CDYLIB`, `STATICLIB`, or `BIN`) and `<DEP>` is the name of the dependency.
  As with other Cargo environment variables, dependency names are converted to uppercase, with dashes replaced by underscores.

  If your manifest renames the dependency, `<DEP>` corresponds to the name you specify, not the original package name.

- `CARGO_<ARTIFACT-TYPE>_FILE_<DEP>_<NAME>` --- This is the full path to the artifact.

  `<ARTIFACT-TYPE>` is the `artifact` specified for the dependency (uppercased as above), `<DEP>` is the name of the dependency (transformed as above), and `<NAME>` is the name of the artifact from the dependency.

  Note that `<NAME>` is not modified in any way from the `name` specified in the crate supplying the artifact, or the crate name if not specified; for instance, it may be in lowercase, or contain dashes.

  For convenience, if the artifact name matches the original package name, cargo additionally supplies a copy of this variable with the `_<NAME>` suffix omitted.
  For instance, if the `cmake` crate supplies a binary named `cmake`, Cargo supplies both `CARGO_BIN_FILE_CMAKE` and `CARGO_BIN_FILE_CMAKE_cmake`.

For each kind of dependency, these variables are supplied to the same part of the build process that has access to that kind of dependency:

- For build-dependencies, these variables are supplied to the `build.rs` script, and can be accessed using [`std::env::var_os`](https://doc.rust-lang.org/std/env/fn.var_os.html).
  (As with any OS file path, these may or may not be valid UTF-8.)
- For normal dependencies, these variables are supplied during the compilation of the crate, and can be accessed using the [`env!`] macro.
- For dev-dependencies, these variables are supplied during the compilation of examples, tests, and benchmarks, and can be accessed using the [`env!`] macro.

[`env!`]: https://doc.rust-lang.org/std/macro.env.html

### artifact-dependencies: Examples

#### Example: use a binary executable from a build script

In the `Cargo.toml` file, you can specify a dependency on a binary to make available for a build script:

```toml
[build-dependencies]
some-build-tool = { version = "1.0", artifact = "bin" }
```

Then inside the build script, the binary can be executed at build time:

```rust
fn main() {
    let build_tool = std::env::var_os("CARGO_BIN_FILE_SOME_BUILD_TOOL").unwrap();
    let status = std::process::Command::new(build_tool)
        .arg("do-stuff")
        .status()
        .unwrap();
    if !status.success() {
        eprintln!("failed!");
        std::process::exit(1);
    }
}
```

#### Example: use _cdylib_ artifact in build script

The `Cargo.toml` in the consuming package, building the `bar` library as `cdylib`
for a specific build target…

```toml
[build-dependencies]
bar = { artifact = "cdylib", version = "1.0", target = "wasm32-unknown-unknown" }
```

…along with the build script in `build.rs`.

```rust
fn main() {
    wasm::run_file(std::env::var("CARGO_CDYLIB_FILE_BAR").unwrap());
}
```

#### Example: use _binary_ artifact and its library in a binary

The `Cargo.toml` in the consuming package, building the `bar` binary for inclusion
as artifact while making it available as library as well…

```toml
[dependencies]
bar = { artifact = "bin", version = "1.0", lib = true }
```

…along with the executable using `main.rs`.

```rust
fn main() {
    bar::init();
    command::run(env!("CARGO_BIN_FILE_BAR"));
}
```

## publish-timeout
* Tracking Issue: [11222](https://github.com/rust-lang/cargo/issues/11222)

The `publish.timeout` key in a config file can be used to control how long
`cargo publish` waits between posting a package to the registry and it being
available in the local index.

A timeout of `0` prevents any checks from occurring. The current default is
`60` seconds.

It requires the `-Zpublish-timeout` command-line options to be set.

```toml
# config.toml
[publish]
timeout = 300  # in seconds
```

## asymmetric-token
* Tracking Issue: [10519](https://github.com/rust-lang/cargo/issues/10519)
* RFC: [#3231](https://github.com/rust-lang/rfcs/pull/3231)

The `-Z asymmetric-token` flag enables the `cargo:paseto` credential provider which allows Cargo to authenticate to registries without sending secrets over the network.

In [`config.toml`](config.md) and `credentials.toml` files there is a field called `private-key`, which is a private key formatted in the secret [subset of `PASERK`](https://github.com/paseto-standard/paserk/blob/master/types/secret.md) and is used to sign asymmetric tokens

A keypair can be generated with `cargo login --generate-keypair` which will:
- generate a public/private keypair in the currently recommended fashion.
- save the private key in `credentials.toml`.
- print the public key in [PASERK public](https://github.com/paseto-standard/paserk/blob/master/types/public.md) format.

It is recommended that the `private-key` be saved in `credentials.toml`. It is also supported in `config.toml`, primarily so that it can be set using the associated environment variable, which is the recommended way to provide it in CI contexts. This setup is what we have for the `token` field for setting a secret token.

There is also an optional field called `private-key-subject` which is a string chosen by the registry.
This string will be included as part of an asymmetric token and should not be secret.
It is intended for the rare use cases like "cryptographic proof that the central CA server authorized this action". Cargo requires it to be non-whitespace printable ASCII. Registries that need non-ASCII data should base64 encode it.

Both fields can be set with `cargo login --registry=name --private-key --private-key-subject="subject"` which will prompt you to put in the key value.

A registry can have at most one of `private-key` or `token` set.

All PASETOs will include `iat`, the current time in ISO 8601 format. Cargo will include the following where appropriate:
- `sub` an optional, non-secret string chosen by the registry that is expected to be claimed with every request. The value will be the `private-key-subject` from the `config.toml` file.
- `mutation` if present, indicates that this request is a mutating operation (or a read-only operation if not present), must be one of the strings `publish`, `yank`, or `unyank`.
  - `name` name of the crate related to this request.
  - `vers` version string of the crate related to this request.
  - `cksum` the SHA256 hash of the crate contents, as a string of 64 lowercase hexadecimal digits, must be present only when `mutation` is equal to `publish`
- `challenge` the challenge string received from a 401/403 from this server this session. Registries that issue challenges must track which challenges have been issued/used and never accept a given challenge more than once within the same validity period (avoiding the need to track every challenge ever issued).

The "footer" (which is part of the signature) will be a JSON string in UTF-8 and include:
- `url` the RFC 3986 compliant URL where cargo got the config.json file,
  - If this is a registry with an HTTP index, then this is the base URL that all index queries are relative to.
  - If this is a registry with a GIT index, it is the URL Cargo used to clone the index.
- `kid` the identifier of the private key used to sign the request, using the [PASERK IDs](https://github.com/paseto-standard/paserk/blob/master/operations/ID.md) standard.

PASETO includes the message that was signed, so the server does not have to reconstruct the exact string from the request in order to check the signature. The server does need to check that the signature is valid for the string in the PASETO and that the contents of that string matches the request.
If a claim should be expected for the request but is missing in the PASETO then the request must be rejected.

## `cargo config`

* Original Issue: [#2362](https://github.com/rust-lang/cargo/issues/2362)
* Tracking Issue: [#9301](https://github.com/rust-lang/cargo/issues/9301)

The `cargo config` subcommand provides a way to display the configuration
files that cargo loads. It currently includes the `get` subcommand which
can take an optional config value to display.

```console
cargo +nightly -Zunstable-options config get build.rustflags
```

If no config value is included, it will display all config values. See the
`--help` output for more options available.

## rustc `--print`

* Tracking Issue: [#9357](https://github.com/rust-lang/cargo/issues/9357)

`cargo rustc --print=VAL` forwards the `--print` flag to `rustc` in order to
extract information from `rustc`. This runs `rustc` with the corresponding
[`--print`](https://doc.rust-lang.org/rustc/command-line-arguments.html#--print-print-compiler-information)
flag, and then immediately exits without compiling. Exposing this as a cargo
flag allows cargo to inject the correct target and RUSTFLAGS based on the
current configuration.

The primary use case is to run `cargo rustc --print=cfg` to get config values
for the appropriate target and influenced by any other RUSTFLAGS.


## Different binary name

* Tracking Issue: [#9778](https://github.com/rust-lang/cargo/issues/9778)
* PR: [#9627](https://github.com/rust-lang/cargo/pull/9627)

The `different-binary-name` feature allows setting the filename of the binary without having to obey the
restrictions placed on crate names. For example, the crate name must use only `alphanumeric` characters
or `-` or `_`, and cannot be empty.

The `filename` parameter should **not** include the binary extension, `cargo` will figure out the appropriate
extension and use that for the binary on its own.

The `filename` parameter is only available in the `[[bin]]` section of the manifest.

```toml
cargo-features = ["different-binary-name"]

[package]
name =  "foo"
version = "0.0.1"

[[bin]]
name = "foo"
filename = "007bar"
path = "src/main.rs"
```

## scrape-examples

* RFC: [#3123](https://github.com/rust-lang/rfcs/pull/3123)
* Tracking Issue: [#9910](https://github.com/rust-lang/cargo/issues/9910)

The `-Z rustdoc-scrape-examples` flag tells Rustdoc to search crates in the current workspace
for calls to functions. Those call-sites are then included as documentation. You can use the flag
like this:

```
cargo doc -Z unstable-options -Z rustdoc-scrape-examples
```

By default, Cargo will scrape examples from the example targets of packages being documented.
You can individually enable or disable targets from being scraped with the `doc-scrape-examples` flag, such as:

```toml
# Enable scraping examples from a library
[lib]
doc-scrape-examples = true

# Disable scraping examples from an example target
[[example]]
name = "my-example"
doc-scrape-examples = false
```

**Note on tests:** enabling `doc-scrape-examples` on test targets will not currently have any effect. Scraping
examples from tests is a work-in-progress.

**Note on dev-dependencies:** documenting a library does not normally require the crate's dev-dependencies. However,
example targets require dev-deps. For backwards compatibility, `-Z rustdoc-scrape-examples` will *not* introduce a
dev-deps requirement for `cargo doc`. Therefore examples will *not* be scraped from example targets under the
following conditions:

1. No target being documented requires dev-deps, AND
2. At least one crate with targets being documented has dev-deps, AND
3. The `doc-scrape-examples` parameter is unset or false for all `[[example]]` targets.

If you want examples to be scraped from example targets, then you must not satisfy one of the above conditions.
For example, you can set `doc-scrape-examples` to true for one example target, and that signals to Cargo that
you are ok with dev-deps being build for `cargo doc`.


## check-cfg

* RFC: [#3013](https://github.com/rust-lang/rfcs/pull/3013)
* Tracking Issue: [#10554](https://github.com/rust-lang/cargo/issues/10554)

`-Z check-cfg` command line enables compile time checking of Cargo features as well as `rustc`
well known names and values in `#[cfg]`, `cfg!`, `#[link]` and `#[cfg_attr]` with the `rustc`
and `rustdoc` unstable `--check-cfg` command line.

You can use the flag like this:

```
cargo check -Z unstable-options -Z check-cfg
```

### `cargo:rustc-check-cfg=CHECK_CFG`

The `rustc-check-cfg` instruction tells Cargo to pass the given value to the
`--check-cfg` flag to the compiler. This may be used for compile-time
detection of unexpected conditional compilation name and/or values.

This can only be used in combination with `-Zcheck-cfg` otherwise it is ignored
with a warning.

If you want to integrate with Cargo features, only use `-Zcheck-cfg` instead of
trying to do it manually with this option.

You can use the instruction like this:

```rust,no_run
// build.rs
println!("cargo:rustc-check-cfg=cfg(foo, bar)");
```

```
cargo check -Z unstable-options -Z check-cfg
```

## codegen-backend

The `codegen-backend` feature makes it possible to select the codegen backend used by rustc using a profile.

Example:

```toml
[package]
name = "foo"

[dependencies]
serde = "1.0.117"

[profile.dev.package.foo]
codegen-backend = "cranelift"
```

To set this in a profile in Cargo configuration, you need to use either
`-Z codegen-backend` or `[unstable]` table to enable it. For example,

```toml
# .cargo/config.toml
[unstable]
codegen-backend = true

[profile.dev.package.foo]
codegen-backend = "cranelift"
```

## gitoxide

* Tracking Issue: [#11813](https://github.com/rust-lang/cargo/issues/11813)

With the 'gitoxide' unstable feature, all or the specified git operations will be performed by 
the `gitoxide` crate instead of `git2`.

While `-Zgitoxide` enables all currently implemented features, one can individually select git operations
to run with `gitoxide` with the `-Zgitoxide=operation[,operationN]` syntax.

Valid operations are the following:

* `fetch` - All fetches are done with `gitoxide`, which includes git dependencies as well as the crates index.
* `shallow-index` - perform a shallow clone of the index.
* `shallow-deps` - perform a shallow clone of git dependencies.
* `checkout` *(planned)* - checkout the worktree, with support for filters and submodules.

**Details on shallow clones**

* To enable shallow clones, add `-Zgitoxide=fetch,shallow_deps` for fetching git dependencies or `-Zgitoxide=fetch,shallow_index` for fetching registry index.
* Shallow-cloned and shallow-checked-out git repositories reside at their own `-shallow` suffixed directories, i.e,
  - `~/.cargo/registry/index/*-shallow`
  - `~/.cargo/git/db/*-shallow`
  - `~/.cargo/git/checkouts/*-shallow`
* When the unstable feature is on, fetching/cloning a git repository is always a shallow fetch. This roughly equals to `git fetch --depth 1` everywhere.
* Even with the presence of `Cargo.lock` or specifying a commit `{ rev = "…" }`, gitoxide is still smart enough to shallow fetch without unshallowing the existing repository.

## script

* Tracking Issue: [#12207](https://github.com/rust-lang/cargo/issues/12207)

Cargo can directly run `.rs` files as:
```console
$ cargo +nightly -Zscript file.rs
```
where `file.rs` can be as simple as:
```rust
fn main() {}
```

A user may optionally specify a manifest in a `cargo` code fence in a module-level comment, like:
````rust
#!/usr/bin/env -S cargo +nightly -Zscript
```cargo
[dependencies]
clap = { version = "4.2", features = ["derive"] }
```

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[clap(short, long, help = "Path to config")]
    config: Option<std::path::PathBuf>,
}

fn main() {
    let args = Args::parse();
    println!("{:?}", args);
}
````

### Single-file packages

In addition to today's multi-file packages (`Cargo.toml` file with other `.rs`
files), we are adding the concept of single-file packages which may contain an
embedded manifest.  There is no required distinguishment for a single-file
`.rs` package from any other `.rs` file.

Single-file packages may be selected via `--manifest-path`, like
`cargo test --manifest-path foo.rs`. Unlike `Cargo.toml`, these files cannot be auto-discovered.

A single-file package may contain an embedded manifest.  An embedded manifest
is stored using `TOML` in rust "frontmatter", a markdown code-fence with `cargo`
at the start of the infostring at the top of the file.

Inferred / defaulted manifest fields:
- `package.name = <slugified file stem>`
- `package.version = "0.0.0"` to [call attention to this crate being used in unexpected places](https://matklad.github.io/2021/08/22/large-rust-workspaces.html#Smaller-Tips)
- `package.publish = false` to avoid accidental publishes, particularly if we
  later add support for including them in a workspace.
- `package.edition = <current>` to avoid always having to add an embedded
  manifest at the cost of potentially breaking scripts on rust upgrades
  - Warn when `edition` is unspecified to raise awareness of this

Disallowed manifest fields:
- `[workspace]`, `[lib]`, `[[bin]]`, `[[example]]`, `[[test]]`, `[[bench]]`
- `package.workspace`, `package.build`, `package.links`, `package.autobins`, `package.autoexamples`, `package.autotests`, `package.autobenches`

The default `CARGO_TARGET_DIR` for single-file packages is at `$CARGO_HOME/target/<hash>`:
- Avoid conflicts from multiple single-file packages being in the same directory
- Avoid problems with the single-file package's parent directory being read-only
- Avoid cluttering the user's directory

The lockfile for single-file packages will be placed in `CARGO_TARGET_DIR`.  In
the future, when workspaces are supported, that will allow a user to have a
persistent lockfile.

### Manifest-commands

You may pass a manifest directly to the `cargo` command, without a subcommand,
like `foo/Cargo.toml` or a single-file package like `foo.rs`.  This is mostly
intended for being put in `#!` lines.

The precedence for how to interpret `cargo <subcommand>` is
1. Built-in xor single-file packages
2. Aliases
3. External subcommands

A parameter is identified as a manifest-command if it has one of:
- Path separators
- A `.rs` extension
- The file name is `Cargo.toml`

Differences between `cargo run --manifest-path <path>` and `cargo <path>`
- `cargo <path>` runs with the config for `<path>` and not the current dir, more like `cargo install --path <path>`
- `cargo <path>` is at a verbosity level below the normal default.  Pass `-v` to get normal output.

### Documentation Updates

## Edition 2024
* Tracking Issue: (none created yet)
* RFC: [rust-lang/rfcs#3501](https://github.com/rust-lang/rfcs/pull/3501)

Support for the 2024 [edition] can be enabled by adding the `edition2024`
unstable feature to the top of `Cargo.toml`:

```toml
cargo-features = ["edition2024"]

[package]
name = "my-package"
version = "0.1.0"
edition = "2024"
```

If you want to transition an existing project from a previous edition, then
`cargo fix --edition` can be used on the nightly channel. After running `cargo
fix`, you can switch the edition to 2024 as illustrated above.

This feature is very unstable, and is only intended for early testing and
experimentation. Future nightly releases may introduce changes for the 2024
edition that may break your build.

[edition]: ../../edition-guide/index.html

# Stabilized and removed features

## Compile progress

The compile-progress feature has been stabilized in the 1.30 release.
Progress bars are now enabled by default.
See [`term.progress`](config.md#termprogresswhen) for more information about
controlling this feature.

## Edition

Specifying the `edition` in `Cargo.toml` has been stabilized in the 1.31 release.
See [the edition field](manifest.md#the-edition-field) for more information
about specifying this field.

## rename-dependency

Specifying renamed dependencies in `Cargo.toml` has been stabilized in the 1.31 release.
See [renaming dependencies](specifying-dependencies.md#renaming-dependencies-in-cargotoml)
for more information about renaming dependencies.

## Alternate Registries

Support for alternate registries has been stabilized in the 1.34 release.
See the [Registries chapter](registries.md) for more information about alternate registries.

## Offline Mode

The offline feature has been stabilized in the 1.36 release.
See the [`--offline` flag](../commands/cargo.md#option-cargo---offline) for
more information on using the offline mode.

## publish-lockfile

The `publish-lockfile` feature has been removed in the 1.37 release.
The `Cargo.lock` file is always included when a package is published if the
package contains a binary target. `cargo install` requires the `--locked` flag
to use the `Cargo.lock` file.
See [`cargo package`](../commands/cargo-package.md) and
[`cargo install`](../commands/cargo-install.md) for more information.

## default-run

The `default-run` feature has been stabilized in the 1.37 release.
See [the `default-run` field](manifest.md#the-default-run-field) for more
information about specifying the default target to run.

## cache-messages

Compiler message caching has been stabilized in the 1.40 release.
Compiler warnings are now cached by default and will be replayed automatically
when re-running Cargo.

## install-upgrade

The `install-upgrade` feature has been stabilized in the 1.41 release.
[`cargo install`] will now automatically upgrade packages if they appear to be
out-of-date. See the [`cargo install`] documentation for more information.

[`cargo install`]: ../commands/cargo-install.md

## Profile Overrides

Profile overrides have been stabilized in the 1.41 release.
See [Profile Overrides](profiles.md#overrides) for more information on using
overrides.

## Config Profiles

Specifying profiles in Cargo config files and environment variables has been
stabilized in the 1.43 release.
See the [config `[profile]` table](config.md#profile) for more information
about specifying [profiles](profiles.md) in config files.

## crate-versions

The `-Z crate-versions` flag has been stabilized in the 1.47 release.
The crate version is now automatically included in the
[`cargo doc`](../commands/cargo-doc.md) documentation sidebar.

## Features

The `-Z features` flag has been stabilized in the 1.51 release.
See [feature resolver version 2](features.md#feature-resolver-version-2)
for more information on using the new feature resolver.

## package-features

The `-Z package-features` flag has been stabilized in the 1.51 release.
See the [resolver version 2 command-line flags](features.md#resolver-version-2-command-line-flags)
for more information on using the features CLI options.

## Resolver

The `resolver` feature in `Cargo.toml` has been stabilized in the 1.51 release.
See the [resolver versions](resolver.md#resolver-versions) for more
information about specifying resolvers.

## extra-link-arg

The `extra-link-arg` feature to specify additional linker arguments in build
scripts has been stabilized in the 1.56 release. See the [build script
documentation](build-scripts.md#outputs-of-the-build-script) for more
information on specifying extra linker arguments.

## configurable-env

The `configurable-env` feature to specify environment variables in Cargo
configuration has been stabilized in the 1.56 release. See the [config
documentation](config.html#env) for more information about configuring
environment variables.

## rust-version

The `rust-version` field in `Cargo.toml` has been stabilized in the 1.56 release.
See the [rust-version field](manifest.html#the-rust-version-field) for more
information on using the `rust-version` field and the `--ignore-rust-version` option.

## patch-in-config

The `-Z patch-in-config` flag, and the corresponding support for
`[patch]` section in Cargo configuration files has been stabilized in
the 1.56 release. See the [patch field](config.html#patch) for more
information.

## edition 2021

The 2021 edition has been stabilized in the 1.56 release.
See the [`edition` field](manifest.md#the-edition-field) for more information on setting the edition.
See [`cargo fix --edition`](../commands/cargo-fix.md) and [The Edition Guide](../../edition-guide/index.html) for more information on migrating existing projects.


## Custom named profiles

Custom named profiles have been stabilized in the 1.57 release. See the
[profiles chapter](profiles.md#custom-profiles) for more information.

## Profile `strip` option

The profile `strip` option has been stabilized in the 1.59 release. See the
[profiles chapter](profiles.md#strip) for more information.

## Future incompat report

Support for generating a future-incompat report has been stabilized
in the 1.59 release. See the [future incompat report chapter](future-incompat-report.md)
for more information.

## Namespaced features

Namespaced features has been stabilized in the 1.60 release.
See the [Features chapter](features.md#optional-dependencies) for more information.

## Weak dependency features

Weak dependency features has been stabilized in the 1.60 release.
See the [Features chapter](features.md#dependency-features) for more information.

## timings

The `-Ztimings` option has been stabilized as `--timings` in the 1.60 release.
(`--timings=html` and the machine-readable `--timings=json` output remain
unstable and require `-Zunstable-options`.)

## config-cli

The `--config` CLI option has been stabilized in the 1.63 release. See
the [config documentation](config.html#command-line-overrides) for more
information.

## multitarget

The `-Z multitarget` option has been stabilized in the 1.64 release.
See [`build.target`](config.md#buildtarget) for more information about
setting the default [target platform triples][target triple].

## crate-type

The `--crate-type` flag for `cargo rustc` has been stabilized in the 1.64
release. See the [`cargo rustc` documentation](../commands/cargo-rustc.md)
for more information.


## Workspace Inheritance

Workspace Inheritance has been stabilized in the 1.64 release.
See [workspace.package](workspaces.md#the-package-table),
[workspace.dependencies](workspaces.md#the-dependencies-table),
and [inheriting-a-dependency-from-a-workspace](specifying-dependencies.md#inheriting-a-dependency-from-a-workspace)
for more information.

## terminal-width

The `-Z terminal-width` option has been stabilized in the 1.68 release.
The terminal width is always passed to the compiler when running from a
terminal where Cargo can automatically detect the width.

## sparse-registry

Sparse registry support has been stabilized in the 1.68 release.
See [Registry Protocols](registries.md#registry-protocols) for more information.

### `cargo logout`

The [`cargo logout`] command has been stabilized in the 1.70 release.

[target triple]: ../appendix/glossary.md#target '"target" (glossary)'
[`cargo logout`]: ../commands/cargo-logout.md

## `doctest-in-workspace`

The `-Z doctest-in-workspace` option for `cargo test` has been stabilized and
enabled by default in the 1.72 release. See the
[`cargo test` documentation](../commands/cargo-test.md#working-directory-of-tests)
for more information about the working directory for compiling and running tests.

## keep-going

The `--keep-going` option has been stabilized in the 1.74 release. See the
[`--keep-going` flag](../commands/cargo-build.html#option-cargo-build---keep-going)
in `cargo build` as an example for more details.

## `[lints]`

[`[lints]`](manifest.html#the-lints-section) (enabled via `-Zlints`) has been stabilized in the 1.74 release.

## credential-process

The `-Z credential-process` feature has been stabilized in the 1.74 release.

See [Registry Authentication](registry-authentication.md) documentation for details.

## registry-auth

The `-Z registry-auth` feature has been stabilized in the 1.74 release with the additional
requirement that a credential-provider is configured.

See [Registry Authentication](registry-authentication.md) documentation for details.
