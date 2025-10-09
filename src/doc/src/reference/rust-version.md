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

> **MSRV:** Respected as of 1.56

## Uses

**Diagnostics:**

When your package is compiled on an unsupported toolchain, Cargo will report that as an error to the user. This makes the support expectations clear and avoids reporting a less direct diagnostic like invalid syntax or missing functionality
in the standard library. This affects all [Cargo targets](cargo-targets.md) in the
package, including binaries, examples, test suites, benchmarks, etc.
A user can opt-in to an unsupported build of a package with the `--ignore-rust-version` flag.


**Development aid:**

`cargo add` will auto-select the dependency's version requirement to be the latest version compatible with your `rust-version`.
If that isn't the latest version, `cargo add` will inform users so they can make the choice on whether to keep it or update your `rust-version`.

The [resolver](resolver.md#rust-version) may take Rust version into account when picking dependencies.

Other tools may also take advantage of it, like `cargo clippy`'s
[`incompatible_msrv` lint](https://rust-lang.github.io/rust-clippy/stable/index.html#incompatible_msrv).

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

## Setting and Updating Rust Version

What Rust versions to support is a trade off between
- Costs for the maintainer in not using newer features of the Rust toolchain or their dependencies
- Costs to users who would benefit from a package using newer features of a toolchain, e.g. reducing build times by migrating to a feature in the standard library from a polyfill
- Availability of a package to users supporting older Rust versions

> **Note:** [Changing `rust-version`](semver.md#env-new-rust) is assumed to be a minor incompatibility

> **Recommendation:** Choose a policy for what Rust versions to support and when that is changed so users can compare it with their own policy and,
> if it isn't compatible,
> decide whether the loss of general improvements or the risk of a blocking bug that won't be fixed is acceptable or not.
>
> The simplest policy to support is to always use the latest Rust version.
>
> Depending on your risk profile, the next simplest approach is to continue to support old major or minor versions of your package that support older Rust versions.

### Selecting supported Rust versions

Users of your package are most likely to track their supported Rust versions to:
- Their Rust toolchain vendor's support policy, e.g. The Rust Project or a Linux distribution
  - Note: the Rust Project only offers bug fixes and security updates for the latest version.
- A fixed schedule for users to re-verify their packages with the new toolchain, e.g. the first release of the year, every 5 releases.

In addition, users are unlikely to be using the new Rust version immediately but need time to notice and re-verify or might not be aligned on the exact same schedule..

Example version policies:
- "N-2", meaning "latest version with a 2 release grace window for updating"
- Every even release with a 2 release grace window for updating
- Every version from this calendar year with a one year grace window for updating

> **Note:** To find the minimum `rust-version` compatible with your project as-is, you can use third-party tools like [`cargo-msrv`](https://crates.io/crates/cargo-msrv).

### Update timeline

When your policy specifies you no longer need to support a Rust version, you can update `rust-version` immediately or when needed.

By allowing `rust-version` to drift from your policy,
you offer users more of a grace window for upgrading.
However, this is too unpredictable to be relied on for aligning with the Rust version users track.

The further `rust-version` drifts from your specified policy,
the more likely users are to infer a policy you did not intend,
leading to frustration at the unmet expectations.

When drift is allowed,
there is the question of what is "justifiable enough" to drop supported Versions.
Each person can come to a reasonably different justification;
working through that discussion can be frustrating for the involved parties.
This will disempower those who would want to avoid that type of conflict,
which is particularly the case for new or casual contributors who either
feel that they are not in a position to raise the question or
that the conflict may hurt the chance of their change being merged.

### Multiple Policies in a Workspace

Cargo allows supporting multiple policies within one workspace.

Verifying specific packages under specific Rust versions can get complicated.
Tools like [`cargo-hack`](https://crates.io/crates/cargo-hack) can help.

For any dependency shared across policies,
the lowest common versions must be used as Cargo
[unifies SemVer-compatible versions](resolver.md#semver-compatibility),
potentially limiting access to features of the shared dependency for the workspace member with the higher `rust-version`.

To allow users to patch a dependency on one of your workspace members,
every package in the workspace would need to be loadable in the oldest Rust version supported by the workspace.

When using [`incompatible-rust-versions = "fallback"`](config.md#resolverincompatible-rust-versions),
the Rust version of one package can affect dependency versions selected for another package with a different Rust version.
See the [resolver](resolver.md#rust-version) chapter for more details.

### One or More Policies

One way to mitigate the downsides of supporting older Rust versions is to apply your policy to older major or minor versions of your package that you continue to support.
You likely still need a policy for what Rust versions the development branch support compared to the release branches for those major or minor versions.

Only updating the development branch when "needed"' can help reduce the number of supported release branches.

There is the question of what can be backported into these release branches.
By backporting new functionality between minor versions,
the next available version would be missing it which could be considered a breaking change, violating SemVer.
Backporting changes also comes with the risk of introducing bugs.

Supporting older versions comes at a cost.
This cost is dependent on the risk and impact of bugs within the package and what is acceptable for backporting.
Creating the release branches on-demand and putting the backport burden on the community are ways to balance this cost.

There is not yet a way for dependency management tools to report that a non-latest version is still supported,
shifting the responsibility to users to notice this in documentation.

For example, a Rust version support policy could look like:
- The development branch tracks to the latest stable release from the Rust Project, updated when needed
  - The minor version will be raised when changing `rust-version`
- The project supports every version for this calendar year, with another year grace window
  - The last minor version that supports a supported Rust version will receive community provided bug fixes
  - Fixes must be backported to all supported minor releases between the development branch and the needed supported Rust version
