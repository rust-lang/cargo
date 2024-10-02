# Rust Version

The `rust-version` field is an optional key that tells cargo what version of the
Rust toolchain you support for your package.

```toml
[package]
# ...
rust-version = "1.56"
```

The Rust version must be a bare version number with at least one component; it
cannot include semver operators or pre-release identifiers. Compiler pre-release
identifiers such as -nightly will be ignored while checking the Rust version.

To find the minimum `rust-version` compatible with your project, you can use third-party tools like [`cargo-msrv`](https://crates.io/crates/cargo-msrv).

When used on packages that get published, we recommend [verifying the `rust-version`](../guide/continuous-integration.md#verifying-rust-version).

> **MSRV:** Respected as of 1.56

## Uses

**Diagnostics:**

When your package is compiled on an unsupported toolchain,
Cargo will provide clearer diagnostics about the insufficient toolchain version rather than reporting invalid syntax or missing functionality in the standard library.
This affects all [Cargo targets](cargo-targets.md) in the package, including binaries, examples, test suites,
benchmarks, etc.

**Development aid:**

`cargo add` will auto-select the dependency's version requirement to be the latest version compatible with your `rust-version`.
If that isn't the latest version, `cargo add` will inform users so they can make the choice on whether to keep it or update your `rust-version`.

Other tools may also take advantage of it, like `cargo clippy`'s
[`incompatible_msrv` lint](https://rust-lang.github.io/rust-clippy/stable/index.html#/incompatible_msrv).

> **Note:** The `rust-version` may be ignored using the `--ignore-rust-version` option.
