# Profiles

Profiles provide a way to alter the compiler settings, influencing things like
optimizations and debugging symbols.

Cargo has 4 built-in profiles: `dev`, `release`, `test`, and `bench`. The
profile is automatically chosen based on which command is being run if a
profile is not specified on the command-line. In addition to the built-in
profiles, custom user-defined profiles can also be specified.

Profile settings can be changed in [`Cargo.toml`](manifest.md) with the
`[profile]` table. Within each named profile, individual settings can be changed
with key/value pairs like this:

```toml
[profile.dev]
opt-level = 1               # Use slightly better optimizations.
overflow-checks = false     # Disable integer overflow checks.
```

Cargo only looks at the profile settings in the `Cargo.toml` manifest at the
root of the workspace. Profile settings defined in dependencies will be
ignored.

Additionally, profiles can be overridden from a [config] definition.
Specifying a profile in a config file or environment variable will override
the settings from `Cargo.toml`.

[config]: config.md

## Profile settings

The following is a list of settings that can be controlled in a profile.

### opt-level

The `opt-level` setting controls the [`-C opt-level` flag] which controls the level
of optimization. Higher optimization levels may produce faster runtime code at
the expense of longer compiler times. Higher levels may also change and
rearrange the compiled code which may make it harder to use with a debugger.

The valid options are:

* `0`: no optimizations
* `1`: basic optimizations
* `2`: some optimizations
* `3`: all optimizations
* `"s"`: optimize for binary size
* `"z"`: optimize for binary size, but also turn off loop vectorization.

It is recommended to experiment with different levels to find the right
balance for your project. There may be surprising results, such as level `3`
being slower than `2`, or the `"s"` and `"z"` levels not being necessarily
smaller. You may also want to reevaluate your settings over time as newer
versions of `rustc` changes optimization behavior.

See also [Profile Guided Optimization] for more advanced optimization
techniques.

[`-C opt-level` flag]: ../../rustc/codegen-options/index.html#opt-level
[Profile Guided Optimization]: ../../rustc/profile-guided-optimization.html

### debug

The `debug` setting controls the [`-C debuginfo` flag] which controls the
amount of debug information included in the compiled binary.

The valid options are:

