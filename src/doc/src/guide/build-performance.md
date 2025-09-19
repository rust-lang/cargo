# Optimizing Build Performance

Cargo configuration options and source code organization patterns can help improve build performance, by prioritizing it over other aspects which may not be as important for your circumstances.

Same as when optimizing runtime performance, be sure to measure these changes against the workflows you actually care about, as we provide general guidelines and your circumstances may be different, it is possible that some of these approaches might actually make build performance worse for your use-case.

Example workflows to consider include:
- Compiler feedback as you develop (`cargo check` after making a code change)
- Test feedback as you develop (`cargo test` after making a code change)
- CI builds

## Cargo and Compiler Configuration

Cargo uses configuration defaults that try to balance several aspects, including debuggability, runtime performance, build performance, binary size and others. This section describes several approaches for changing these defaults that should be designed to maximize build performance.

You can set the described options either in the [`Cargo.toml` manifest](../reference/profiles.md), which will make them available for all developers who work on the given crate/project, or in the [`config.toml` configuration file](../reference/config.md), where you can apply them only for you or even globally for all your local projects.

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
- ✅ Faster build times
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