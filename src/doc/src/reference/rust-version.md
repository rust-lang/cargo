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

## Support Expectations

These are general expectations; some packages may document when they do not follow these.

**Complete:**

All functionality, including binaries and API, are available on the supported Rust versions under every [feature](features.md).

**Verified:**

A package's functionality is verified on its supported Rust versions, including automated testing.
See also our
[Rust version CI guide](../guide/continuous-integration.md#verifying-rust-version).

**Patchable:**

When licenses allow it,
users can [override their local dependency](overriding-dependencies.md) with a fork of your package.
In this situation, Cargo may load the entire workspace for the patched dependency which should work on the supported Rust versions, even if other packages in the workspace have different supported Rust versions.

**Dependency Support:**

In support of the above,
it is expected that each dependency's version-requirement supports at least one version compatible with your `rust-version`.
However,
it is **not** expected that the dependency specification excludes versions incompatible with your `rust-version`.
In fact, supporting both allows you to balance the needs of users that support older Rust versions with those that don't.
