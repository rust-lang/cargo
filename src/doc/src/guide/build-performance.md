# Optimizing Build Performance

Cargo configuration options and source code organization patterns can help improve build performance, by prioritizing it over other aspects which may not be as important for your circumstances.

Same as when optimizing runtime performance, be sure to measure these changes against the workflows you actually care about, as we provide general guidelines and your circumstances may be different, it is possible that some of these approaches might actually make build performance worse for your use-case.

Example workflows to consider include:
- Compiler feedback as you develop (`cargo check` after making a code change)
- Test feedback as you develop (`cargo test` after making a code change)
- CI builds

## Cargo and Compiler Configuration

Cargo uses configuration defaults that try to balance several aspects, including debuggability, runtime performance, build performance, binary size and others. This section describes several approaches for changing these defaults that should be designed to maximize build performance.

Common locations to override defaults are:
- [`Cargo.toml` manifest](../reference/profiles.md)
  - Available to all developers contributing to your project
  - Limited in what configuration is supported (see [#12738](https://github.com/rust-lang/cargo/issues/12738) for expanding this)
- [`$WORKSPACE_ROOT/.cargo/config.toml` configuration file](../reference/config.md)
  - Available to all developers contributing to your project
  - Unlike `Cargo.toml`, this is sensitive to what directory you invoke `cargo` from (see [#2930](https://github.com/rust-lang/cargo/issues/2930))
- [`$CARGO_HOME/.cargo/config.toml` configuration file](../reference/config.md)
  - For a developer to control the defaults for their development

### Reduce amount of generated debug information

Recommendation: Add to your `Cargo.toml` or `.cargo/config.toml`:

```toml
[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false

[profile.debugging]
inherits = "dev"
debug = true
```

This will:
- Change the [`dev` profile](../reference/profiles.md#dev) (default for development commands) to:
  - Limit [debug information](../reference/profiles.md#debug) for workspace members to what is needed for useful panic backtraces
  - Avoid generating any debug information for dependencies
- Provide an opt-in for when debugging via [`--profile debugging`](../reference/profiles.md#custom-profiles)

Trade-offs:
- ✅ Faster code generation (`cargo build`)
- ✅ Faster link times
- ✅ Smaller disk usage of the `target` directory
- ❌ Requires a full rebuild to have a high-quality debugger experience

### Use an alternative codegen backend

Recommendation:

- Install the Cranelift codegen backend rustup component
    ```console
    $ rustup component add rustc-codegen-cranelift-preview --toolchain nightly
    ```
- Add to your `Cargo.toml` or `.cargo/config.toml`:
    ```toml
    [profile.dev]
    codegen-backend = "cranelift"
    ```
- Run Cargo with `-Z codegen-backend` or enable the [`codegen-backend`](../reference/unstable.md#codegen-backend) feature in `.cargo/config.toml`.
  - This is required because this is currently an unstable feature.

This will change the [`dev` profile](../reference/profiles.md#dev) to use the [Cranelift codegen backend](https://github.com/rust-lang/rustc_codegen_cranelift) for generating machine code, instead of the default LLVM backend. The Cranelift backend should generate code faster than LLVM, which should result in improved build performance.

Trade-offs:
- ✅ Faster code generation (`cargo build`)
- ❌ **Requires using nightly Rust and an [unstable Cargo feature][codegen-backend-feature]**
- ❌ Worse runtime performance of the generated code
  - Speeds up build part of `cargo test`, but might increase its test execution part
- ❌ Only available for [certain targets](https://github.com/rust-lang/rustc_codegen_cranelift?tab=readme-ov-file#platform-support)
- ❌ Might not support all Rust features (e.g. unwinding)

[codegen-backend-feature]: ../reference/unstable.md#codegen-backend

### Enable the experimental parallel frontend

Recommendation: Add to your `.cargo/config.toml`:

```toml
[build]
rustflags = "-Zthreads=8"
```

This [`rustflags`][build.rustflags] will enable the [parallel frontend][parallel-frontend-blog] of the Rust compiler, and tell it to use `n` threads. The value of `n` should be chosen according to the number of cores available on your system, although there are diminishing returns. We recommend using at most `8` threads.

Trade-offs:
- ✅ Faster build times (both `cargo check` and `cargo build`)
- ❌ **Requires using nightly Rust and an [unstable Rust feature][parallel-frontend-issue]**

[parallel-frontend-blog]: https://blog.rust-lang.org/2023/11/09/parallel-rustc/
[parallel-frontend-issue]: https://github.com/rust-lang/rust/issues/113349
[build.rustflags]: ../reference/config.md#buildrustflags

### Use an alternative linker

Consider: installing and configuring an alternative linker, like [LLD](https://lld.llvm.org/), [mold](https://github.com/rui314/mold) or [wild](https://github.com/davidlattimore/wild). For example, to configure mold on Linux, you can add to your `.cargo/config.toml`:

```toml
[target.'cfg(target_os = "linux")']
# mold, if you have GCC 12+
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

# mold, otherwise
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=/path/to/mold"]
```

While dependencies may be built in parallel, linking all of your dependencies happens at once at the end of your build, which can make linking dominate your build times, especially for incremental rebuilds. Often, the linker Rust uses is already fairly fast and the gains from switching may not be worth it, but it is not always the case. For example, Linux targets besides `x86_64-unknown-linux-gnu` still use the Linux system linker which is quite slow (see [rust#39915](https://github.com/rust-lang/rust/issues/39915) for more details).

Trade-offs:
- ✅ Faster link times
- ❌ Might not support all use-cases, in particular if you depend on C or C++ dependencies

### Resolve features for the whole workspace

Consider: adding to your project's `.cargo/config.toml`

```toml
[resolver]
feature-unification = "workspace"
```

When invoking `cargo`,
[features get activated][resolver-features] based on which workspace members you have selected.
However, when contributing to an application,
you may need to build and test various packages within the application,
which can cause extraneous rebuilds because different sets of features may be activated for common dependencies.
With [`feauture-unification`][feature-unification],
you can reuse more dependency builds by ensuring the same set of dependency features are activated,
independent of which package you are currently building and testing.

Trade-offs:
- ✅ Fewer rebuilds when building different packages in a workspace
- ❌ **Requires using nightly Rust and an [unstable Cargo feature][feature-unification]**
- ❌ A package activating a feature can mask bugs in other packages that should activate it but don't
- ❌ If the feature unification from `--workspace` doesn't work for you, then this won't either

[resolver-features]: ../reference/resolver.md#features
[feature-unification]: ../reference/unstable.md#feature-unification

## Reducing built code

### Removing unused dependencies

Recommendation: Periodically review unused dependencies for removal using third-party tools like
[cargo-machete](https://crates.io/crates/cargo-machete),
[cargo-udeps](https://crates.io/crates/cargo-udeps),
[cargo-shear](https://crates.io/crates/cargo-shear).

When changing code,
it can be easy to miss that a dependency is no longer used and can be removed.

> **Note:** native support for this in Cargo is being tracked in [#15813](https://github.com/rust-lang/cargo/issues/15813).

Trade-offs:
- ✅ Faster full build and link times
- ❌ May incorrectly flag dependencies as unused or miss some

### Removing unused features from dependencies

Recommendation: Periodically review unused features from dependencies for removal using third-party tools like
[cargo-features-manager](https://crates.io/crates/cargo-features-manager),
[cargo-unused-features](https://crates.io/crates/cargo-unused-features).

When changing code,
it can be easy to miss that a dependency's feature is no longer used and can be removed.
This can reduce the number of transitive dependencies being built or
reduce the amount of code within a crate being built.
When removing features, extra caution is needed because features
may also be used for desired behavior or performance changes
which may not always be obvious from compiling or testing.

Trade-offs:
- ✅ Faster full build and link times
- ❌ May incorrectly flag features as unused
