# Dependency Resolution

One of Cargo's primary tasks is to determine the versions of dependencies to
use based on the version requirements specified in each package. This process
is called "dependency resolution" and is performed by the "resolver". The
result of the resolution is stored in the `Cargo.lock` file which "locks" the
dependencies to specific versions, and keeps them fixed over time.

The resolver attempts to unify common dependencies while considering possibly
conflicting requirements. It turns out, however, that in many cases there is no
single "best" dependency resolution, and so the resolver must use heuristics to
choose a preferred solution. The sections below provide some details on how
requirements are handled, and how to work with the resolver.

See the chapter [Specifying Dependencies] for more details about how
dependency requirements are specified.

The [`cargo tree`] command can be used to visualize the result of the
resolver.

[Specifying Dependencies]: specifying-dependencies.md
[`cargo tree`]: ../commands/cargo-tree.md

## SemVer compatibility

Cargo uses [SemVer] for specifying version numbers. This establishes a common
convention for what is compatible between different versions of a package. See
the [SemVer Compatibility] chapter for guidance on what is considered a
"compatible" change. This notion of "compatibility" is important because Cargo
assumes it should be safe to update a dependency within a compatibility range
without breaking the build.

Versions are considered compatible if their left-most non-zero
major/minor/patch component is the same. For example, `1.0.3` and `1.1.0` are
considered compatible, and thus it should be safe to update from the older
release to the newer one. However, an update from `1.1.0` to `2.0.0` would not
be allowed to be made automatically. This convention also applies to versions
with leading zeros. For example, `0.1.0` and `0.1.2` are compatible, but
`0.1.0` and `0.2.0` are not. Similarly, `0.0.1` and `0.0.2` are not
compatible.

As a quick refresher, the
[*version requirement* syntax][Specifying Dependencies] Cargo uses for
dependencies is:

Requirement | Example | Equivalence | Description
------------|---------|-------------|-------------
Caret | `1.2.3` or `^1.2.3` | <code>>=1.2.3,&nbsp;<2.0.0</code> | Any SemVer-compatible version of at least the given value.
Tilde | `~1.2` | <code>>=1.2.0,&nbsp;<1.3.0</code> | Minimum version, with restricted compatibility range.
Wildcard | `1.*` | <code>>=1.0.0,&nbsp;<2.0.0</code> | Any version in the `*` position.
Equals | `=1.2.3` | <code>=1.2.3</code> | Exactly the specified version only.
Comparison | `>1.1` | <code>>=1.2.0</code> | Naive numeric comparison of specified digits.
Compound | <code>>=1.2,&nbsp;<1.5</code> | <code>>=1.2.0,&nbsp;<1.5.0</code> | Multiple requirements that must be simultaneously satisfied.

When multiple packages specify a dependency for a common package, the resolver
attempts to ensure that they use the same version of that common package, as
long as they are within a SemVer compatibility range. It also attempts to use
the greatest version currently available within that compatibility range. For
example, if there are two packages in the resolve graph with the following
requirements:

```toml
# Package A
[dependencies]
bitflags = "1.0"

# Package B
[dependencies]
bitflags = "1.1"
```

If at the time the `Cargo.lock` file is generated, the greatest version of
`bitflags` is `1.2.1`, then both packages will use `1.2.1` because it is the
greatest within the compatibility range. If `2.0.0` is published, it will
still use `1.2.1` because `2.0.0` is considered incompatible.

If multiple packages have a common dependency with semver-incompatible
versions, then Cargo will allow this, but will build two separate copies of
the dependency. For example:

```toml
# Package A
[dependencies]
rand = "0.7"

# Package B
[dependencies]
rand = "0.6"
```

The above will result in Package A using the greatest `0.7` release (`0.7.3`
at the time of this writing) and Package B will use the greatest `0.6` release
(`0.6.5` for example). This can lead to potential problems, see the
[Version-incompatibility hazards] section for more details.

Multiple versions within the same compatibility range are not allowed and will
result in a resolver error if it is constrained to two different versions
within a compatibility range. For example, if there are two packages in the
resolve graph with the following requirements:

```toml
# Package A
[dependencies]
log = "=0.4.11"

# Package B
[dependencies]
log = "=0.4.8"
```

The above will fail because it is not allowed to have two separate copies of
the `0.4` release of the `log` package.

[SemVer]: https://semver.org/
[SemVer Compatibility]: semver.md
[Version-incompatibility hazards]: #version-incompatibility-hazards

