## Unstable Features

Experimental Cargo features are only available on the nightly channel. You
typically use one of the `-Z` flags to enable them. Run `cargo -Z help` to
see a list of flags available.

`-Z unstable-options` is a generic flag for enabling other unstable
command-line flags. Options requiring this will be called out below.

Some unstable features will require you to specify the `cargo-features` key in
`Cargo.toml`.

### no-index-update
* Original Issue: [#3479](https://github.com/rust-lang/cargo/issues/3479)
* Tracking Issue: [#7404](https://github.com/rust-lang/cargo/issues/7404)

The `-Z no-index-update` flag ensures that Cargo does not attempt to update
the registry index. This is intended for tools such as Crater that issue many
Cargo commands, and you want to avoid the network latency for updating the
index each time.

### mtime-on-use
* Original Issue: [#6477](https://github.com/rust-lang/cargo/pull/6477)
* Cache usage meta tracking issue: [#7150](https://github.com/rust-lang/cargo/issues/7150)

The `-Z mtime-on-use` flag is an experiment to have Cargo update the mtime of
used files to make it easier for tools like cargo-sweep to detect which files
are stale. For many workflows this needs to be set on *all* invocations of cargo.
To make this more practical setting the `unstable.mtime_on_use` flag in `.cargo/config`
or the corresponding ENV variable will apply the `-Z mtime-on-use` to all
invocations of nightly cargo. (the config flag is ignored by stable)

### avoid-dev-deps
* Original Issue: [#4988](https://github.com/rust-lang/cargo/issues/4988)
* Stabilization Issue: [#5133](https://github.com/rust-lang/cargo/issues/5133)

When running commands such as `cargo install` or `cargo build`, Cargo
currently requires dev-dependencies to be downloaded, even if they are not
used. The `-Z avoid-dev-deps` flag allows Cargo to avoid downloading
dev-dependencies if they are not needed. The `Cargo.lock` file will not be
generated if dev-dependencies are skipped.

### minimal-versions
* Original Issue: [#4100](https://github.com/rust-lang/cargo/issues/4100)
* Tracking Issue: [#5657](https://github.com/rust-lang/cargo/issues/5657)

> Note: It is not recommended to use this feature. Because it enforces minimal
> versions for all transitive dependencies, its usefulness is limited since
> not all external dependencies declare proper lower version bounds. It is
> intended that it will be changed in the future to only enforce minimal
> versions for direct dependencies.

When a `Cargo.lock` file is generated, the `-Z minimal-versions` flag will
resolve the dependencies to the minimum semver version that will satisfy the
requirements (instead of the greatest version).

The intended use-case of this flag is to check, during continuous integration,
that the versions specified in Cargo.toml are a correct reflection of the
minimum versions that you are actually using. That is, if Cargo.toml says
`foo = "1.0.0"` that you don't accidentally depend on features added only in
`foo 1.5.0`.

### out-dir
* Original Issue: [#4875](https://github.com/rust-lang/cargo/issues/4875)
* Tracking Issue: [#6790](https://github.com/rust-lang/cargo/issues/6790)

This feature allows you to specify the directory where artifacts will be
copied to after they are built. Typically artifacts are only written to the
`target/release` or `target/debug` directories. However, determining the
exact filename can be tricky since you need to parse JSON output. The
`--out-dir` flag makes it easier to predictably access the artifacts. Note
that the artifacts are copied, so the originals are still in the `target`
directory. Example:

```
cargo +nightly build --out-dir=out -Z unstable-options
```

### doctest-xcompile
* Tracking Issue: [#7040](https://github.com/rust-lang/cargo/issues/7040)
* Tracking Rustc Issue: [#64245](https://github.com/rust-lang/rust/issues/64245)

This flag changes `cargo test`'s behavior when handling doctests when
a target is passed. Currently, if a target is passed that is different
from the host cargo will simply skip testing doctests. If this flag is
present, cargo will continue as normal, passing the tests to doctest,
while also passing it a `--target` option, as well as enabling
`-Zunstable-features --enable-per-target-ignores` and passing along
information from `.cargo/config`. See the rustc issue for more information.

```
cargo test --target foo -Zdoctest-xcompile
```

### Profile Overrides
* Tracking Issue: [rust-lang/rust#48683](https://github.com/rust-lang/rust/issues/48683)
* RFC: [#2282](https://github.com/rust-lang/rfcs/blob/master/text/2282-profile-dependencies.md)

Profiles can be overridden for specific packages and custom build scripts.
The general format looks like this:

```toml
cargo-features = ["profile-overrides"]

[package]
...

[profile.dev]
opt-level = 0
debug = true

# the `image` crate will be compiled with -Copt-level=3
[profile.dev.package.image]
opt-level = 3

# All dependencies (but not this crate itself or any workspace member)
# will be compiled with -Copt-level=2 . This includes build dependencies.
[profile.dev.package."*"]
opt-level = 2

# Build scripts or proc-macros and their dependencies will be compiled with
# `-Copt-level=3`. By default, they use the same rules as the rest of the
# profile.
[profile.dev.build-override]
opt-level = 3
```

Overrides can be specified for any profile, including custom named profiles.


### Custom named profiles

* Tracking Issue: [rust-lang/cargo#6988](https://github.com/rust-lang/cargo/issues/6988)
* RFC: [#2678](https://github.com/rust-lang/rfcs/pull/2678)

With this feature you can define custom profiles having new names. With the
custom profile enabled, build artifacts can be emitted by default to
directories other than `release` or `debug`, based on the custom profile's
name.

For example:

```toml
cargo-features = ["named-profiles"]

[profile.release-lto]
inherits = "release"
lto = true
````

An `inherits` key is used in order to receive attributes from other profiles,
so that a new custom profile can be based on the standard `dev` or `release`
profile presets. Cargo emits errors in case `inherits` loops are detected. When
considering inheritance hierarchy, all profiles directly or indirectly inherit
from either from `release` or from `dev`.

Valid profile names are: must not be empty, use only alphanumeric characters or
`-` or `_`.

Passing `--profile` with the profile's name to various Cargo commands, directs
operations to use the profile's attributes. Overrides that are specified in the
profiles from which the custom profile inherits are inherited too.

For example, using `cargo build` with `--profile` and the manifest from above:

```
cargo +nightly build --profile release-lto -Z unstable-options
```

When a custom profile is used, build artifcats go to a different target by
default. In the example above, you can expect to see the outputs under
`target/release-lto`.


#### New `dir-name` attribute

Some of the paths generated under `target/` have resulted in a de-facto "build
protocol", where `cargo` is invoked as a part of a larger project build. So, to
preserve the existing behavior, there is also a new attribute `dir-name`, which
when left unspecified, defaults to the name of the profile. For example:

```toml
[profile.release-lto]
inherits = "release"
dir-name = "lto"  # Emits to target/lto instead of target/release-lto
lto = true
```


### Config Profiles
* Tracking Issue: [rust-lang/rust#48683](https://github.com/rust-lang/rust/issues/48683)
* RFC: [#2282](https://github.com/rust-lang/rfcs/blob/master/text/2282-profile-dependencies.md)

Profiles can be specified in `.cargo/config` files. The `-Z config-profile`
command-line flag is required to use this feature. The format is the same as
in a `Cargo.toml` manifest. If found in multiple config files, settings will
be merged using the regular [config hierarchy](config.md#hierarchical-structure).
Config settings take precedence over manifest settings.

```toml
[profile.dev]
opt-level = 3
```

```
cargo +nightly build -Z config-profile
```


### Namespaced features
* Original issue: [#1286](https://github.com/rust-lang/cargo/issues/1286)
* Tracking Issue: [#5565](https://github.com/rust-lang/cargo/issues/5565)

Currently, it is not possible to have a feature and a dependency with the same
name in the manifest. If you set `namespaced-features` to `true`, the namespaces
for features and dependencies are separated. The effect of this is that, in the
feature requirements, dependencies have to be prefixed with `crate:`. Like this:

```toml
[package]
namespaced-features = true

[features]
bar = ["crate:baz", "foo"]
foo = []

[dependencies]
baz = { version = "0.1", optional = true }
```

To prevent unnecessary boilerplate from having to explicitly declare features
for each optional dependency, implicit features get created for any optional
dependencies where a feature of the same name is not defined. However, if
a feature of the same name as a dependency is defined, that feature must
include the dependency as a requirement, as `foo = ["crate:foo"]`.


### Build-plan
* Tracking Issue: [#5579](https://github.com/rust-lang/cargo/issues/5579)

The `--build-plan` argument for the `build` command will output JSON with
information about which commands would be run without actually executing
anything. This can be useful when integrating with another build tool.
Example:

```
cargo +nightly build --build-plan -Z unstable-options
```

### Metabuild
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

### install-upgrade
* Tracking Issue: [#6797](https://github.com/rust-lang/cargo/issues/6797)

The `install-upgrade` feature changes the behavior of `cargo install` so that
it will reinstall a package if it is not "up-to-date". If it is "up-to-date",
it will do nothing and exit with success instead of failing. Example:

```
cargo +nightly install foo -Z install-upgrade
```

Cargo tracks some information to determine if a package is "up-to-date",
including:

- The package version and source.
- The set of binary names installed.
- The chosen features.
- The release mode (`--debug`).
- The target (`--target`).

If any of these values change, then Cargo will reinstall the package.

Installation will still fail if a different package installs a binary of the
same name. `--force` may be used to unconditionally reinstall the package.

Installing with `--path` will always build and install, unless there are
conflicting binaries from another package.

Additionally, a new flag `--no-track` is available to prevent `cargo install`
from writing tracking information in `$CARGO_HOME` about which packages are
installed.

### public-dependency
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

### build-std
* Tracking Repository: https://github.com/rust-lang/wg-cargo-std-aware

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

```
$ rustup component add rust-src --toolchain nightly
```

It is also required today that the `-Z build-std` flag is combined with the
`--target` flag. Note that you're not forced to do a cross compilation, you're
just forced to pass `--target` in one form or another.

Usage looks like:

```
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

```
$ cargo +nightly build -Z build-std=core,alloc
```

The value here is a comma-separated list of standard library crates to build.

#### Requirements

As a summary, a list of requirements today to use `-Z build-std` are:

* You must install libstd's source code through `rustup component add rust-src`
* You must pass `--target`
* You must use both a nightly Cargo and a nightly rustc
* The `-Z build-std` flag must be passed to all `cargo` invocations.

#### Reporting bugs and helping out

The `-Z build-std` feature is in the very early stages of development! This
feature for Cargo has an extremely long history and is very large in scope, and
this is just the beginning. If you'd like to report bugs please either report
them to:

* Cargo - https://github.com/rust-lang/cargo/issues/new - for implementation bugs
* The tracking repository -
  https://github.com/rust-lang/wg-cargo-std-aware/issues/new - for larger design
  questions.

Also if you'd like to see a feature that's not yet implemented and/or if
something doesn't quite work the way you'd like it to, feel free to check out
the [issue tracker](https://github.com/rust-lang/wg-cargo-std-aware/issues) of
the tracking repository, and if it's not there please file a new issue!

### timings
* Tracking Issue: [#7405](https://github.com/rust-lang/cargo/issues/7405)

The `timings` feature gives some information about how long each compilation
takes, and tracks concurrency information over time.

```
cargo +nightly build -Z timings
```

The `-Ztimings` flag can optionally take a comma-separated list of the
following values:

- `html` — Saves a file called `cargo-timing.html` to the current directory
  with a report of the compilation. Files are also saved with a timestamp in
  the filename if you want to look at older runs.
- `info` — Displays a message to stdout after each compilation finishes with
  how long it took.
- `json` — Emits some JSON information about timing information.

The default if none are specified is `html,info`.

#### Reading the graphs

There are two graphs in the output. The "unit" graph shows the duration of
each unit over time. A "unit" is a single compiler invocation. There are lines
that show which additional units are "unlocked" when a unit finishes. That is,
it shows the new units that are now allowed to run because their dependencies
are all finished. Hover the mouse over a unit to highlight the lines. This can
help visualize the critical path of dependencies. This may change between runs
because the units may finish in different orders.

The "codegen" times are highlighted in a lavender color. In some cases, build
pipelining allows units to start when their dependencies are performing code
generation. This information is not always displayed (for example, binary
units do not show when code generation starts).

The "custom build" units are `build.rs` scripts, which when run are
highlighted in orange.

The second graph shows Cargo's concurrency over time. The three lines are:
- "Waiting" (red) — This is the number of units waiting for a CPU slot to
  open.
- "Inactive" (blue) — This is the number of units that are waiting for their
  dependencies to finish.
- "Active" (green) — This is the number of units currently running.

Note: This does not show the concurrency in the compiler itself. `rustc`
coordinates with Cargo via the "job server" to stay within the concurrency
limit. This currently mostly applies to the code generation phase.

Tips for addressing compile times:
- Look for slow dependencies.
    - Check if they have features that you may wish to consider disabling.
    - Consider trying to remove the dependency completely.
- Look for a crate being built multiple times with different versions. Try to
  remove the older versions from the dependency graph.
- Split large crates into smaller pieces.
- If there are a large number of crates bottlenecked on a single crate, focus
  your attention on improving that one crate to improve parallelism.

### binary-dep-depinfo
* Tracking rustc issue: [#63012](https://github.com/rust-lang/rust/issues/63012)

The `-Z binary-dep-depinfo` flag causes Cargo to forward the same flag to
`rustc` which will then cause `rustc` to include the paths of all binary
dependencies in the "dep info" file (with the `.d` extension). Cargo then uses
that information for change-detection (if any binary dependency changes, then
the crate will be rebuilt). The primary use case is for building the compiler
itself, which has implicit dependencies on the standard library that would
otherwise be untracked for change-detection.

### panic-abort-tests

The `-Z panic-abort-tests` flag will enable nightly support to compile test
harness crates with `-Cpanic=abort`. Without this flag Cargo will compile tests,
and everything they depend on, with `-Cpanic=unwind` because it's the only way
`test`-the-crate knows how to operate. As of [rust-lang/rust#64158], however,
the `test` crate supports `-C panic=abort` with a test-per-process, and can help
avoid compiling crate graphs multiple times.

It's currently unclear how this feature will be stabilized in Cargo, but we'd
like to stabilize it somehow!

[rust-lang/rust#64158]: https://github.com/rust-lang/rust/pull/64158
