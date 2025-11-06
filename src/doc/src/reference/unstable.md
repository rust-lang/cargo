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
  `--artifact-dir` option is only available on nightly:

  ```cargo +nightly build --artifact-dir=out -Z unstable-options```

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

*For the latest nightly, see the [nightly version] of this page.*

[config file]: config.md
[nightly channel]: ../../book/appendix-07-nightly-rust.html
[stabilized]: https://doc.crates.io/contrib/process/unstable.html#stabilization
[nightly version]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html

## List of unstable features

* Unstable-specific features
    * [-Z allow-features](#allow-features) --- Provides a way to restrict which unstable features are used.
* Build scripts and linking
    * [Metabuild](#metabuild) --- Provides declarative build scripts.
    * [Multiple Build Scripts](#multiple-build-scripts) --- Allows use of multiple build scripts.
* Resolver and features
    * [no-index-update](#no-index-update) --- Prevents cargo from updating the index cache.
    * [avoid-dev-deps](#avoid-dev-deps) --- Prevents the resolver from including dev-dependencies during resolution.
    * [minimal-versions](#minimal-versions) --- Forces the resolver to use the lowest compatible version instead of the highest.
    * [direct-minimal-versions](#direct-minimal-versions) — Forces the resolver to use the lowest compatible version instead of the highest.
    * [public-dependency](#public-dependency) --- Allows dependencies to be classified as either public or private.
    * [msrv-policy](#msrv-policy) --- MSRV-aware resolver and version selection
    * [precise-pre-release](#precise-pre-release) --- Allows pre-release versions to be selected with `update --precise`
    * [sbom](#sbom) --- Generates SBOM pre-cursor files for compiled artifacts
    * [update-breaking](#update-breaking) --- Allows upgrading to breaking versions with `update --breaking`
    * [feature-unification](#feature-unification) --- Enable new feature unification modes in workspaces
* Output behavior
    * [artifact-dir](#artifact-dir) --- Adds a directory where artifacts are copied to.
    * [build-dir-new-layout](#build-dir-new-layout) --- Enables the new build-dir filesystem layout
    * [Different binary name](#different-binary-name) --- Assign a name to the built binary that is separate from the crate name.
    * [root-dir](#root-dir) --- Controls the root directory relative to which paths are printed
* Compile behavior
    * [mtime-on-use](#mtime-on-use) --- Updates the last-modified timestamp on every dependency every time it is used, to provide a mechanism to delete unused artifacts.
    * [build-std](#build-std) --- Builds the standard library instead of using pre-built binaries.
    * [build-std-features](#build-std-features) --- Sets features to use with the standard library.
    * [binary-dep-depinfo](#binary-dep-depinfo) --- Causes the dep-info file to track binary dependencies.
    * [checksum-freshness](#checksum-freshness) --- When passed, the decision as to whether a crate needs to be rebuilt is made using file checksums instead of the file mtime.
    * [panic-abort-tests](#panic-abort-tests) --- Allows running tests with the "abort" panic strategy.
    * [host-config](#host-config) --- Allows setting `[target]`-like configuration settings for host build targets.
    * [no-embed-metadata](#no-embed-metadata) --- Passes `-Zembed-metadata=no` to the compiler, which avoid embedding metadata into rlib and dylib artifacts, to save disk space.
    * [target-applies-to-host](#target-applies-to-host) --- Alters whether certain flags will be passed to host build targets.
    * [gc](#gc) --- Global cache garbage collection.
    * [open-namespaces](#open-namespaces) --- Allow multiple packages to participate in the same API namespace
    * [panic-immediate-abort](#panic-immediate-abort) --- Passes `-Cpanic=immediate-abort` to the compiler.
    * [compile-time-deps](#compile-time-deps) --- Perma-unstable feature for rust-analyzer
* rustdoc
    * [rustdoc-map](#rustdoc-map) --- Provides mappings for documentation to link to external sites like [docs.rs](https://docs.rs/).
    * [scrape-examples](#scrape-examples) --- Shows examples within documentation.
    * [output-format](#output-format-for-rustdoc) --- Allows documentation to also be emitted in the experimental [JSON format](https://doc.rust-lang.org/nightly/nightly-rustc/rustdoc_json_types/).
    * [rustdoc-depinfo](#rustdoc-depinfo) --- Use dep-info files in rustdoc rebuild detection.
* `Cargo.toml` extensions
    * [Profile `rustflags` option](#profile-rustflags-option) --- Passed directly to rustc.
    * [Profile `hint-mostly-unused` option](#profile-hint-mostly-unused-option) --- Hint that a dependency is mostly unused, to optimize compilation time.
    * [codegen-backend](#codegen-backend) --- Select the codegen backend used by rustc.
    * [per-package-target](#per-package-target) --- Sets the `--target` to use for each individual package.
    * [artifact dependencies](#artifact-dependencies) --- Allow build artifacts to be included into other build artifacts and build them for different targets.
    * [Profile `trim-paths` option](#profile-trim-paths-option) --- Control the sanitization of file paths in build outputs.
    * [`[lints.cargo]`](#lintscargo) --- Allows configuring lints for Cargo.
    * [path bases](#path-bases) --- Named base directories for path dependencies.
    * [`unstable-editions`](#unstable-editions) --- Allows use of editions that are not yet stable.
* Information and metadata
    * [unit-graph](#unit-graph) --- Emits JSON for Cargo's internal graph structure.
    * [`cargo rustc --print`](#rustc---print) --- Calls rustc with `--print` to display information from rustc.
    * [Build analysis](#build-analysis) --- Record and persist detailed build metrics across runs, with new commands to query past builds.
* Configuration
    * [config-include](#config-include) --- Adds the ability for config files to include other files.
    * [`cargo config`](#cargo-config) --- Adds a new subcommand for viewing config files.
* Registries
    * [publish-timeout](#publish-timeout) --- Controls the timeout between uploading the crate and being available in the index
    * [asymmetric-token](#asymmetric-token) --- Adds support for authentication tokens using asymmetric cryptography (`cargo:paseto` provider).
* Other
    * [gitoxide](#gitoxide) --- Use `gitoxide` instead of `git2` for a set of operations.
    * [script](#script) --- Enable support for single-file `.rs` packages.
    * [lockfile-path](#lockfile-path) --- Allows to specify a path to lockfile other than the default path `<workspace_root>/Cargo.lock`.
    * [native-completions](#native-completions) --- Move cargo shell completions to native completions.
    * [warnings](#warnings) --- controls warning behavior; options for allowing or denying warnings.
    * [Package message format](#package-message-format) --- Message format for `cargo package`.
    * [`fix-edition`](#fix-edition) --- A permanently unstable edition migration helper.
    * [Plumbing subcommands](https://github.com/crate-ci/cargo-plumbing) --- Low, level commands that act as APIs for Cargo, like `cargo metadata`

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

## artifact-dir
* Original Issue: [#4875](https://github.com/rust-lang/cargo/issues/4875)
* Tracking Issue: [#6790](https://github.com/rust-lang/cargo/issues/6790)

This feature allows you to specify the directory where artifacts will be copied
to after they are built. Typically artifacts are only written to the
`target/release` or `target/debug` directories. However, determining the exact
filename can be tricky since you need to parse JSON output. The `--artifact-dir`
flag makes it easier to predictably access the artifacts. Note that the
artifacts are copied, so the originals are still in the `target` directory.
Example:

```sh
cargo +nightly build --artifact-dir=out -Z unstable-options
```

This can also be specified in `.cargo/config.toml` files.

```toml
[build]
artifact-dir = "out"
```

## root-dir
* Original Issue: [#9887](https://github.com/rust-lang/cargo/issues/9887)
* Tracking Issue: None (not currently slated for stabilization)

The `-Zroot-dir` flag sets the root directory relative to which paths are printed.
This affects both diagnostics and paths emitted by the `file!()` macro.

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

## Multiple Build Scripts
* Tracking Issue: [#14903](https://github.com/rust-lang/cargo/issues/14903)
* Original Pull Request: [#15630](https://github.com/rust-lang/cargo/pull/15630)

Multiple Build Scripts feature allows you to have multiple build scripts in your package.

Include `cargo-features` at the top of `Cargo.toml` and add `multiple-build-scripts` to enable feature.
Add the paths of the build scripts as an array in `package.build`. For example:

```toml
cargo-features = ["multiple-build-scripts"]

[package]
name = "mypackage"
version = "0.0.1"
build = ["foo.rs", "bar.rs"]
```

**Accessing Output Directories**:  Output directory of each build script can be accessed by using `<script-name>_OUT_DIR` 
  where the `<script-name>` is the file-stem of the build script, exactly as-is.
  For example, `bar_OUT_DIR` for script at `foo/bar.rs`. (Only set during compilation, can be accessed via `env!` macro)

## public-dependency
* Tracking Issue: [#44663](https://github.com/rust-lang/rust/issues/44663)

The 'public-dependency' feature allows marking dependencies as 'public'
or 'private'. When this feature is enabled, additional information is passed to rustc to allow
the [exported_private_dependencies](../../rustc/lints/listing/warn-by-default.html#exported-private-dependencies) lint to function properly.

To enable this feature, you can either use `-Zpublic-dependency`

```sh
cargo +nightly run -Zpublic-dependency
```

or `[unstable]` table, for example,

```toml
# .cargo/config.toml
[unstable]
public-dependency = true
```

`public-dependency` could also be enabled in `cargo-features`, **though this is deprecated and will be removed soon**.

```toml
cargo-features = ["public-dependency"]

[dependencies]
my_dep = { version = "1.2.3", public = true }
private_dep = "2.0.0" # Will be 'private' by default
```

Documentation updates:
- For workspace's "The `dependencies` table" section, include `public` as an unsupported field for `workspace.dependencies`

## msrv-policy
- [RFC: MSRV-aware Resolver](https://rust-lang.github.io/rfcs/3537-msrv-resolver.html)
- [#9930](https://github.com/rust-lang/cargo/issues/9930) (MSRV-aware resolver)

Catch-all unstable feature for MSRV-aware cargo features under
[RFC 2495](https://github.com/rust-lang/rfcs/pull/2495).

### MSRV-aware cargo add

This was stabilized in 1.79 in [#13608](https://github.com/rust-lang/cargo/pull/13608).

### MSRV-aware resolver

This was stabilized in 1.84 in [#14639](https://github.com/rust-lang/cargo/pull/14639).

### Convert `incompatible_toolchain` error into a lint

Unimplemented

### `--update-rust-version` flag for `cargo add`, `cargo update`

Unimplemented

### `package.rust-version = "toolchain"`

Unimplemented

### Update `cargo new` template to set `package.rust-version = "toolchain"`

Unimplemented

## precise-pre-release

* Tracking Issue: [#13290](https://github.com/rust-lang/cargo/issues/13290)
* RFC: [#3493](https://github.com/rust-lang/rfcs/pull/3493)

The `precise-pre-release` feature allows pre-release versions to be selected with `update --precise`
even when a pre-release is not specified by a projects `Cargo.toml`.

Take for example this `Cargo.toml`.

```toml
[dependencies]
my-dependency = "0.1.1"
```

It's possible to update `my-dependency` to a pre-release with `update -Zunstable-options my-dependency --precise 0.1.2-pre.0`.
This is because `0.1.2-pre.0` is considered compatible with `0.1.1`.
It would not be possible to upgrade to `0.2.0-pre.0` from `0.1.1` in the same way.

## sbom
* Tracking Issue: [#13709](https://github.com/rust-lang/cargo/pull/13709)
* RFC: [#3553](https://github.com/rust-lang/rfcs/pull/3553)

The `sbom` build config allows to generate so-called SBOM pre-cursor files
alongside each compiled artifact. A Software Bill Of Material (SBOM) tool can
incorporate these generated files to collect important information from the cargo
build process that are difficult or impossible to obtain in another way.

To enable this feature either set the `sbom` field in the `.cargo/config.toml`

```toml
[unstable]
sbom = true

[build]
sbom = true
```

or set the `CARGO_BUILD_SBOM` environment variable to `true`. The functionality
is available behind the flag `-Z sbom`.

The generated output files are in JSON format and follow the naming scheme
`<artifact>.cargo-sbom.json`. The JSON file contains information about dependencies,
target, features and the used `rustc` compiler.

SBOM pre-cursor files are generated for all executable and linkable outputs
that are uplifted into the target or artifact directories.

### Environment variables Cargo sets for crates

* `CARGO_SBOM_PATH` -- a list of generated SBOM precursor files, separated by the platform PATH separator. The list can be split with `std::env::split_paths`.

### SBOM pre-cursor schema

```json5
{
  // Schema version.
  "version": 1,
  // Index into the crates array for the root crate.
  "root": 0,
  // Array of all crates. There may be duplicates of the same crate if that
  // crate is compiled differently (different opt-level, features, etc).
  "crates": [
    {
      // Fully qualified package ID specification
      "id": "path+file:///sample-package#0.1.0",
      // List of target kinds: bin, lib, rlib, dylib, cdylib, staticlib, proc-macro, example, test, bench, custom-build
      "kind": ["bin"],
      // Enabled feature flags.
      "features": [],
      // Dependencies for this crate.
      "dependencies": [
        {
          // Index in to the crates array.
          "index": 1,
          // Dependency kind: 
          // Normal: A dependency linked to the artifact produced by this crate.
          // Build: A compile-time dependency used to build this crate (build-script or proc-macro).
          "kind": "normal"
        },
        {
          // A crate can depend on another crate with both normal and build edges.
          "index": 1,
          "kind": "build"
        }
      ]
    },
    {
      "id": "registry+https://github.com/rust-lang/crates.io-index#zerocopy@0.8.16",
      "kind": ["bin"],
      "features": [],
      "dependencies": []
    }
  ],
  // Information about rustc used to perform the compilation.
  "rustc": {
    // Compiler version
    "version": "1.86.0-nightly",
    // Compiler wrapper
    "wrapper": null,
    // Compiler workspace wrapper
    "workspace_wrapper": null,
    // Commit hash for rustc
    "commit_hash": "bef3c3b01f690de16738b1c9f36470fbfc6ac623",
    // Host target triple
    "host": "x86_64-pc-windows-msvc",
    // Verbose version string: `rustc -vV`
    "verbose_version": "rustc 1.86.0-nightly (bef3c3b01 2025-02-04)\nbinary: rustc\ncommit-hash: bef3c3b01f690de16738b1c9f36470fbfc6ac623\ncommit-date: 2025-02-04\nhost: x86_64-pc-windows-msvc\nrelease: 1.86.0-nightly\nLLVM version: 19.1.7\n"
  }
}
```

## update-breaking

* Tracking Issue: [#12425](https://github.com/rust-lang/cargo/issues/12425)

Allow upgrading dependencies version requirements in `Cargo.toml` across SemVer
incompatible versions using with the `--breaking` flag.

This only applies to dependencies when
- The package is a dependency of a workspace member
- The dependency is not renamed
- A SemVer-incompatible version is available
- The "SemVer operator" is used (`^` which is the default)

Users may further restrict which packages get upgraded by specifying them on
the command line.

Example:
```console
$ cargo +nightly -Zunstable-options update --breaking
$ cargo +nightly -Zunstable-options update --breaking clap
```

*This is meant to fill a similar role as [cargo-upgrade](https://github.com/killercup/cargo-edit/)*

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

## checksum-freshness
* Tracking issue: [#14136](https://github.com/rust-lang/cargo/issues/14136)

The `-Z checksum-freshness` flag will replace the use of file mtimes in cargo's
fingerprints with a file checksum value. This is most useful on systems with a poor
mtime implementation, or in CI/CD. The checksum algorithm can change without notice
between cargo versions. Fingerprints are used by cargo to determine when a crate needs to be rebuilt.

For the time being files ingested by build script will continue to use mtimes, even when `checksum-freshness`
is enabled. This is not intended as a long term solution.

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

The `include` key in a config file can be used to load another config file.
For example:

```toml
# .cargo/config.toml
include = "other-config.toml"

[build]
jobs = 4
```

```toml
# .cargo/other-config.toml
[build]
rustflags = ["-W", "unsafe-code"]
```

### Documentation updates

#### `include`

* Type: string, array of strings, or array of tables
* Default: none

Loads additional config files. Paths are relative to the config file that
includes them. Only paths ending with `.toml` are accepted.

Supports the following formats:

```toml
# single path
include = "path/to/mordor.toml"

# array of paths
include = ["frodo.toml", "samwise.toml"]

# inline tables
include = [
    "simple.toml",
    { path = "optional.toml", optional = true }
]

# array of tables
[[include]]
path = "required.toml"

[[include]]
path = "optional.toml"
optional = true
```

When using table syntax (inline tables or array of tables), the following
fields are supported:

* `path` (string, required): Path to the config file to include.
* `optional` (boolean, default: false): If `true`, missing files are silently
  skipped instead of causing an error.

The merge behavior of `include` is different from other config values:

1. Config values are first loaded from the `include` path.
    * If `include` is an array, config values are loaded and merged from left
      to right for each path.
    * This step recurses if included config files also contain `include` keys.
2. Then, the config file's own values are merged on top of the included config.

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

The `host` key in a config file can be used to pass flags to host build targets
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

The generic `host` table above will be entirely ignored when building on an
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
        "name": "my_package",
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

## Profile `hint-mostly-unused` option
* Tracking Issue: [#15644](https://github.com/rust-lang/cargo/issues/15644)

This feature provides a new option in the `[profile]` section to enable the
rustc `hint-mostly-unused` option. This is primarily useful to enable for
specific dependencies:

```toml
[profile.dev.package.huge-mostly-unused-dependency]
hint-mostly-unused = true
```

To enable this feature, pass `-Zprofile-hint-mostly-unused`. However, since
this option is a hint, using it without passing `-Zprofile-hint-mostly-unused`
will only warn and ignore the profile option. Versions of Cargo prior to the
introduction of this feature will give an "unused manifest key" warning, but
will otherwise function without erroring. This allows using the hint in a
crate's `Cargo.toml` without mandating the use of a newer Cargo to build it.

A crate can also provide this hint automatically for crates that depend on it,
using the `[hints]` table (which will likewise be ignored by older Cargo):

```toml
[hints]
mostly-unused = true
```

This will cause the crate to default to hint-mostly-unused, unless overridden
via `profile`, which takes precedence, and which can only be specified in the
top-level crate being built.

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

## output-format for rustdoc

* Tracking Issue: [#13283](https://github.com/rust-lang/cargo/issues/13283)

This flag determines the output format of `cargo rustdoc`, accepting `html` or `json`, providing tools with a way to lean on [rustdoc's experimental JSON format](https://doc.rust-lang.org/nightly/nightly-rustc/rustdoc_json_types/).

You can use the flag like this:

```
cargo rustdoc -Z unstable-options --output-format json
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
* `checkout` *(planned)* - checkout the worktree, with support for filters and submodules.

## git

* Tracking Issue: [#13285](https://github.com/rust-lang/cargo/issues/13285)

With the 'git' unstable feature, both `gitoxide` and `git2` will perform shallow fetches of the crate
index and git dependencies.

While `-Zgit` enables all currently implemented features, one can individually select when to perform
shallow fetches with the `-Zgit=operation[,operationN]` syntax.

Valid operations are the following:

* `shallow-index` - perform a shallow clone of the index.
* `shallow-deps` - perform a shallow clone of git dependencies.

**Details on shallow clones**

* To enable shallow clones, add `-Zgit=shallow-deps` for fetching git dependencies or `-Zgit=shallow-index` for fetching registry index.
* Shallow-cloned and shallow-checked-out git repositories reside at their own `-shallow` suffixed directories, i.e,
  - `~/.cargo/registry/index/*-shallow`
  - `~/.cargo/git/db/*-shallow`
  - `~/.cargo/git/checkouts/*-shallow`
* When the unstable feature is on, fetching/cloning a git repository is always a shallow fetch. This roughly equals to `git fetch --depth 1` everywhere.
* Even with the presence of `Cargo.lock` or specifying a commit `{ rev = "…" }`, gitoxide and libgit2 are still smart enough to shallow fetch without unshallowing the existing repository.

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
---cargo
[dependencies]
clap = { version = "4.2", features = ["derive"] }
---

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
- `package.edition = <current>` to avoid always having to add an embedded
  manifest at the cost of potentially breaking scripts on rust upgrades
  - Warn when `edition` is unspecified to raise awareness of this

Disallowed manifest fields:
- `[workspace]`, `[lib]`, `[[bin]]`, `[[example]]`, `[[test]]`, `[[bench]]`
- `package.workspace`, `package.build`, `package.links`, `package.autolib`, `package.autobins`, `package.autoexamples`, `package.autotests`, `package.autobenches`

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

When running a package with an embedded manifest,
[`arg0`](https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.arg0) will be the scripts path.
To get the executable's path, see [`current_exe`](https://doc.rust-lang.org/std/env/fn.current_exe.html).

### Documentation Updates

## Profile `trim-paths` option

* Tracking Issue: [rust-lang/cargo#12137](https://github.com/rust-lang/cargo/issues/12137)
* Tracking Rustc Issue: [rust-lang/rust#111540](https://github.com/rust-lang/rust/issues/111540)

This adds a new profile setting to control how paths are sanitized in the resulting binary.
This can be enabled like so:

```toml
cargo-features = ["trim-paths"]

[package]
# ...

[profile.release]
trim-paths = ["diagnostics", "object"]
```

To set this in a profile in Cargo configuration,
you need to use either `-Z trim-paths` or `[unstable]` table to enable it.
For example,

```toml
# .cargo/config.toml
[unstable]
trim-paths = true

[profile.release]
trim-paths = ["diagnostics", "object"]
```

### Documentation updates

#### trim-paths

*as a new ["Profiles settings" entry](./profiles.html#profile-settings)*

`trim-paths` is a profile setting which enables and controls the sanitization of file paths in build outputs.
It takes the following values:

- `"none"` and `false` --- disable path sanitization
- `"macro"` --- sanitize paths in the expansion of `std::file!()` macro.
    This is where paths in embedded panic messages come from
- `"diagnostics"` --- sanitize paths in printed compiler diagnostics
- `"object"` --- sanitize paths in compiled executables or libraries
- `"all"` and `true` --- sanitize paths in all possible locations

It also takes an array with the combinations of `"macro"`, `"diagnostics"`, and `"object"`.

It is defaulted to `none` for the `dev` profile, and `object` for the `release` profile.
You can manually override it by specifying this option in `Cargo.toml`:

```toml
[profile.dev]
trim-paths = "all"

[profile.release]
trim-paths = ["object", "diagnostics"]
```

The default `release` profile setting (`object`) sanitizes only the paths in emitted executable or library files.
It always affects paths from macros such as panic messages, and in debug information only if they will be embedded together with the binary
(the default on platforms with ELF binaries, such as Linux and windows-gnu),
but will not touch them if they are in separate files (the default on Windows MSVC and macOS).
But the paths to these separate files are sanitized.

If `trim-paths` is not `none` or `false`, then the following paths are sanitized if they appear in a selected scope:

1. Path to the source files of the standard and core library (sysroot) will begin with `/rustc/[rustc commit hash]`,
   e.g. `/home/username/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/result.rs` ->
   `/rustc/fe72845f7bb6a77b9e671e6a4f32fe714962cec4/library/core/src/result.rs`
2. Path to the current package will be stripped, relatively to the current workspace root, e.g. `/home/username/crate/src/lib.rs` -> `src/lib.rs`.
3. Path to dependency packages will be replaced with `[package name]-[version]`. E.g. `/home/username/deps/foo/src/lib.rs` -> `foo-0.1.0/src/lib.rs`

When a path to the source files of the standard and core library is *not* in scope for sanitization,
the emitted path will depend on if `rust-src` component is present.
If it is, then some paths will point to the copy of the source files on your file system;
if it isn't, then they will show up as `/rustc/[rustc commit hash]/library/...`
(just like when it is selected for sanitization).
Paths to all other source files will not be affected.

This will not affect any hard-coded paths in the source code, such as in strings.

#### Environment variable

*as a new entry of ["Environment variables Cargo sets for build scripts"](./environment-variables.md#environment-variables-cargo-sets-for-crates)*

* `CARGO_TRIM_PATHS` --- The value of `trim-paths` profile option.
    `false`, `"none"`, and empty arrays would be converted to `none`.
    `true` and `"all"` become `all`.
    Values in a non-empty array would be joined into a comma-separated list.
    If the build script introduces absolute paths to built artifacts (such as by invoking a compiler),
    the user may request them to be sanitized in different types of artifacts.
    Common paths requiring sanitization include `OUT_DIR`, `CARGO_MANIFEST_DIR` and `CARGO_MANIFEST_PATH`,
    plus any other introduced by the build script, such as include directories.

## gc

* Tracking Issue: [#12633](https://github.com/rust-lang/cargo/issues/12633)

The `-Zgc` flag is used to enable certain features related to garbage-collection of cargo's global cache within the cargo home directory.

#### Automatic gc configuration

The `-Zgc` flag will enable Cargo to read extra configuration options related to garbage collection.
The settings available are:

```toml
# Example config.toml file.

# Sub-table for defining specific settings for cleaning the global cache.
[cache.global-clean]
# Anything older than this duration will be deleted in the source cache.
max-src-age = "1 month"
# Anything older than this duration will be deleted in the compressed crate cache.
max-crate-age = "3 months"
# Any index older than this duration will be deleted from the index cache.
max-index-age = "3 months"
# Any git checkout older than this duration will be deleted from the checkout cache.
max-git-co-age = "1 month"
# Any git clone older than this duration will be deleted from the git cache.
max-git-db-age = "3 months"
```

Note that the [`cache.auto-clean-frequency`] option was stabilized in Rust 1.88.

[`cache.auto-clean-frequency`]: config.md#cacheauto-clean-frequency

### Manual garbage collection with `cargo clean`

Manual deletion can be done with the `cargo clean gc -Zgc` command.
Deletion of cache contents can be performed by passing one of the cache options:

- `--max-src-age=DURATION` --- Deletes source cache files that have not been used since the given age.
- `--max-crate-age=DURATION` --- Deletes crate cache files that have not been used since the given age.
- `--max-index-age=DURATION` --- Deletes registry indexes that have not been used since then given age (including their `.crate` and `src` files).
- `--max-git-co-age=DURATION` --- Deletes git dependency checkouts that have not been used since then given age.
- `--max-git-db-age=DURATION` --- Deletes git dependency clones that have not been used since then given age.
- `--max-download-age=DURATION` --- Deletes any downloaded cache data that has not been used since then given age.
- `--max-src-size=SIZE` --- Deletes the oldest source cache files until the cache is under the given size.
- `--max-crate-size=SIZE` --- Deletes the oldest crate cache files until the cache is under the given size.
- `--max-git-size=SIZE` --- Deletes the oldest git dependency caches until the cache is under the given size.
- `--max-download-size=SIZE` --- Deletes the oldest downloaded cache data until the cache is under the given size.

A DURATION is specified in the form "N seconds/minutes/days/weeks/months" where N is an integer.

A SIZE is specified in the form "N *suffix*" where *suffix* is B, kB, MB, GB, kiB, MiB, or GiB, and N is an integer or floating point number. If no suffix is specified, the number is the number of bytes.

```sh
cargo clean gc -Zgc
cargo clean gc -Zgc --max-download-age=1week
cargo clean gc -Zgc --max-git-size=0 --max-download-size=100MB
```

## open-namespaces

* Tracking Issue: [#13576](https://github.com/rust-lang/cargo/issues/13576)

Allow multiple packages to participate in the same API namespace

This can be enabled like so:
```toml
cargo-features = ["open-namespaces"]

[package]
# ...
```

## panic-immediate-abort

* Tracking Issue: [#16042](https://github.com/rust-lang/cargo/issues/16042)
* Upstream Tracking Issue: [rust-lang/rust#147286](https://github.com/rust-lang/rust/issues/147286)

Extends the `panic` profile setting to support the
[`immediate-abort`](../../rustc/codegen-options/index.html#panic) panic strategy.
This can be enabled like so:

```toml
# Cargo.toml
cargo-features = ["panic-immediate-abort"]

[package]
# ...

[profile.release]
panic = "immediate-abort"
```

To set this in a profile in Cargo configuration,
you need to use either `-Z panic-immediate-abort` CLI flag
or the `[unstable]` table to enable it.
For example,

```toml
# .cargo/config.toml
[unstable]
panic-immediate-abort = true

[profile.release]
panic = "immediate-abort"
```

## `[lints.cargo]`

* Tracking Issue: [#12235](https://github.com/rust-lang/cargo/issues/12235)

A new `lints` tool table for `cargo` that can be used to configure lints emitted
by `cargo` itself when `-Zcargo-lints` is used
```toml
[lints.cargo]
implicit-features = "warn"
```

This will work with
[RFC 2906 `workspace-deduplicate`](https://rust-lang.github.io/rfcs/2906-cargo-workspace-deduplicate.html):
```toml
[workspace.lints.cargo]
implicit-features = "warn"

[lints]
workspace = true
```

## Path Bases

* Tracking Issue: [#14355](https://github.com/rust-lang/cargo/issues/14355)

A `path` dependency may optionally specify a base by setting the `base` key to
the name of a path base from the `[path-bases]` table in either the
[configuration](config.md) or one of the [built-in path bases](#built-in-path-bases).
The value of that path base is prepended to the `path` value (along with a path
separator if necessary) to produce the actual location where Cargo will look for
the dependency.

For example, if the `Cargo.toml` contains:

```toml
cargo-features = ["path-bases"]

[dependencies]
foo = { base = "dev", path = "foo" }
```

Given a `[path-bases]` table in the configuration that contains:

```toml
[path-bases]
dev = "/home/user/dev/rust/libraries/"
```

This will produce a `path` dependency `foo` located at
`/home/user/dev/rust/libraries/foo`.

Path bases can be either absolute or relative. Relative path bases are relative
to the parent directory of the configuration file that declared that path base.

The name of a path base must use only [alphanumeric](https://doc.rust-lang.org/std/primitive.char.html#method.is_alphanumeric)
characters or `-` or `_`, must start with an [alphabetic](https://doc.rust-lang.org/std/primitive.char.html#method.is_alphabetic)
character, and must not be empty.

If the name of path base used in a dependency is neither in the configuration
nor one of the built-in path base, then Cargo will raise an error.

#### Built-in path bases

Cargo provides implicit path bases that can be used without the need to specify
them in a `[path-bases]` table.

* `workspace` - If a project is [a workspace or workspace member](workspaces.md)
then this path base is defined as the parent directory of the root `Cargo.toml`
of the workspace.

If a built-in path base name is also declared in the configuration, then Cargo
will prefer the value in the configuration. The allows Cargo to add new built-in
path bases without compatibility issues (as existing uses will shadow the
built-in name).

## lockfile-path
* Original Issue: [#5707](https://github.com/rust-lang/cargo/issues/5707)
* Tracking Issue: [#14421](https://github.com/rust-lang/cargo/issues/14421)

This feature allows you to specify the path of lockfile Cargo.lock. 
By default, lockfile is written into `<workspace_root>/Cargo.lock`. 
However, when sources are stored in read-only directory, most of the cargo commands 
would fail, trying to write a lockfile. The `--lockfile-path`
flag makes it easier to work with readonly sources. 
Note, that currently path must end with `Cargo.lock`. Meaning, if you want to use 
this feature in multiple projects, lockfiles should be stored in different directories.
Example:

```sh
cargo +nightly metadata --lockfile-path=$LOCKFILES_ROOT/my-project/Cargo.lock -Z unstable-options
```

## native-completions
* Original Issue: [#6645](https://github.com/rust-lang/cargo/issues/6645)
* Tracking Issue: [#14520](https://github.com/rust-lang/cargo/issues/14520)

This feature moves the handwritten completion scripts to Rust native, making it
easier for us to add, extend and test new completions. This feature is enabled with the
nightly channel, without requiring additional `-Z` options.

Areas of particular interest for feedback
- Arguments that need escaping or quoting that aren't handled correctly
- Inaccuracies in the information
- Bugs in parsing of the command-line
- Arguments that don't report their completions
- If a known issue is being problematic

Feedback can be broken down into
- What completion candidates are reported
  - Known issues: [#14520](https://github.com/rust-lang/cargo/issues/14520), [`A-completions`](https://github.com/rust-lang/cargo/labels/A-completions)
  - [Report an issue](https://github.com/rust-lang/cargo/issues/new) or [discuss the behavior](https://github.com/rust-lang/cargo/issues/14520)
- Shell integration, command-line parsing, and completion filtering
  - Known issues: [clap#3166](https://github.com/clap-rs/clap/issues/3166), [clap's `A-completions`](https://github.com/clap-rs/clap/labels/A-completion)
  - [Report an issue](https://github.com/clap-rs/clap/issues/new/choose) or [discuss the behavior](https://github.com/clap-rs/clap/discussions/new/choose)

When in doubt, you can discuss this in [#14520](https://github.com/rust-lang/cargo/issues/14520) or on [zulip](https://rust-lang.zulipchat.com/#narrow/stream/246057-t-cargo)

### How to use native-completions feature:
- bash:
  Add `source <(CARGO_COMPLETE=bash cargo +nightly)` to `~/.local/share/bash-completion/completions/cargo`.

- zsh:
  Add `source <(CARGO_COMPLETE=zsh cargo +nightly)` to your `.zshrc`.
  
- fish:
  Add `source (CARGO_COMPLETE=fish cargo +nightly | psub)` to `$XDG_CONFIG_HOME/fish/completions/cargo.fish`

- elvish:
  Add `eval (E:CARGO_COMPLETE=elvish cargo +nightly | slurp)` to `$XDG_CONFIG_HOME/elvish/rc.elv`

- powershell:
  Add `CARGO_COMPLETE=powershell cargo +nightly | Invoke-Expression` to `$PROFILE`.

## warnings

* Original Issue: [#8424](https://github.com/rust-lang/cargo/issues/8424)
* Tracking Issue: [#14802](https://github.com/rust-lang/cargo/issues/14802)

The `-Z warnings` feature enables the `build.warnings` configuration option to control how
Cargo handles warnings. If the `-Z warnings` unstable flag is not enabled, then
the `build.warnings` config will be ignored.

This setting currently only applies to rustc warnings. It may apply to additional warnings (such as Cargo lints or Cargo warnings)
in the future.

### `build.warnings`
* Type: string
* Default: `warn`
* Environment: `CARGO_BUILD_WARNINGS`

Controls how Cargo handles warnings. Allowed values are:
* `warn`: warnings are emitted as warnings (default).
* `allow`: warnings are hidden.
* `deny`: if warnings are emitted, an error will be raised at the end of the operation and the process will exit with a failure exit code. 

## feature unification

* RFC: [#3692](https://github.com/rust-lang/rfcs/blob/master/text/3692-feature-unification.md)
* Tracking Issue: [#14774](https://github.com/rust-lang/cargo/issues/14774)

The `-Z feature-unification` enables the `resolver.feature-unification`
configuration option to control how features are unified across a workspace.
If the `-Z feature-unification` unstable flag is not enabled,
then the `resolver.feature-unification` configuration will be ignored.

### `resolver.feature-unification`

* Type: string
* Default: `"selected"`
* Environment: `CARGO_RESOLVER_FEATURE_UNIFICATION`

Specify which packages participate in [feature unification](../reference/features.html#feature-unification).

* `selected`: Merge dependency features from all packages specified for the current build.
* `workspace`: Merge dependency features across all workspace members,
  regardless of which packages are specified for the current build.
* `package`: Dependency features are considered on a package-by-package basis,
  preferring duplicate builds of dependencies when different sets of features are activated by the packages.

## Package message format

* Original Issue: [#11666](https://github.com/rust-lang/cargo/issues/11666)
* Tracking Issue: [#15353](https://github.com/rust-lang/cargo/issues/15353)

The `--message-format` flag in `cargo package` controls the output message format.
Currently, it only works with the `--list` flag and affects the file listing format,
Requires `-Zunstable-options`.
See [`cargo package --message-format`](../commands/cargo-package.md#option-cargo-package---message-format)
for more information.

## rustdoc depinfo

* Original Issue: [#12266](https://github.com/rust-lang/cargo/issues/12266)
* Tracking Issue: [#15370](https://github.com/rust-lang/cargo/issues/15370)

The `-Z rustdoc-depinfo` flag leverages rustdoc's dep-info files to determine
whether documentations are required to re-generate. This can be combined with
`-Z checksum-freshness` to detect checksum changes rather than file mtime.

## no-embed-metadata
* Original Pull Request: [#15378](https://github.com/rust-lang/cargo/pull/15378)
* Tracking Issue: [#15495](https://github.com/rust-lang/cargo/issues/15495)

The default behavior of Rust is to embed crate metadata into `rlib` and `dylib` artifacts.
Since Cargo also passes `--emit=metadata` to these intermediate artifacts to enable pipelined
compilation, this means that a lot of metadata ends up being duplicated on disk, which wastes
disk space in the target directory.

This feature tells Cargo to pass the `-Zembed-metadata=no` flag to the compiler, which instructs
it not to embed metadata within rlib and dylib artifacts. In this case, the metadata will only
be stored in `.rmeta` files.

```console
cargo +nightly -Zno-embed-metadata build
```

## `unstable-editions`

The `unstable-editions` value in the `cargo-features` list allows a `Cargo.toml` manifest to specify an edition that is not yet stable.

```toml
cargo-features = ["unstable-editions"]

[package]
name = "my-package"
edition = "future"
```

When new editions are introduced, the `unstable-editions` feature is required until the edition is stabilized.

The special "future" edition is a home for new features that are under development, and is permanently unstable. The "future" edition also has no new behavior by itself. Each change in the future edition requires an opt-in such as a `#![feature(...)]` attribute.

## `fix-edition`

`-Zfix-edition` is a permanently unstable flag to assist with testing edition migrations, particularly with the use of crater. It only works with the `cargo fix` subcommand. It takes two different forms:

- `-Zfix-edition=start=$INITIAL` --- This form checks if the current edition is equal to the given number. If not, it exits with success (because we want to ignore older editions). If it is, then it runs the equivalent of `cargo check`. This is intended to be used with crater's "start" toolchain to set a baseline for the "before" toolchain.
- `-Zfix-edition=end=$INITIAL,$NEXT` --- This form checks if the current edition is equal to the given `$INITIAL` value. If not, it exits with success. If it is, then it performs an edition migration to the edition specified in `$NEXT`. Afterwards, it will modify `Cargo.toml` to add the appropriate `cargo-features = ["unstable-edition"]`, update the `edition` field, and run the equivalent of `cargo check` to verify that the migration works on the new edition.

For example:

```console
cargo +nightly fix -Zfix-edition=end=2024,future
```

## section-timings
* Original Pull Request: [#15780](https://github.com/rust-lang/cargo/pull/15780)
* Tracking Issue: [#15817](https://github.com/rust-lang/cargo/issues/15817)

This feature can be used to extend the output of `cargo build --timings`. It will tell rustc
to produce timings of individual compilation sections, which will be then displayed in the timings
HTML/JSON output.

```console
cargo +nightly -Zsection-timings build --timings
```

## Build analysis

* Original Issue: [rust-lang/rust-project-goals#332](https://github.com/rust-lang/rust-project-goals/pull/332)
* Tracking Issue: [#15844](https://github.com/rust-lang/cargo/issues/15844)

The `-Zbuild-analysis` feature records and persists detailed build metrics
(timings, rebuild reasons, etc.) across runs, with new commands to query past builds.

```toml
# Example config.toml file.

# Enable the build metric collection
[build.analysis]
enabled = true
```

## build-dir-new-layout

* Tracking Issue: [#15010](https://github.com/rust-lang/cargo/issues/15010)

Enables the new build-dir filesystem layout.
This layout change unblocks work towards caching and locking improvements.


## compile-time-deps

This permanently-unstable flag to only build proc-macros and build scripts (and their required dependencies),
as well as run the build scripts.

It is intended for use by tools like rust-analyzer and will never be stabilized.

Example:

```console
cargo +nightly build --compile-time-deps -Z unstable-options
cargo +nightly check --compile-time-deps --all-targets -Z unstable-options
```

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

## check-cfg

The `-Z check-cfg` feature has been stabilized in the 1.80 release by making it the
default behavior.

See the [build script documentation](build-scripts.md#rustc-check-cfg) for information
about specifying custom cfgs.

## Edition 2024

The 2024 edition has been stabilized in the 1.85 release.
See the [`edition` field](manifest.md#the-edition-field) for more information on setting the edition.
See [`cargo fix --edition`](../commands/cargo-fix.md) and [The Edition Guide](../../edition-guide/index.html) for more information on migrating existing projects.

## Automatic garbage collection

Support for automatically deleting old files was stabilized in Rust 1.88.
More information can be found in the [config chapter](config.md#cache).

## doctest-xcompile

Doctest cross-compiling is now unconditionally enabled starting in Rust 1.89. Running doctests with `cargo test` will now honor the `--target` flag.

## package-workspace

Multi-package publishing has been stabilized in Rust 1.90.0.

## build-dir

Support for `build.build-dir` was stabilized in the 1.91 release.
See the [config documentation](config.md#buildbuild-dir) for information about changing the build-dir

## Build-plan

The `--build-plan` argument for the `build` command has been removed in 1.93.0-nightly.
See <https://github.com/rust-lang/cargo/issues/7614> for the reason for its removal.