### Version-incompatibility hazards

When multiple versions of a crate appear in the resolve graph, this can cause
problems when types from those crates are exposed by the crates using them.
This is because the types and items are considered different by the Rust
compiler, even if they have the same name. Libraries should take care when
publishing a SemVer-incompatible version (for example, publishing `2.0.0`
after `1.0.0` has been in use), particularly for libraries that are widely
used.

The "[semver trick]" is a workaround for this problem of publishing a breaking
change while retaining compatibility with older versions. The linked page goes
into detail about what the problem is and how to address it. In short, when a
library wants to publish a SemVer-breaking release, publish the new release,
and also publish a point release of the previous version that reexports the
types from the newer version.

These incompatibilities usually manifest as a compile-time error, but
sometimes they will only appear as a runtime misbehavior. For example, let's
say there is a common library named `foo` that ends up appearing with both
version `1.0.0` and `2.0.0` in the resolve graph. If [`downcast_ref`] is used
on a object created by a library using version `1.0.0`, and the code calling
`downcast_ref` is downcasting to a type from version `2.0.0`, the downcast
will fail at runtime.

It is important to make sure that if you have multiple versions of a library
that you are properly using them, especially if it is ever possible for the
types from different versions to be used together. The [`cargo tree
-d`][`cargo tree`] command can be used to identify duplicate versions and
where they come from. Similarly, it is important to consider the impact on the
ecosystem if you publish a SemVer-incompatible version of a popular library.

[semver trick]: https://github.com/dtolnay/semver-trick
[`downcast_ref`]: ../../std/any/trait.Any.html#method.downcast_ref

### Pre-releases

SemVer has the concept of "pre-releases" with a dash in the version, such as
`1.0.0-alpha`, or `1.0.0-beta`. Cargo will avoid automatically using
pre-releases unless explicitly asked. For example, if `1.0.0-alpha` of package
`foo` is published, then a requirement of `foo = "1.0"` will *not* match, and
will return an error. The pre-release must be specified, such as `foo =
"1.0.0-alpha"`. Similarly [`cargo install`] will avoid pre-releases unless
explicitly asked to install one.

Cargo allows "newer" pre-releases to be used automatically. For example, if
`1.0.0-beta` is published, then a requirement `foo = "1.0.0-alpha"` will allow
updating to the `beta` version. Beware that pre-release versions can be
unstable, and as such care should be taken when using them. Some projects may
choose to publish breaking changes between pre-release versions. It is
recommended to not use pre-release dependencies in a library if your library
is not also a pre-release. Care should also be taken when updating your
`Cargo.lock`, and be prepared if a pre-release update causes issues.

The pre-release tag may be separated with periods to distinguish separate
components. Numeric components will use numeric comparison. For example,
`1.0.0-alpha.4` will use numeric comparison for the `4` component. That means
that if `1.0.0-alpha.11` is published, that will be chosen as the greatest
release. Non-numeric components are compared lexicographically.

[`cargo install`]: ../commands/cargo-install.md

### Version metadata

SemVer has the concept of "version metadata" with a plus in the version, such
as `1.0.0+21AF26D3`. This metadata is usually ignored, and should not be used
in a version requirement. You should never publish multiple versions that
differ only in the metadata tag.

## Other constraints

Version requirements aren't the only constraint that the resolver considers
when selecting and unifying dependencies. The following sections cover some of
the other constraints that can affect resolution.

### Features

