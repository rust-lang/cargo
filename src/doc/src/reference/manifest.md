## The Manifest Format

The `Cargo.toml` file for each package is called its *manifest*. Every manifest
file consists of one or more sections.

<a id="package-metadata"></a>
### The `[package]` section

The first section in a `Cargo.toml` is `[package]`.

```toml
[package]
name = "hello_world" # the name of the package
version = "0.1.0"    # the current version, obeying semver
authors = ["Alice <a@example.com>", "Bob <b@example.com>"]
```

The only fields required by Cargo are [`name`](#the-name-field) and
[`version`](#the-version-field). If publishing to a registry, the registry may
require additional fields. See the notes below and [the publishing
chapter][publishing] for requirements for publishing to [crates.io].

#### The `name` field

The package name is an identifier used to refer to the package. It is used
when listed as a dependency in another package, and as the default name of
inferred lib and bin targets.

The name must use only [alphanumeric] characters or `-` or `_`, and cannot be empty.
Note that [`cargo new`] and [`cargo init`] impose some additional restrictions on
the package name, such as enforcing that it is a valid Rust identifier and not
a keyword. [crates.io] imposes even more restrictions, such as
enforcing only ASCII characters, not a reserved name, not a special Windows
name such as "nul", is not too long, etc.

[alphanumeric]: ../../std/primitive.char.html#method.is_alphanumeric

#### The `version` field

Cargo bakes in the concept of [Semantic
Versioning](https://semver.org/), so make sure you follow some basic rules:

* Before you reach 1.0.0, anything goes, but if you make breaking changes,
  increment the minor version. In Rust, breaking changes include adding fields to
  structs or variants to enums.
* After 1.0.0, only make breaking changes when you increment the major version.
  Don’t break the build.
* After 1.0.0, don’t add any new public API (no new `pub` anything) in patch-level
  versions. Always increment the minor version if you add any new `pub` structs,
  traits, fields, types, functions, methods or anything else.
* Use version numbers with three numeric parts such as 1.0.0 rather than 1.0.

#### The `authors` field

The `authors` field lists people or organizations that are considered the
"authors" of the package. The exact meaning is open to interpretation — it may
list the original or primary authors, current maintainers, or owners of the
package. These names will be listed on the crate's page on
[crates.io]. An optional email address may be included within angled
brackets at the end of each author.

> **Note**: [crates.io] requires at least one author to be listed.

#### The `edition` field

You can opt in to a specific [Rust Edition] for your package with the
`edition` key in `Cargo.toml`. If you don't specify the edition, it will
default to 2015.

```toml
[package]
# ...
edition = '2018'
```

The `edition` key affects which edition your package is compiled with. Cargo
will always generate packages via [`cargo new`] with the `edition` key set to the
latest edition. Setting the `edition` key in `[package]` will affect all
targets/crates in the package, including test suites, benchmarks, binaries,
examples, etc.

#### The `description` field

The description is a short blurb about the package. [crates.io] will display
this with your package. This should be plain text (not Markdown).

```toml
[package]
# ...
description = "A short description of my package"
```

> **Note**: [crates.io] requires the `description` to be set.

#### The `documentation` field

The `documentation` field specifies a URL to a website hosting the crate's
documentation. If no URL is specified in the manifest file, [crates.io] will
automatically link your crate to the corresponding [docs.rs] page.

```toml
[package]
# ...
documentation = "https://docs.rs/bitflags"
```

> **Note**: [crates.io] may not show certain sites if they are known to not be
> hosting documentation and are possibly of malicious intent e.g., ad tracking
> networks. At this time, the site `rust-ci.org` is not allowed.

#### The `readme` field

The `readme` field should be the path to a file in the package root (relative
to this `Cargo.toml`) that contains general information about the package.
This file will be transferred to the registry when you publish. [crates.io]
will interpret it as Markdown and render it on the crate's page.

```toml
[package]
# ...
readme = "README.md"
```

#### The `homepage` field

The `homepage` field should be a URL to a site that is the home page for your
package.

```toml
[package]
# ...
homepage = "https://serde.rs/"
```

#### The `repository` field

The `repository` field should be a URL to the source repository for your
package.

```toml
[package]
# ...
repository = "https://github.com/rust-lang/cargo/"
```

#### The `license` and `license-file` fields

The `license` field contains the name of the software license that the package
is released under. The `license-file` field contains the path to a file
containing the text of the license (relative to this `Cargo.toml`).

[crates.io] interprets the `license` field as an [SPDX 2.1 license
expression][spdx-2.1-license-expressions]. The name must be a known license
from the [SPDX license list 3.6][spdx-license-list-3.6]. Parentheses are not
currently supported. See the [SPDX site] for more information.

SPDX license expressions support AND and OR operators to combine multiple
licenses.[^slash]

```toml
[package]
# ...
license = "MIT OR Apache-2.0"
```

Using `OR` indicates the user may choose either license. Using `AND` indicates
the user must comply with both licenses simultaneously. The `WITH` operator
indicates a license with a special exception. Some examples:

* `MIT OR Apache-2.0`
* `LGPL-2.1 AND MIT AND BSD-2-Clause`
* `GPL-2.0+ WITH Bison-exception-2.2`

If a package is using a nonstandard license, then the `license-file` field may
be specified in lieu of the `license` field.

```toml
[package]
# ...
license-file = "LICENSE.txt"
```

> **Note**: [crates.io] requires either `license` or `license-file` to be set.

[^slash]: Previously multiple licenses could be separated with a `/`, but that
usage is deprecated.

#### The `keywords` field

The `keywords` field is an array of strings that describe this package. This
can help when searching for the package on a registry, and you may choose any
words that would help someone find this crate.

```toml
[package]
# ...
keywords = ["gamedev", "graphics"]
```

> **Note**: [crates.io] has a maximum of 5 keywords. Each keyword must be
> ASCII text, start with a letter, and only contain letters, numbers, `_` or
> `-`, and have at most 20 characters.

#### The `categories` field

The `categories` field is an array of strings of the categories this package
belongs to.

```toml
categories = ["command-line-utilities", "development-tools::cargo-plugins"]
```

> **Note**: [crates.io] has a maximum of 5 categories. Each category should
> match one of the strings available at <https://crates.io/category_slugs>, and
> must match exactly.

#### The `workspace` field

The `workspace` field can be used to configure the workspace that this package
will be a member of. If not specified this will be inferred as the first
Cargo.toml with `[workspace]` upwards in the filesystem.

```toml
[package]
# ...
workspace = "path/to/workspace/root"
```

For more information, see the documentation for the [workspace table
below](#the-workspace-section).

<a id="package-build"></a>
#### The `build` field

The `build` field specifies a file in the package root which is a [build
script] for building native code. More information can be found in the [build
script guide][build script].

[build script]: build-scripts.md

```toml
[package]
# ...
build = "build.rs"
```

The default is `"build.rs"`, which loads the script from a file named
`build.rs` in the root of the package. Use `build = "custom_build_name.rs"` to
specify a path to a different file or `build = false` to disable automatic
detection of the build script.

#### The `links` field

The `links` field specifies the name of a native library that is being linked
to. More information can be found in the [`links`][links] section of the build
script guide.

[links]: build-scripts.md#the-links-manifest-key

```toml
[package]
# ...
links = "foo"
```

#### The `exclude` and `include` fields

You can explicitly specify that a set of file patterns should be ignored or
included for the purposes of packaging. The patterns specified in the
`exclude` field identify a set of files that are not included, and the
patterns in `include` specify files that are explicitly included.

The patterns should be [gitignore]-style patterns. Briefly:

- `foo` matches any file or directory with the name `foo` anywhere in the
  package. This is equivalent to the pattern `**/foo`.
- `/foo` matches any file or directory with the name `foo` only in the root of
  the package.
- `foo/` matches any *directory* with the name `foo` anywhere in the package.
- Common glob patterns like `*`, `?`, and `[]` are supported:
  - `*` matches zero or more characters except `/`.  For example, `*.html`
    matches any file or directory with the `.html` extension anywhere in the
    package.
  - `?` matches any character except `/`. For example, `foo?` matches `food`,
    but not `foo`.
  - `[]` allows for matching a range of characters. For example, `[ab]`
    matches either `a` or `b`. `[a-z]` matches letters a through z.
- `**/` prefix matches in any directory. For example, `**/foo/bar` matches the
  file or directory `bar` anywhere that is directly under directory `foo`.
- `/**` suffix matches everything inside. For example, `foo/**` matches all
  files inside directory `foo`, including all files in subdirectories below
  `foo`.
- `/**/` matches zero or more directories. For example, `a/**/b` matches
  `a/b`, `a/x/b`, `a/x/y/b`, and so on.
- `!` prefix negates a pattern. For example, a pattern of `src/**.rs` and
  `!foo.rs` would match all files with the `.rs` extension inside the `src`
  directory, except for any file named `foo.rs`.

If git is being used for a package, the `exclude` field will be seeded with
the `gitignore` settings from the repository.

```toml
[package]
# ...
exclude = ["build/**/*.o", "doc/**/*.html"]
```

```toml
[package]
# ...
include = ["src/**/*", "Cargo.toml"]
```

The options are mutually exclusive: setting `include` will override an
`exclude`. Note that `include` must be an exhaustive list of files as otherwise
necessary source files may not be included. The package's `Cargo.toml` is
automatically included.

The include/exclude list is also used for change tracking in some situations.
For targets built with `rustdoc`, it is used to determine the list of files to
track to determine if the target should be rebuilt. If the package has a
[build script] that does not emit any `rerun-if-*` directives, then the
include/exclude list is used for tracking if the build script should be re-run
if any of those files change.

[gitignore]: https://git-scm.com/docs/gitignore

#### The `publish` field

The `publish` field can be used to prevent a package from being published to a
package registry (like *crates.io*) by mistake, for instance to keep a package
private in a company.

```toml
[package]
# ...
publish = false
```

The value may also be an array of strings which are registry names that are
allowed to be published to.

```toml
[package]
# ...
publish = ["some-registry-name"]
```

### The `[badges]` section

[crates.io] can display various badges for build status, test coverage, etc. for
each crate. All badges are optional.

- The badges pertaining to build status that are currently available are
  Appveyor, CircleCI, Cirrus CI, GitLab, Azure DevOps, Travis CI and Bitbucket
  Pipelines.
- Available badges pertaining to code test coverage are Codecov and Coveralls.
- There are also maintenance-related badges based on isitmaintained.com
  which state the issue resolution time, percent of open issues, and future
  maintenance intentions.

Most badge specifications require a `repository` key. It is expected to be in
`user/repo` format.

```toml
[badges]

# Appveyor: `repository` is required. `branch` is optional; default is `master`
# `service` is optional; valid values are `github` (default), `bitbucket`, and
# `gitlab`; `id` is optional; you can specify the appveyor project id if you
# want to use that instead. `project_name` is optional; use when the repository
# name differs from the appveyor project name.
appveyor = { repository = "...", branch = "master", service = "github" }

# Circle CI: `repository` is required. `branch` is optional; default is `master`
circle-ci = { repository = "...", branch = "master" }

# Cirrus CI: `repository` is required. `branch` is optional; default is `master`
cirrus-ci = { repository = "...", branch = "master" }

# GitLab: `repository` is required. `branch` is optional; default is `master`
gitlab = { repository = "...", branch = "master" }

# Azure DevOps: `project` is required. `pipeline` is required. `build` is optional; default is `1`
# Note: project = `organization/project`, pipeline = `name_of_pipeline`, build = `definitionId`
azure-devops = { project = "...", pipeline = "...", build="2" }

# Travis CI: `repository` in format "<user>/<project>" is required.
# `branch` is optional; default is `master`
travis-ci = { repository = "...", branch = "master" }

# Bitbucket Pipelines: `repository` is required. `branch` is required
bitbucket-pipelines = { repository = "...", branch = "master" }

# Codecov: `repository` is required. `branch` is optional; default is `master`
# `service` is optional; valid values are `github` (default), `bitbucket`, and
# `gitlab`.
codecov = { repository = "...", branch = "master", service = "github" }

# Coveralls: `repository` is required. `branch` is optional; default is `master`
# `service` is optional; valid values are `github` (default) and `bitbucket`.
coveralls = { repository = "...", branch = "master", service = "github" }

# Is it maintained resolution time: `repository` is required.
is-it-maintained-issue-resolution = { repository = "..." }

# Is it maintained percentage of open issues: `repository` is required.
is-it-maintained-open-issues = { repository = "..." }

# Maintenance: `status` is required. Available options are:
# - `actively-developed`: New features are being added and bugs are being fixed.
# - `passively-maintained`: There are no plans for new features, but the maintainer intends to
#   respond to issues that get filed.
# - `as-is`: The crate is feature complete, the maintainer does not intend to continue working on
#   it or providing support, but it works for the purposes it was designed for.
# - `experimental`: The author wants to share it with the community but is not intending to meet
#   anyone's particular use case.
# - `looking-for-maintainer`: The current maintainer would like to transfer the crate to someone
#   else.
# - `deprecated`: The maintainer does not recommend using this crate (the description of the crate
#   can describe why, there could be a better solution available or there could be problems with
#   the crate that the author does not want to fix).
# - `none`: Displays no badge on crates.io, since the maintainer has not chosen to specify
#   their intentions, potential crate users will need to investigate on their own.
maintenance = { status = "..." }
```

#### The `metadata` table

Cargo by default will warn about unused keys in `Cargo.toml` to assist in
detecting typos and such. The `package.metadata` table, however, is completely
ignored by Cargo and will not be warned about. This section can be used for
tools which would like to store package configuration in `Cargo.toml`. For
example:

```toml
[package]
name = "..."
# ...

# Metadata used when generating an Android APK, for example.
[package.metadata.android]
package-name = "my-awesome-android-app"
assets = "path/to/static"
```

#### The `default-run` field

The `default-run` field in the `[package]` section of the manifest can be used
to specify a default binary picked by [`cargo run`]. For example, when there is
both `src/bin/a.rs` and `src/bin/b.rs`:

```toml
[package]
default-run = "a"
```

### Dependency sections

See the [specifying dependencies page](specifying-dependencies.md) for
information on the `[dependencies]`, `[dev-dependencies]`,
`[build-dependencies]`, and target-specific `[target.*.dependencies]` sections.

### The `[profile.*]` sections

The `[profile]` tables provide a way to customize compiler settings such as
optimizations and debug settings. See [the Profiles chapter](profiles.md) for
more detail.

### The `[features]` section

Cargo supports features to allow expression of:

* conditional compilation options (usable through `cfg` attributes);
* optional dependencies, which enhance a package, but are not required; and
* clusters of optional dependencies, such as `postgres-all`, that would include the
  `postgres` package, the `postgres-macros` package, and possibly other packages
  (such as development-time mocking libraries, debugging tools, etc.).

A feature of a package is either an optional dependency, or a set of other
features. The format for specifying features is:

```toml
[package]
name = "awesome"

[features]
# The default set of optional packages. Most people will want to use these
# packages, but they are strictly optional. Note that `session` is not a package
# but rather another feature listed in this manifest.
default = ["jquery", "uglifier", "session"]

# A feature with no dependencies is used mainly for conditional compilation,
# like `#[cfg(feature = "go-faster")]`.
go-faster = []

# The `secure-password` feature depends on the bcrypt package. This aliasing
# will allow people to talk about the feature in a higher-level way and allow
# this package to add more requirements to the feature in the future.
secure-password = ["bcrypt"]

# Features can be used to reexport features of other packages. The `session`
# feature of package `awesome` will ensure that the `session` feature of the
# package `cookie` is also enabled.
session = ["cookie/session"]

[dependencies]
# These packages are mandatory and form the core of this package’s distribution.
cookie = "1.2.0"
oauth = "1.1.0"
route-recognizer = "=2.1.0"

# A list of all of the optional dependencies, some of which are included in the
# above `features`. They can be opted into by apps.
jquery = { version = "1.0.2", optional = true }
uglifier = { version = "1.5.3", optional = true }
bcrypt = { version = "*", optional = true }
civet = { version = "*", optional = true }
```

To use the package `awesome`:

```toml
[dependencies.awesome]
version = "1.3.5"
default-features = false # do not include the default features, and optionally
                         # cherry-pick individual features
features = ["secure-password", "civet"]
```

#### Rules

The usage of features is subject to a few rules:

* Feature names must not conflict with other package names in the manifest. This
  is because they are opted into via `features = [...]`, which only has a single
  namespace.
* With the exception of the `default` feature, all features are opt-in. To opt
  out of the default feature, use `default-features = false` and cherry-pick
  individual features.
* Feature groups are not allowed to cyclically depend on one another.
* Dev-dependencies cannot be optional.
* Features groups can only reference optional dependencies.
* When a feature is selected, Cargo will call `rustc` with `--cfg
  feature="${feature_name}"`. If a feature group is included, it and all of its
  individual features will be included. This can be tested in code via
  `#[cfg(feature = "foo")]`.

Note that it is explicitly allowed for features to not actually activate any
optional dependencies. This allows packages to internally enable/disable
features without requiring a new dependency.

> **Note**: [crates.io] requires feature names to only contain ASCII letters,
> digits, `_`, or `-`.

#### Usage in end products

One major use-case for this feature is specifying optional features in
end-products. For example, the Servo package may want to include optional
features that people can enable or disable when they build it.

In that case, Servo will describe features in its `Cargo.toml` and they can be
enabled using command-line flags:

```console
$ cargo build --release --features "shumway pdf"
```

Default features could be excluded using `--no-default-features`.

#### Usage in packages

In most cases, the concept of *optional dependency* in a library is best
expressed as a separate package that the top-level application depends on.

However, high-level packages, like Iron or Piston, may want the ability to
curate a number of packages for easy installation. The current Cargo system
allows them to curate a number of mandatory dependencies into a single package
for easy installation.

In some cases, packages may want to provide additional curation for optional
dependencies:

* grouping a number of low-level optional dependencies together into a single
  high-level feature;
* specifying packages that are recommended (or suggested) to be included by
  users of the package; and
* including a feature (like `secure-password` in the motivating example) that
  will only work if an optional dependency is available, and would be difficult
  to implement as a separate package (for example, it may be overly difficult to
  design an IO package to be completely decoupled from OpenSSL, with opt-in via
  the inclusion of a separate package).

In almost all cases, it is an antipattern to use these features outside of
high-level packages that are designed for curation. If a feature is optional, it
can almost certainly be expressed as a separate package.

### The `[workspace]` section

Packages can define a workspace which is a set of crates that will all share the
same `Cargo.lock` and output directory. The `[workspace]` table can be defined
as:

```toml
[workspace]

# Optional key, inferred from path dependencies if not present.
# Additional non-path dependencies that should be included must be given here.
# In particular, for a virtual manifest, all members have to be listed.
members = ["path/to/member1", "path/to/member2", "path/to/member3/*"]

# Optional key, empty if not present.
exclude = ["path1", "path/to/dir2"]
```

Workspaces were added to Cargo as part of [RFC 1525] and have a number of
properties:

* A workspace can contain multiple crates where one of them is the *root crate*.
* The *root crate*'s `Cargo.toml` contains the `[workspace]` table, but is not
  required to have other configuration.
* Whenever any crate in the workspace is compiled, output is placed in the
  *workspace root* (i.e., next to the *root crate*'s `Cargo.toml`).
* The lock file for all crates in the workspace resides in the *workspace root*.
* The `[patch]`, `[replace]` and `[profile.*]` sections in `Cargo.toml`
  are only recognized
  in the *root crate*'s manifest, and ignored in member crates' manifests.

[RFC 1525]: https://github.com/rust-lang/rfcs/blob/master/text/1525-cargo-workspace.md

The *root crate* of a workspace, indicated by the presence of `[workspace]` in
its manifest, is responsible for defining the entire workspace. All `path`
dependencies residing in the workspace directory become members. You can add
additional packages to the workspace by listing them in the `members` key. Note
that members of the workspaces listed explicitly will also have their path
dependencies included in the workspace. Sometimes a package may have a lot of
workspace members and it can be onerous to keep up to date. The `members` list
can also use [globs][globs] to match multiple paths. Finally, the `exclude`
key can be used to blacklist paths from being included in a workspace. This can
be useful if some path dependencies aren't desired to be in the workspace at
all.

The `package.workspace` manifest key (described above) is used in member crates
to point at a workspace's root crate. If this key is omitted then it is inferred
to be the first crate whose manifest contains `[workspace]` upwards in the
filesystem.

A crate may either specify `package.workspace` or specify `[workspace]`. That
is, a crate cannot both be a root crate in a workspace (contain `[workspace]`)
and also be a member crate of another workspace (contain `package.workspace`).

Most of the time workspaces will not need to be dealt with as [`cargo new`] and
[`cargo init`] will handle workspace configuration automatically.

[globs]: https://docs.rs/glob/0.2.11/glob/struct.Pattern.html

#### Virtual Manifest

In workspace manifests, if the `package` table is present, the workspace root
crate will be treated as a normal package, as well as a workspace. If the
`package` table is not present in a workspace manifest, it is called a *virtual
manifest*.

#### Package selection

In a workspace, package-related cargo commands like [`cargo build`] apply to
packages selected by `-p` / `--package` or `--workspace` command-line parameters.
When neither is specified, the optional `default-members` configuration is used:

```toml
[workspace]
members = ["path/to/member1", "path/to/member2", "path/to/member3/*"]
default-members = ["path/to/member2", "path/to/member3/foo"]
```

When specified, `default-members` must expand to a subset of `members`.

When `default-members` is not specified, the default is the root manifest
if it is a package, or every member manifest (as if `--workspace` were specified
on the command-line) for virtual workspaces.

### The `[patch]` Section

This section of Cargo.toml can be used to [override dependencies][replace] with
other copies. The syntax is similar to the `[dependencies]` section:

```toml
[patch.crates-io]
foo = { git = 'https://github.com/example/foo' }
bar = { path = 'my/local/bar' }

[dependencies.baz]
git = 'https://github.com/example/baz'

[patch.'https://github.com/example/baz']
baz = { git = 'https://github.com/example/patched-baz', branch = 'my-branch' }
```

The `[patch]` table is made of dependency-like sub-tables. Each key after
`[patch]` is a URL of the source that is being patched, or the name of a
registry. The name `crates-io` may be used to override the default registry
[crates.io]. The first `[patch]` in the example above demonstrates overriding
[crates.io], and the second `[patch]` demonstrates overriding a git source.

Each entry in these tables is a normal dependency specification, the same as
found in the `[dependencies]` section of the manifest. The dependencies listed
in the `[patch]` section are resolved and used to patch the source at the
URL specified. The above manifest snippet patches the `crates-io` source (e.g.
crates.io itself) with the `foo` crate and `bar` crate. It also
patches the `https://github.com/example/baz` source with a `my-branch` that
comes from elsewhere.

Sources can be patched with versions of crates that do not exist, and they can
also be patched with versions of crates that already exist. If a source is
patched with a crate version that already exists in the source, then the
source's original crate is replaced.

More information about overriding dependencies can be found in the [overriding
dependencies][replace] section of the documentation and [RFC 1969] for the
technical specification of this feature.

[RFC 1969]: https://github.com/rust-lang/rfcs/pull/1969
[replace]: specifying-dependencies.md#overriding-dependencies

#### Using `[patch]` with multiple versions

You can patch in multiple versions of the same crate with the `package` key used
to rename dependencies. For example let's say that the `serde` crate has a
bugfix that we'd like to use to its 1.\* series but we'd also like to prototype
using a 2.0.0 version of serde we have in our git repository. To configure this
we'd do:

```toml
[patch.crates-io]
serde = { git = 'https://github.com/serde-rs/serde' }
serde2 = { git = 'https://github.com/example/serde', package = 'serde', branch = 'v2' }
```

The first `serde = ...` directive indicates that serde 1.\* should be used from
the git repository (pulling in the bugfix we need) and the second `serde2 = ...`
directive indicates that the `serde` package should also be pulled from the `v2`
branch of `https://github.com/example/serde`. We're assuming here that
`Cargo.toml` on that branch mentions version 2.0.0.

Note that when using the `package` key the `serde2` identifier here is actually
ignored. We simply need a unique name which doesn't conflict with other patched
crates.

### The `[replace]` Section

> **Note**: `[replace]` is deprecated. You should use the [`[patch]`][patch]
> table instead.

This section of Cargo.toml can be used to [override dependencies][replace] with
other copies. The syntax is similar to the `[dependencies]` section:

```toml
[replace]
"foo:0.1.0" = { git = 'https://github.com/example/foo' }
"bar:1.0.2" = { path = 'my/local/bar' }
```

Each key in the `[replace]` table is a [package ID
specification](pkgid-spec.md), which allows arbitrarily choosing a node in the
dependency graph to override (the 3-part version number is required). The
value of each key is the same as the `[dependencies]` syntax for specifying
dependencies, except that you can't specify features. Note that when a crate
is overridden the copy it's overridden with must have both the same name and
version, but it can come from a different source (e.g., git or a local path).

More information about overriding dependencies can be found in the [overriding
dependencies][replace] section of the documentation.

[`cargo build`]: ../commands/cargo-build.md
[`cargo init`]: ../commands/cargo-init.md
[`cargo new`]: ../commands/cargo-new.md
[`cargo run`]: ../commands/cargo-run.md
[crates.io]: https://crates.io/
[docs.rs]: https://docs.rs/
[publishing]: publishing.md
[Rust Edition]: ../../edition-guide/index.html
[spdx-2.1-license-expressions]: https://spdx.org/spdx-specification-21-web-version#h.jxpfx0ykyb60
[spdx-license-list-3.6]: https://github.com/spdx/license-list-data/tree/v3.6
[SPDX site]: https://spdx.org/license-list
[patch]: #the-patch-section

<script>
(function() {
    var fragments = {
        "#the-project-layout": "../guide/project-layout.html",
        "#examples": "cargo-targets.html#examples",
        "#tests": "cargo-targets.html#tests",
        "#integration-tests": "cargo-targets.html#integration-tests",
        "#configuring-a-target": "cargo-targets.html#configuring-a-target",
        "#target-auto-discovery": "cargo-targets.html#target-auto-discovery",
        "#the-required-features-field": "cargo-targets.html#the-required-features-field",
        "#building-dynamic-or-static-libraries": "cargo-targets.html#the-crate-type-field"
    };
    var target = fragments[window.location.hash];
    if (target) {
        var url = window.location.toString();
        var base = url.substring(0, url.lastIndexOf('/'));
        window.location.replace(base + "/" + target);
    }
})();
</script>