* `0`, `false`, or `"none"`: no debug info at all, default for [`release`](#release)
* `"line-directives-only"`: line info directives only. For the nvptx* targets this enables [profiling]. For other use cases, `line-tables-only` is the better, more compatible choice.
* `"line-tables-only"`: line tables only. Generates the minimal amount of debug info for backtraces with filename/line number info, but not anything else, i.e. no variable or function parameter info.
* `1` or `"limited"`: debug info without type or variable-level information. Generates more detailed module-level info than `line-tables-only`.
* `2`, `true`, or `"full"`: full debug info, default for [`dev`](#dev)

For more information on what each option does see `rustc`'s docs on [debuginfo].

You may wish to also configure the [`split-debuginfo`](#split-debuginfo) option
depending on your needs as well.

[`-C debuginfo` flag]: ../../rustc/codegen-options/index.html#debuginfo
[debuginfo]: ../../rustc/codegen-options/index.html#debuginfo
[profiling]: https://reviews.llvm.org/D46061

### split-debuginfo

The `split-debuginfo` setting controls the [`-C split-debuginfo` flag] which
controls whether debug information, if generated, is either placed in the
executable itself or adjacent to it.

This option is a string and acceptable values are the same as those the
[compiler accepts][`-C split-debuginfo` flag]. The default value for this option
is `unpacked` on macOS for profiles that have debug information otherwise
enabled. Otherwise the default for this option is [documented with rustc][`-C
split-debuginfo` flag] and is platform-specific. Some options are only
available on the [nightly channel]. The Cargo default may change in the future
once more testing has been performed, and support for DWARF is stabilized.

Be aware that Cargo and rustc have different defaults for this option. This
option exists to allow Cargo to experiment on different combinations of flags
thus providing better debugging and developer experience.

[nightly channel]: ../../book/appendix-07-nightly-rust.html
[`-C split-debuginfo` flag]: ../../rustc/codegen-options/index.html#split-debuginfo

### strip

The `strip` option controls the [`-C strip` flag], which directs rustc to
strip either symbols or debuginfo from a binary. This can be enabled like so:

```toml
[package]
# ...

[profile.release]
strip = "debuginfo"
```

Possible string values of `strip` are `"none"`, `"debuginfo"`, and `"symbols"`.
The default is `"none"`.

You can also configure this option with the boolean values `true` or `false`.
`strip = true` is equivalent to `strip = "symbols"`. `strip = false` is
equivalent to `strip = "none"` and disables `strip` completely.

[`-C strip` flag]: ../../rustc/codegen-options/index.html#strip

### debug-assertions

The `debug-assertions` setting controls the [`-C debug-assertions` flag] which
turns `cfg(debug_assertions)` [conditional compilation] on or off. Debug
assertions are intended to include runtime validation which is only available
in debug/development builds. These may be things that are too expensive or
otherwise undesirable in a release build. Debug assertions enables the
[`debug_assert!` macro] in the standard library.

The valid options are:

* `true`: enabled
* `false`: disabled

[`-C debug-assertions` flag]: ../../rustc/codegen-options/index.html#debug-assertions
[conditional compilation]: ../../reference/conditional-compilation.md#debug_assertions
[`debug_assert!` macro]: ../../std/macro.debug_assert.html

### overflow-checks

The `overflow-checks` setting controls the [`-C overflow-checks` flag] which
controls the behavior of [runtime integer overflow]. When overflow-checks are
enabled, a panic will occur on overflow.

The valid options are:

* `true`: enabled
* `false`: disabled

[`-C overflow-checks` flag]: ../../rustc/codegen-options/index.html#overflow-checks
[runtime integer overflow]: ../../reference/expressions/operator-expr.md#overflow

### lto

The `lto` setting controls `rustc`'s [`-C lto`], [`-C linker-plugin-lto`], and
[`-C embed-bitcode`] options, which control LLVM's [link time optimizations].
LTO can produce better optimized code, using whole-program analysis, at the cost
of longer linking time.

The valid options are:

* `false`: Performs "thin local LTO" which performs "thin" LTO on the local
  crate only across its [codegen units](#codegen-units). No LTO is performed
  if codegen units is 1 or [opt-level](#opt-level) is 0.
* `true` or `"fat"`: Performs "fat" LTO which attempts to perform
  optimizations across all crates within the dependency graph.
* `"thin"`: Performs ["thin" LTO]. This is similar to "fat", but takes
  substantially less time to run while still achieving performance gains
  similar to "fat".
* `"off"`: Disables LTO.

See the [linker-plugin-lto chapter] if you are interested in cross-language LTO.
This is not yet supported natively in Cargo, but can be performed via
`RUSTFLAGS`.

[`-C lto`]: ../../rustc/codegen-options/index.html#lto
[link time optimizations]: https://llvm.org/docs/LinkTimeOptimization.html
[`-C linker-plugin-lto`]: ../../rustc/codegen-options/index.html#linker-plugin-lto
[`-C embed-bitcode`]: ../../rustc/codegen-options/index.html#embed-bitcode
[linker-plugin-lto chapter]: ../../rustc/linker-plugin-lto.html
["thin" LTO]: http://blog.llvm.org/2016/06/thinlto-scalable-and-incremental-lto.html

### panic

The `panic` setting controls the [`-C panic` flag] which controls which panic
strategy to use.

The valid options are:

* `"unwind"`: Unwind the stack upon panic.
* `"abort"`: Terminate the process upon panic.

When set to `"unwind"`, the actual value depends on the default of the target
platform. For example, the NVPTX platform does not support unwinding, so it
always uses `"abort"`.

Tests, benchmarks, build scripts, and proc macros ignore the `panic` setting.
The `rustc` test harness currently requires `unwind` behavior. See the
[`panic-abort-tests`] unstable flag which enables `abort` behavior.

Additionally, when using the `abort` strategy and building a test, all of the
dependencies will also be forced to build with the `unwind` strategy.

[`-C panic` flag]: ../../rustc/codegen-options/index.html#panic
[`panic-abort-tests`]: unstable.md#panic-abort-tests

### incremental

The `incremental` setting controls the [`-C incremental` flag] which controls
whether or not incremental compilation is enabled. Incremental compilation
causes `rustc` to save additional information to disk which will be reused
when recompiling the crate, improving re-compile times. The additional
information is stored in the `target` directory.

The valid options are:

* `true`: enabled
* `false`: disabled

Incremental compilation is only used for workspace members and "path"
dependencies.

The incremental value can be overridden globally with the `CARGO_INCREMENTAL`
[environment variable] or the [`build.incremental`] config variable.

[`-C incremental` flag]: ../../rustc/codegen-options/index.html#incremental
[environment variable]: environment-variables.md
[`build.incremental`]: config.md#buildincremental

### codegen-units

The `codegen-units` setting controls the [`-C codegen-units` flag] which
controls how many "code generation units" a crate will be split into. More
code generation units allows more of a crate to be processed in parallel
possibly reducing compile time, but may produce slower code.

This option takes an integer greater than 0.

The default is 256 for [incremental](#incremental) builds, and 16 for
non-incremental builds.

[`-C codegen-units` flag]: ../../rustc/codegen-options/index.html#codegen-units

### rpath

The `rpath` setting controls the [`-C rpath` flag] which controls
whether or not [`rpath`] is enabled.

[`-C rpath` flag]: ../../rustc/codegen-options/index.html#rpath
[`rpath`]: https://en.wikipedia.org/wiki/Rpath

## Default profiles

### dev

The `dev` profile is used for normal development and debugging. It is the
default for build commands like [`cargo build`], and is used for `cargo install --debug`.

The default settings for the `dev` profile are:

```toml
[profile.dev]
opt-level = 0
debug = true
split-debuginfo = '...'  # Platform-specific.
strip = "none"
debug-assertions = true
overflow-checks = true
lto = false
panic = 'unwind'
incremental = true
codegen-units = 256
rpath = false
```

### release

The `release` profile is intended for optimized artifacts used for releases
and in production. This profile is used when the `--release` flag is used, and
is the default for [`cargo install`].

The default settings for the `release` profile are:

```toml
[profile.release]
opt-level = 3
debug = false
split-debuginfo = '...'  # Platform-specific.
strip = "none"
debug-assertions = false
overflow-checks = false
lto = false
panic = 'unwind'
incremental = false
codegen-units = 16
rpath = false
```

### test

The `test` profile is the default profile used by [`cargo test`].
The `test` profile inherits the settings from the [`dev`](#dev) profile.

### bench

The `bench` profile is the default profile used by [`cargo bench`].
The `bench` profile inherits the settings from the [`release`](#release) profile.

### Build Dependencies

To compile quickly, all profiles, by default, do not optimize build
dependencies (build scripts, proc macros, and their dependencies), and avoid
computing debug info when a build dependency is not used as a runtime
dependency. The default settings for build overrides are:

```toml
[profile.dev.build-override]
opt-level = 0
codegen-units = 256
debug = false # when possible

[profile.release.build-override]
opt-level = 0
codegen-units = 256
```

However, if errors occur while running build dependencies, turning full debug
info on will improve backtraces and debuggability when needed:

```toml
debug = true
```

Build dependencies otherwise inherit settings from the active profile in use, as
described in [Profile selection](#profile-selection).

## Custom profiles

In addition to the built-in profiles, additional custom profiles can be
defined. These may be useful for setting up multiple workflows and build
modes. When defining a custom profile, you must specify the `inherits` key to
specify which profile the custom profile inherits settings from when the
setting is not specified.

For example, let's say you want to compare a normal release build with a
release build with [LTO](#lto) optimizations, you can specify something like
the following in `Cargo.toml`:

```toml
[profile.release-lto]
inherits = "release"
lto = true
```

The `--profile` flag can then be used to choose this custom profile:

```console
cargo build --profile release-lto
```

The output for each profile will be placed in a directory of the same name
as the profile in the [`target` directory]. As in the example above, the
output would go into the `target/release-lto` directory.

[`target` directory]: ../guide/build-cache.md

## Profile selection

The profile used depends on the command, the command-line flags like
`--release` or `--profile`, and the package (in the case of
[overrides](#overrides)). The default profile if none is specified is:

| Command | Default Profile |
|---------|-----------------|
| [`cargo run`], [`cargo build`],<br>[`cargo check`], [`cargo rustc`] | [`dev` profile](#dev) |
| [`cargo test`] | [`test` profile](#test)
| [`cargo bench`] | [`bench` profile](#bench)
| [`cargo install`] | [`release` profile](#release)

You can switch to a different profile using the `--profile=NAME` option which will used the given profile.
The `--release` flag is equivalent to `--profile=release`.

The selected profile applies to all Cargo targets, 
including [library](./cargo-targets.md#library),
[binary](./cargo-targets.md#binaries), 
[example](./cargo-targets.md#examples), 
[test](./cargo-targets.md#tests), 
and [benchmark](./cargo-targets.md#benchmarks).

The profile for specific packages can be specified with
[overrides](#overrides), described below.

[`cargo bench`]: ../commands/cargo-bench.md
[`cargo build`]: ../commands/cargo-build.md
[`cargo check`]: ../commands/cargo-check.md
[`cargo install`]: ../commands/cargo-install.md
[`cargo run`]: ../commands/cargo-run.md
[`cargo rustc`]: ../commands/cargo-rustc.md
[`cargo test`]: ../commands/cargo-test.md

## Overrides

Profile settings can be overridden for specific packages and build-time
crates. To override the settings for a specific package, use the `package`
table to change the settings for the named package:

```toml
# The `foo` package will use the -Copt-level=3 flag.
[profile.dev.package.foo]
opt-level = 3
```

The package name is actually a [Package ID Spec](pkgid-spec.md), so you can
target individual versions of a package with syntax such as
`[profile.dev.package."foo:2.1.0"]`.

To override the settings for all dependencies (but not any workspace member),
use the `"*"` package name:

```toml
# Set the default for dependencies.
[profile.dev.package."*"]
opt-level = 2
```

To override the settings for build scripts, proc macros, and their
dependencies, use the `build-override` table:

```toml
# Set the settings for build scripts and proc-macros.
[profile.dev.build-override]
opt-level = 3
```

> Note: When a dependency is both a normal dependency and a build dependency,
> Cargo will try to only build it once when `--target` is not specified. When
> using `build-override`, the dependency may need to be built twice, once as a
> normal dependency and once with the overridden build settings. This may
> increase initial build times.

The precedence for which value is used is done in the following order (first
match wins):

1. `[profile.dev.package.name]` --- A named package.
2. `[profile.dev.package."*"]` --- For any non-workspace member.
3. `[profile.dev.build-override]` --- Only for build scripts, proc macros, and
   their dependencies.
4. `[profile.dev]` --- Settings in `Cargo.toml`.
5. Default values built-in to Cargo.

Overrides cannot specify the `panic`, `lto`, or `rpath` settings.

### Overrides and generics

The location where generic code is instantiated will influence the
optimization settings used for that generic code. This can cause subtle
interactions when using profile overrides to change the optimization level of
a specific crate. If you attempt to raise the optimization level of a
dependency which defines generic functions, those generic functions may not be
optimized when used in your local crate. This is because the code may be
generated in the crate where it is instantiated, and thus may use the
optimization settings of that crate.

For example, [nalgebra] is a library which defines vectors and matrices making
heavy use of generic parameters. If your local code defines concrete nalgebra
types like `Vector4<f64>` and uses their methods, the corresponding nalgebra
code will be instantiated and built within your crate. Thus, if you attempt to
increase the optimization level of `nalgebra` using a profile override, it may
not result in faster performance.

Further complicating the issue, `rustc` has some optimizations where it will
attempt to share monomorphized generics between crates. If the opt-level is 2
or 3, then a crate will not use monomorphized generics from other crates, nor
will it export locally defined monomorphized items to be shared with other
crates. When experimenting with optimizing dependencies for development,
consider trying opt-level 1, which will apply some optimizations while still
allowing monomorphized items to be shared.

[nalgebra]: https://crates.io/crates/nalgebra
