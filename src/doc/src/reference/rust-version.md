# Rust Version

The `rust-version` field is an optional key that tells cargo what version of the
Rust language and compiler you support for your package.
If the currently selected version of the Rust compiler is older than the stated
version, cargo will exit with an error, telling the user what version is
required.
This affects all targets/crates in the package, including test suites,
benchmarks, binaries, examples, etc.

The `rust-version` may be ignored using the `--ignore-rust-version` option.

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
