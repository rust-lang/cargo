# Glossary

### Artifact

An *artifact* is the file or set of files created as a result of the
compilation process. This includes linkable libraries and executable binaries.

### Crate

Every target in a package is a *crate*. Crates are either libraries or
executable binaries. It may loosely refer to either the source code of the
target, or the compiled artifact that the target produces. A crate may also
refer to a compressed package fetched from a registry.

### Edition

A *Rust edition* is a developmental landmark of the Rust language. The
[edition of a package][edition-field] is specified in the `Cargo.toml`
manifest, and individual targets can specify which edition they use. See the
[Edition Guide] for more information.

### Feature

The meaning of *feature* depends on the context:

- A [*feature*][feature] is a named flag which allows for conditional
  compilation. A feature can refer to an optional dependency, or an arbitrary
  name defined in a `Cargo.toml` manifest that can be checked within source
  code.

- Cargo has [*unstable feature flags*][cargo-unstable] which can be used to
  enable experimental behavior of Cargo itself.

- The Rust compiler and Rustdoc have their own unstable feature flags (see
  [The Unstable Book][unstable-book] and [The Rustdoc
  Book][rustdoc-unstable]).

- CPU targets have [*target features*][target-feature] which specify
  capabilities of a CPU.

### Index

The index is the searchable list of crates in a registry.

### Lock file

The `Cargo.lock` *lock file* is a file that captures the exact version of
every dependency used in a workspace or package. It is automatically generated
by Cargo. See [Cargo.toml vs Cargo.lock].

### Manifest

A [*manifest*][manifest] is a description of a package or a workspace in a
file named `Cargo.toml`.

A [*virtual manifest*][virtual] is a `Cargo.toml` file that only describes a
workspace, and does not include a package.

### Member

A *member* is a package that belongs to a workspace.

### Package

A *package* is a collection of source files and a `Cargo.toml` manifest which
describes the package. A package has a name and version which is used for
specifying dependencies between packages. A package contains multiple targets,
which are either libraries or executable binaries.

The *package root* is the directory where the package's `Cargo.toml` manifest
is located.

The [*package ID specification*][pkgid-spec], or *SPEC*, is a string used to
uniquely reference a specific version of a package from a specific source.

### Project

Another name for a [package](#package).

### Registry

A *registry* is a service that contains a collection of downloadable crates
that can be installed or used as dependencies for a package. The default
registry is [crates.io](https://crates.io). The registry has an *index* which
contains a list of all crates, and tells Cargo how to download the crates that
are needed.

### Source

A *source* is a provider that contains crates that may be included as
dependencies for a package. There are several kinds of sources:

- **Registry source** — See [registry](#registry).
- **Local registry source** — A set of crates stored as compressed files on
  the filesystem. See [Local Registry Sources].
- **Directory source** — A set of crates stored as uncompressed files on the
  filesystem. See [Directory Sources].
- **Path source** — An individual package located on the filesystem (such as a
  [path dependency]) or a set of multiple packages (such as [path overrides]).
- **Git source** — Packages located in a git repository (such as a [git
  dependency] or [git source]).

See [Source Replacement] for more information.

### Spec

See [package ID specification](#package).

### Target

The meaning of the term *target* depends on the context:

- **Cargo Target** — Cargo packages consist of *targets* which correspond to
  artifacts that will be produced. Packages can have library, binary, example,
  test, and benchmark targets. The [list of targets][targets] are configured
  in the `Cargo.toml` manifest, often inferred automatically by the [directory
  layout] of the source files.
- **Target Directory** — Cargo places all built artifacts and intermediate
  files in the *target* directory. By default this is a directory named
  `target` at the workspace root, or the package root if not using a
  workspace. The directory may be changed with the `--target-dir` command-line
  option, the `CARGO_TARGET_DIR` [environment variable], or the
  `build.target-dir` [config option].
- **Target Architecture** — The OS and machine architecture for the built
  artifacts are typically referred to as a *target*.
- **Target Triple** — A triple is a specific format for specifying a target
  architecture. Triples may be referred to as a *target triple* which is the
  architecture for the artifact produced, and the *host triple* which is the
  architecture that the compiler is running on. The target triple can be
  specified with the `--target` command-line option or the `build.target`
  [config option]. The general format of the triple is
  `<arch><sub>-<vendor>-<sys>-<abi>` where:

  - `arch` = The base CPU architecture, for example `x86_64`, `i686`, `arm`,
    `thumb`, `mips`, etc.
  - `sub` = The CPU sub-architecture, for example `arm` has `v7`, `v7s`,
    `v5te`, etc.
  - `vendor` = The vendor, for example `unknown`, `apple`, `pc`, `linux`, etc.
  - `sys` = The system name, for example `linux`, `windows`, etc. `none` is
    typically used for bare-metal without an OS.
  - `abi` = The ABI, for example `gnu`, `android`, `eabi`, etc.

  Some parameters may be omitted. Run `rustc --print target-list` for a list of
  supported targets.

### Test Targets

Cargo *test targets* generate binaries which help verify proper operation and
correctness of code. There are two types of test artifacts:

* **Unit test** — A *unit test* is an executable binary compiled directly from
  a library or a binary target. It contains the entire contents of the library
  or binary code, and runs `#[test]` annotated functions, intended to verify
  individual units of code.
* **Integration test target** — An [*integration test
  target*][integration-tests] is an executable binary compiled from a *test
  target* which is a distinct crate whose source is located in the `tests`
  directory or specified by the [`[[test]]` table][targets] in the
  `Cargo.toml` manifest. It is intended to only test the public API of a
  library, or execute a binary to verify its operation.

### Workspace

A [*workspace*][workspace] is a collection of one or more packages that share
common dependency resolution (with a shared `Cargo.lock`), output directory,
and various settings such as profiles.

A [*virtual workspace*][virtual] is a workspace where the root `Cargo.toml`
manifest does not define a package, and only lists the workspace members.

The *workspace root* is the directory where the workspace's `Cargo.toml`
manifest is located.


[Cargo.toml vs Cargo.lock]: guide/cargo-toml-vs-cargo-lock.html
[Directory Sources]: reference/source-replacement.html#directory-sources
[Local Registry Sources]: reference/source-replacement.html#local-registry-sources
[Source Replacement]: reference/source-replacement.html
[cargo-unstable]: reference/unstable.html
[config option]: reference/config.html
[directory layout]: reference/manifest.html#the-project-layout
[edition guide]: ../edition-guide/index.html
[edition-field]: reference/manifest.html#the-edition-field-optional
[environment variable]: reference/environment-variables.html
[feature]: reference/manifest.html#the-features-section
[git dependency]: reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories
[git source]: reference/source-replacement.html
[integration-tests]: reference/manifest.html#integration-tests
[manifest]: reference/manifest.html
[path dependency]: reference/specifying-dependencies.html#specifying-path-dependencies
[path overrides]: reference/specifying-dependencies.html#overriding-with-local-dependencies
[pkgid-spec]: reference/pkgid-spec.html
[rustdoc-unstable]: https://doc.rust-lang.org/nightly/rustdoc/unstable-features.html
[target-feature]: ../reference/attributes/codegen.html#the-target_feature-attribute
[targets]: reference/manifest.html#configuring-a-target
[unstable-book]: https://doc.rust-lang.org/nightly/unstable-book/index.html
[virtual]: reference/manifest.html#virtual-manifest
[workspace]: reference/manifest.html#the-workspace-section
