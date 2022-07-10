# Packages and Resolution

## Workspaces

The [`Workspace`] object is usually created very early by calling the
[`workspace`][ws-method] helper method. This discovers the root of the
workspace, and loads all the workspace members as a [`Package`] object. Each
package corresponds to a single `Cargo.toml` (which is deserialized into a
[`Manifest`]), and may define several [`Target`]s, such as the library,
binaries, integration test or examples. Targets are crates (each target
defines a crate root, like `src/lib.rs` or `examples/foo.rs`) and are what is
actually compiled by `rustc`.

## Packages and Sources

There are several data structures that are important to understand how
packages are found and loaded:

* [`Package`] — A package, which is a `Cargo.toml` manifest and its associated
  source files.
    * [`PackageId`] — A unique identifier for a package.
* [`Source`] — An abstraction for something that can fetch packages (a remote
  registry, a git repo, the local filesystem, etc.). Check out the [source
  implementations] for all the details about registries, indexes, git
  dependencies, etc.
    * [`SourceId`] — A unique identifier for a source.
* [`SourceMap`] — Map of all available sources.
* [`PackageRegistry`] — This is the main interface for how the dependency
  resolver finds packages. It contains the `SourceMap`, and handles things
  like the `[patch]` table. The `Registry` trait provides a generic interface
  to the `PackageRegistry`, but this is only used for providing an alternate
  implementation of the `PackageRegistry` for testing. The dependency resolver
  sends a query to the `PackageRegistry` to "get me all packages that match
  this dependency declaration".
* [`Summary`] — A summary is a subset of a [`Manifest`], and is essentially
  the information that can be found in a registry index. Queries against the
  `PackageRegistry` yields a `Summary`. The resolver uses the summary
  information to build the dependency graph.
* [`PackageSet`] — Contains all of the `Package` objects. This works with the
  [`Downloads`] struct to coordinate downloading packages. It has a reference
  to the `SourceMap` to get the `Source` objects which tell the `Downloads`
  struct which URLs to fetch.

All of these come together in the [`ops::resolve`] module. This module
contains the primary functions for performing resolution (described below). It
also handles downloading of packages. It is essentially where all of the data
structures above come together.

## Resolver

[`Resolve`] is the representation of a directed graph of package dependencies,
which uses [`PackageId`]s for nodes. This is the data structure that is saved
to the `Cargo.lock` file. If there is no lock file, Cargo constructs a resolve
by finding a graph of packages which matches declared dependency specification
according to SemVer.

[`ops::resolve`] is the front-end for creating a `Resolve`. It handles loading
the `Cargo.lock` file, checking if it needs updating, etc.

Resolution is currently performed twice. It is performed once with all
features enabled. This is the resolve that gets saved to `Cargo.lock`. It then
runs again with only the specific features the user selected on the
command-line. Ideally this second run will get removed in the future when
transitioning to the new feature resolver.

### Feature resolver

A new feature-specific resolver was added in 2020 which adds more
sophisticated feature resolution. It is located in the [`resolver::features`]
module. The original dependency resolver still performs feature unification,
as it can help reduce the dependencies it has to consider during resolution
(rather than assuming every optional dependency of every package is enabled).
Checking if a feature is enabled must go through the new feature resolver.


[`Workspace`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/workspace.rs
[ws-method]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/util/command_prelude.rs#L298-L318
[`Package`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/package.rs
[`Target`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/core/manifest.rs#L181-L206
[`Manifest`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/core/manifest.rs#L27-L51
[`Source`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/source/mod.rs
[`SourceId`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/source/source_id.rs
[`SourceMap`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/core/source/mod.rs#L245-L249
[`PackageRegistry`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/core/registry.rs#L36-L81
[`ops::resolve`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/ops/resolve.rs
[`resolver::features`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/resolver/features.rs#L259
[source implementations]: https://github.com/rust-lang/cargo/tree/master/src/cargo/sources
[`PackageId`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/package_id.rs
[`Summary`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/summary.rs
[`PackageSet`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/core/package.rs#L283-L296
[`Downloads`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/src/cargo/core/package.rs#L298-L352
[`Resolve`]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/resolver/resolve.rs