For the purpose of generating `Cargo.lock`, the resolver builds the dependency
graph as-if all [features] of all [workspace] members are enabled. This
ensures that any optional dependencies are available and properly resolved
with the rest of the graph when features are added or removed with the
[`--features` command-line flag](features.md#command-line-feature-options).
The resolver runs a second time to determine the actual features used when
*compiling* a crate, based on the features selected on the command-line.

Dependencies are resolved with the union of all features enabled on them. For
example, if one package depends on the [`im`] package with the [`serde`
dependency] enabled and another package depends on it with the [`rayon`
dependency] enabled, then `im` will be built with both features enabled, and
the `serde` and `rayon` crates will be included in the resolve graph. If no
packages depend on `im` with those features, then those optional dependencies
will be ignored, and they will not affect resolution.

When building multiple packages in a workspace (such as with `--workspace` or
multiple `-p` flags), the features of the dependencies of all of those
packages are unified. If you have a circumstance where you want to avoid that
unification for different workspace members, you will need to build them via
separate `cargo` invocations.

The resolver will skip over versions of packages that are missing required
features. For example, if a package depends on version `^1` of [`regex`] with
the [`perf` feature], then the oldest version it can select is `1.3.0`,
because versions prior to that did not contain the `perf` feature. Similarly,
if a feature is removed from a new release, then packages that require that
feature will be stuck on the older releases that contain that feature. It is
discouraged to remove features in a SemVer-compatible release. Beware that
optional dependencies also define an implicit feature, so removing an optional
dependency or making it non-optional can cause problems, see [removing an
optional dependency].

[`im`]: https://crates.io/crates/im
[`perf` feature]: https://github.com/rust-lang/regex/blob/1.3.0/Cargo.toml#L56
[`rayon` dependency]: https://github.com/bodil/im-rs/blob/v15.0.0/Cargo.toml#L47
[`regex`]: https://crates.io/crates/regex
[`serde` dependency]: https://github.com/bodil/im-rs/blob/v15.0.0/Cargo.toml#L46
[features]: features.md
[removing an optional dependency]: semver.md#cargo-remove-opt-dep
[workspace]: workspaces.md

#### Feature resolver version 2

When `resolver = "2"` is specified in `Cargo.toml` (see [resolver
versions](#resolver-versions) below), a different feature resolver is used
which uses a different algorithm for unifying features. The version `"1"`
resolver will unify features for a package no matter where it is specified.
The version `"2"` resolver will avoid unifying features in the following
situations:

* Features for target-specific dependencies are not enabled if the target is
  not currently being built. For example:

  ```toml
  [dependencies.common]
  version = "1.0"
  features = ["f1"]

  [target.'cfg(windows)'.dependencies.common]
  version = "1.0"
  features = ["f2"]
  ```

  When building this example for a non-Windows platform, the `f2` feature will
  *not* be enabled.

* Features enabled on [build-dependencies] or proc-macros will not be unified
  when those same dependencies are used as a normal dependency. For example:

  ```toml
  [dependencies]
  log = "0.4"

  [build-dependencies]
  log = {version = "0.4", features=['std']}
  ```

  When building the build script, the `log` crate will be built with the `std`
  feature. When building the library of your package, it will not enable the
  feature.

* Features enabled on [dev-dependencies] will not be unified when those same
  dependencies are used as a normal dependency, unless those dev-dependencies
  are currently being built. For example:

  ```toml
  [dependencies]
  serde = {version = "1.0", default-features = false}

  [dev-dependencies]
  serde = {version = "1.0", features = ["std"]}
  ```

  In this example, the library will normally link against `serde` without the
  `std` feature. However, when built as a test or example, it will include the
  `std` feature. For example, `cargo test` or `cargo build --all-targets` will
  unify these features. Note that dev-dependencies in dependencies are always
  ignored, this is only relevant for the top-level package or workspace
  members.

[build-dependencies]: specifying-dependencies.md#build-dependencies
[dev-dependencies]: specifying-dependencies.md#development-dependencies
[resolver-field]: features.md#resolver-versions

### `links`

The [`links` field] is used to ensure only one copy of a native library is
linked into a binary. The resolver will attempt to find a graph where there is
only one instance of each `links` name. If it is unable to find a graph that
satisfies that constraint, it will return an error.

For example, it is an error if one package depends on [`libgit2-sys`] version
`0.11` and another depends on `0.12`, because Cargo is unable to unify those,
but they both link to the `git2` native library. Due to this requirement, it
is encouraged to be very careful when making SemVer-incompatible releases with
the `links` field if your library is in common use.

[`links` field]: manifest.md#the-links-field
[`libgit2-sys`]: https://crates.io/crates/libgit2-sys

### Yanked versions

[Yanked releases][yank] are those that are marked that they should not be
used. When the resolver is building the graph, it will ignore all yanked
releases unless they already exist in the `Cargo.lock` file.

[yank]: publishing.md#cargo-yank

## Dependency updates

Dependency resolution is automatically performed by all Cargo commands that
need to know about the dependency graph. For example, [`cargo build`] will run
the resolver to discover all the dependencies to build. After the first time
it runs, the result is stored in the `Cargo.lock` file. Subsequent commands
will run the resolver, keeping dependencies locked to the versions in
`Cargo.lock` *if it can*.

If the dependency list in `Cargo.toml` has been modified, for example changing
the version of a dependency from `1.0` to `2.0`, then the resolver will select
a new version for that dependency that matches the new requirements. If that
new dependency introduces new requirements, those new requirements may also
trigger additional updates. The `Cargo.lock` file will be updated with the new
result. The `--locked` or `--frozen` flags can be used to change this behavior
to prevent automatic updates when requirements change, and return an error
instead.

[`cargo update`] can be used to update the entries in `Cargo.lock` when new
versions are published. Without any options, it will attempt to update all
packages in the lock file. The `-p` flag can be used to target the update for
a specific package, and other flags such as `--recursive` or `--precise` can
be used to control how versions are selected.

[`cargo build`]: ../commands/cargo-build.md
[`cargo update`]: ../commands/cargo-update.md

## Overrides

Cargo has several mechanisms to override dependencies within the graph. The
[Overriding Dependencies] chapter goes into detail on how to use overrides.
The overrides appear as an overlay to a registry, replacing the patched
version with the new entry. Otherwise, resolution is performed like normal.

[Overriding Dependencies]: overriding-dependencies.md

## Dependency kinds

There are three kinds of dependencies in a package: normal, [build], and
[dev][dev-dependencies]. For the most part these are all treated the same from
the perspective of the resolver. One difference is that dev-dependencies for
non-workspace members are always ignored, and do not influence resolution.

[Platform-specific dependencies] with the `[target]` table are resolved as-if
all platforms are enabled. In other words, the resolver ignores the platform
or `cfg` expression.

[build]: specifying-dependencies.md#build-dependencies
[dev-dependencies]: specifying-dependencies.md#development-dependencies
[Platform-specific dependencies]: specifying-dependencies.md#platform-specific-dependencies

### dev-dependency cycles

Usually the resolver does not allow cycles in the graph, but it does allow
them for [dev-dependencies]. For example, project "foo" has a dev-dependency
on "bar", which has a normal dependency on "foo" (usually as a "path"
dependency). This is allowed because there isn't really a cycle from the
perspective of the build artifacts. In this example, the "foo" library is
built (which does not need "bar" because "bar" is only used for tests), and
then "bar" can be built depending on "foo", then the "foo" tests can be built
linking to "bar".

Beware that this can lead to confusing errors. In the case of building library
unit tests, there are actually two copies of the library linked into the final
test binary: the one that was linked with "bar", and the one built that
contains the unit tests. Similar to the issues highlighted in the
[Version-incompatibility hazards] section, the types between the two are not
compatible. Be careful when exposing types of "foo" from "bar" in this
situation, since the "foo" unit tests won't treat them the same as the local
types.

If possible, try to split your package into multiple packages and restructure
it so that it remains strictly acyclic.

## Resolver versions

A different feature resolver algorithm can be used by specifying the resolver
version in `Cargo.toml` like this:

```toml
[package]
name = "my-package"
version = "1.0.0"
resolver = "2"
```

The version `"1"` resolver is the original resolver that shipped with Cargo up to version 1.50.
The default is `"2"` if the root package specifies [`edition = "2021"`](manifest.md#the-edition-field) or a newer edition.
Otherwise the default is `"1"`.

The version `"2"` resolver introduces changes in [feature
unification](#features). See the [features chapter][features-2] for more
details.

The resolver is a global option that affects the entire workspace. The
`resolver` version in dependencies is ignored, only the value in the top-level
package will be used. If using a [virtual workspace], the version should be
specified in the `[workspace]` table, for example:

```toml
[workspace]
members = ["member1", "member2"]
resolver = "2"
```

[virtual workspace]: workspaces.md#virtual-workspace
[features-2]: features.md#feature-resolver-version-2

## Recommendations

The following are some recommendations for setting the version within your
package, and for specifying dependency requirements. These are general
guidelines that should apply to common situations, but of course some
situations may require specifying unusual requirements.

* Follow the [SemVer guidelines] when deciding how to update your version
  number, and whether or not you will need to make a SemVer-incompatible
  version change.
* Use caret requirements for dependencies, such as `"1.2.3"`, for most
  situations. This ensures that the resolver can be maximally flexible in
  choosing a version while maintaining build compatibility.
  * Specify all three components with the version you are currently using.
    This helps set the minimum version that will be used, and ensures that
    other users won't end up with an older version of the dependency that
    might be missing something that your package requires.
  * Avoid `*` requirements, as they are not allowed on [crates.io], and they
    can pull in SemVer-breaking changes during a normal `cargo update`.
  * Avoid overly broad version requirements. For example, `>=2.0.0` can pull
    in any SemVer-incompatible version, like version `5.0.0`, which can result
    in broken builds in the future.
  * Avoid overly narrow version requirements if possible. For example, if you
    specify a tilde requirement like `bar="~1.3"`, and another package
    specifies a requirement of `bar="1.4"`, this will fail to resolve, even
    though minor releases should be compatible.
* Try to keep the dependency versions up-to-date with the actual minimum
  versions that your library requires. For example, if you have a requirement
  of `bar="1.0.12"`, and then in a future release you start using new features
  added in the `1.1.0` release of "bar", update your dependency requirement to
  `bar="1.1.0"`.

  If you fail to do this, it may not be immediately obvious because Cargo can
  opportunistically choose the newest version when you run a blanket `cargo
  update`. However, if another user depends on your library, and runs `cargo
  update your-library`, it will *not* automatically update "bar" if it is
  locked in their `Cargo.lock`. It will only update "bar" in that situation if
  the dependency declaration is also updated. Failure to do so can cause
  confusing build errors for the user using `cargo update your-library`.
* If two packages are tightly coupled, then an `=` dependency requirement may
  help ensure that they stay in sync. For example, a library with a companion
  proc-macro library will sometimes make assumptions between the two libraries
  that won't work well if the two are out of sync (and it is never expected to
  use the two libraries independently). The parent library can use an `=`
  requirement on the proc-macro, and re-export the macros for easy access.
* `0.0.x` versions can be used for packages that are permanently unstable.

In general, the stricter you make the dependency requirements, the more likely
it will be for the resolver to fail. Conversely, if you use requirements that
are too loose, it may be possible for new versions to be published that will
break the build.

[SemVer guidelines]: semver.md
[crates.io]: https://crates.io/

## Troubleshooting

The following illustrates some problems you may experience, and some possible
solutions.

### Unexpected dependency duplication

The resolver algorithm may converge on a solution that includes two copies of a
dependency when one would suffice. For example:

```toml
# Package A
[dependencies]
rand = "0.7"

# Package B
[dependencies]
rand = ">=0.6"  # note: open requirements such as this are discouraged
```

In this example, Cargo may build two copies of the `rand` crate, even though a
single copy at version `0.7.3` would meet all requirements. This is because the
resolver's algorithm favors building the latest available version of `rand` for
Package B, which is `0.8.5` at the time of this writing, and that is
incompatible with Package A's specification. The resolver's algorithm does not
currently attempt to "deduplicate" in this situation.

The use of open-ended version requirements like `>=0.6` is discouraged in Cargo.
But, if you run into this situation, the [`cargo update`] command with the
`--precise` flag can be used to manually remove such duplications.

[`cargo update`]: ../commands/cargo-update.md


### SemVer-breaking patch release breaks the build

Sometimes a project may inadvertently publish a point release with a
SemVer-breaking change. When users update with `cargo update`, they will pick
up this new release, and then their build may break. In this situation, it is
recommended that the project should [yank] the release, and either remove the
SemVer-breaking change, or publish it as a new SemVer-major version increase.

If the change happened in a third-party project, if possible try to
(politely!) work with the project to resolve the issue.

While waiting for the release to be yanked, some workarounds depend on the
circumstances:

* If your project is the end product (such as a binary executable), just avoid
  updating the offending package in `Cargo.lock`. This can be done with the
  `--precise` flag in [`cargo update`].
* If you publish a binary on [crates.io], then you can temporarily add an `=`
  requirement to force the dependency to a specific good version.
  * Binary projects can alternatively recommend users to use the `--locked`
    flag with [`cargo install`] to use the original `Cargo.lock` that contains
    the known good version.
* Libraries may also consider publishing a temporary new release with stricter
  requirements that avoid the troublesome dependency. You may want to consider
  using range requirements (instead of `=`) to avoid overly-strict
  requirements that may conflict with other packages using the same
  dependency. Once the problem has been resolved, you can publish another
  point release that relaxes the dependency back to a caret requirement.
* If it looks like the third-party project is unable or unwilling to yank the
  release, then one option is to update your code to be compatible with the
  changes, and update the dependency requirement to set the minimum version to
  the new release. You will also need to consider if this is a SemVer-breaking
  change of your own library, for example if it exposes types from the
  dependency.

