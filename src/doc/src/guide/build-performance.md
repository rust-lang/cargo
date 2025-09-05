# Optimizing Build Performance

Cargo uses configuration defaults that try to balance various aspects, including debuggability, runtime performance, build performance, binary size and others. Because of that, build performance is sometimes traded off for other benefits which may not be as important for your circumstances. This guide will step you through changes you can make to improve build performance.

Same as when optimizing runtime performance, be sure to measure these changes against the workflows you actually care about, as we provide general guidelines and your circumstances may be different.

Example workflows to consider include:
- Compiler feedback as you develop (`cargo check` after making a code change)
- Test feedback as you develop (`cargo test` after making a code change)
- CI builds

Note that some approaches described below currently require using the nightly toolchain.

## Reduce amount of generated debug information

Recommendation: Add to your `Cargo.toml` (for maintainers) or `$CARGO_HOME/.cargo/config.toml` (for contributors):

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
- ✅ Faster per-crate build times
- ✅ Faster link times
- ✅ Smaller disk usage of the `target` directory 
- ❌ Requires a full rebuild to have a high-quality debugger experience

## Use an alternative codegen backend

> **This requires nightly/unstable features**

The component of the Rust compiler that generates executable code is called a "codegen backend". The default backend is LLVM, which produces very optimized code, at the cost of relatively slow compilation time. You can try to use a different codegen backend in order to speed up the compilation of your crate.

You can use the [Cranelift](https://github.com/rust-lang/rustc_codegen_cranelift) backend, which is designed for fast(er) compilation time. You can install this backend using rustup:

```console
$ rustup component add rustc-codegen-cranelift-preview --toolchain nightly
```

and then enable it for a given Cargo profile using the `codegen-backend` option in `Cargo.toml`:
```toml
[profile.dev]
codegen-backend = "cranelift"
```

Since this is currently an unstable option, you will also need to either pass `-Z codegen-backend` to Cargo, or enable this unstable option in the `.cargo/config.toml` file. You can find more information about the unstable `codegen-backend` profile option [here](../reference/unstable.md#codegen-backend).

Note that the Cranelift backend might not support all features used by your crate. It is also available only for a limited set of targets.
