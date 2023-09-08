# Workspaces

A *workspace* is a collection of one or more packages, called *workspace
members*, that are managed together.

The key points of workspaces are:

* Common commands can run across all workspace members, like `cargo check --workspace`.
* All packages share a common [`Cargo.lock`] file which resides in the
  *workspace root*.
* All packages share a common [output directory], which defaults to a
  directory named `target` in the *workspace root*.
* Sharing package metadata, like with [`workspace.package`](#the-package-table).
* The [`[patch]`][patch], [`[replace]`][replace] and [`[profile.*]`][profiles]
  sections in `Cargo.toml` are only recognized in the *root* manifest, and
  ignored in member crates' manifests.

In the `Cargo.toml`, the `[workspace]` table supports the following sections:

* [`[workspace]`](#the-workspace-section) --- Defines a workspace.
  * [`resolver`](resolver.md#resolver-versions) --- Sets the dependency resolver to use.
  * [`members`](#the-members-and-exclude-fields) --- Packages to include in the workspace.
  * [`exclude`](#the-members-and-exclude-fields) --- Packages to exclude from the workspace.
  * [`default-members`](#the-default-members-field) --- Packages to operate on when a specific package wasn't selected.
  * [`package`](#the-package-table) --- Keys for inheriting in packages.
  * [`dependencies`](#the-dependencies-table) --- Keys for inheriting in package dependencies.
  * [`lints`](#the-lints-table) --- Keys for inheriting in package lints.
  * [`metadata`](#the-metadata-table) --- Extra settings for external tools.
* [`[patch]`](overriding-dependencies.md#the-patch-section) --- Override dependencies.
* [`[replace]`](overriding-dependencies.md#the-replace-section) --- Override dependencies (deprecated).
* [`[profile]`](profiles.md) --- Compiler settings and optimizations.

## The `[workspace]` section

To create a workspace, you add the `[workspace]` table to a `Cargo.toml`:
```toml
[workspace]
# ...
```

At minimum, a workspace has to have a member, either with a root package or as
a virtual manifest.

### Root package

If the [`[workspace]` section](#the-workspace-section) is added to a
`Cargo.toml` that already defines a `[package]`, the package is
the *root package* of the workspace. The *workspace root* is the directory
where the workspace's `Cargo.toml` is located.

```toml
[workspace]

[package]
name = "hello_world" # the name of the package
version = "0.1.0"    # the current version, obeying semver
authors = ["Alice <a@example.com>", "Bob <b@example.com>"]
```

### Virtual workspace

Alternatively, a `Cargo.toml` file can be created with a `[workspace]` section
but without a [`[package]` section][package]. This is called a *virtual
manifest*. This is typically useful when there isn't a "primary" package, or
you want to keep all the packages organized in separate directories.

```toml
# [PROJECT_DIR]/Cargo.toml
[workspace]
members = ["hello_world"]
resolver = "2"
```

```toml
# [PROJECT_DIR]/hello_world/Cargo.toml
[package]
name = "hello_world" # the name of the package
version = "0.1.0"    # the current version, obeying semver
edition = "2021"     # the edition, will have no effect on a resolver used in the workspace
authors = ["Alice <a@example.com>", "Bob <b@example.com>"]
```

Note that in a virtual manifest the [`resolver = "2"`](resolver.md#resolver-versions)
should be specified manually. It is usually deduced from the [`package.edition`][package-edition]
field which is absent in virtual manifests and the edition field of a member
won't affect the resolver used by the workspace.

## The `members` and `exclude` fields 

The `members` and `exclude` fields define which packages are members of
the workspace:

```toml
[workspace]
members = ["member1", "path/to/member2", "crates/*"]
exclude = ["crates/foo", "path/to/other"]
```

All [`path` dependencies] residing in the workspace directory automatically
become members. Additional members can be listed with the `members` key, which
should be an array of strings containing directories with `Cargo.toml` files.

The `members` list also supports [globs] to match multiple paths, using
typical filename glob patterns like `*` and `?`.

The `exclude` key can be used to prevent paths from being included in a
workspace. This can be useful if some path dependencies aren't desired to be
in the workspace at all, or using a glob pattern and you want to remove a
directory.

When inside a subdirectory within the workspace, Cargo will automatically
search the parent directories for a `Cargo.toml` file with a `[workspace]`
definition to determine which workspace to use. The [`package.workspace`]
manifest key can be used in member crates to point at a workspace's root to
override this automatic search. The manual setting can be useful if the member
is not inside a subdirectory of the workspace root.

### Package selection

In a workspace, package-related Cargo commands like [`cargo build`] can use
the `-p` / `--package` or `--workspace` command-line flags to determine which
packages to operate on. If neither of those flags are specified, Cargo will
use the package in the current working directory. If the current directory is
a [virtual workspace](#virtual-workspace), it will apply to all members (as if
`--workspace` were specified on the command-line).  See also
[`default-members`](#the-default-members-field).

## The `default-members` field

The optional `default-members` key can be specified to set the members to
operate on when in the workspace root and the package selection flags are not
used:

```toml
[workspace]
members = ["path/to/member1", "path/to/member2", "path/to/member3/*"]
default-members = ["path/to/member2", "path/to/member3/foo"]
```

When specified, `default-members` must expand to a subset of `members`.

## The `package` table

The `workspace.package` table is where you define keys that can be
inherited by members of a workspace. These keys can be inherited by
defining them in the member package with `{key}.workspace = true`.

Keys that are supported:

|                |                 |
|----------------|-----------------|
| `authors`      | `categories`    |
| `description`  | `documentation` |
| `edition`      | `exclude`       |
| `homepage`     | `include`       |
| `keywords`     | `license`       |
| `license-file` | `publish`       |
| `readme`       | `repository`    |
| `rust-version` | `version`       |

- `license-file` and `readme` are relative to the workspace root
- `include` and `exclude` are relative to your package root

Example:
```toml
# [PROJECT_DIR]/Cargo.toml
[workspace]
members = ["bar"]

[workspace.package]
version = "1.2.3"
authors = ["Nice Folks"]
description = "A short description of my package"
documentation = "https://example.com/bar"
```

```toml
# [PROJECT_DIR]/bar/Cargo.toml
[package]
name = "bar"
version.workspace = true
authors.workspace = true
description.workspace = true
documentation.workspace = true
```

## The `dependencies` table

The `workspace.dependencies` table is where you define dependencies to be
inherited by members of a workspace.

Specifying a workspace dependency is similar to [package dependencies][specifying-dependencies] except:
- Dependencies from this table cannot be declared as `optional`
- [`features`][features] declared in this table are additive with the `features` from `[dependencies]`

You can then [inherit the workspace dependency as a package dependency][inheriting-a-dependency-from-a-workspace]

Example:
```toml
# [PROJECT_DIR]/Cargo.toml
[workspace]
members = ["bar"]

[workspace.dependencies]
cc = "1.0.73"
rand = "0.8.5"
regex = { version = "1.6.0", default-features = false, features = ["std"] }
```

```toml
# [PROJECT_DIR]/bar/Cargo.toml
[package]
name = "bar"
version = "0.2.0"

[dependencies]
regex = { workspace = true, features = ["unicode"] }

[build-dependencies]
cc.workspace = true

[dev-dependencies]
rand.workspace = true
```

## The `lints` table

The `workspace.lints` table is where you define lint configuration to be inherited by members of a workspace.

Specifying a workspace lint configuration is similar to package lints.

Example:

```toml
# [PROJECT_DIR]/Cargo.toml
[workspace]
members = ["crates/*"]

[workspace.lints.rust]
unsafe_code = "forbid"
```

```toml
# [PROJECT_DIR]/crates/bar/Cargo.toml
[package]
name = "bar"
version = "0.1.0"

[lints]
workspace = true
```

## The `metadata` table

The `workspace.metadata` table is ignored by Cargo and will not be warned
about. This section can be used for tools that would like to store workspace
configuration in `Cargo.toml`. For example:

```toml
[workspace]
members = ["member1", "member2"]

[workspace.metadata.webcontents]
root = "path/to/webproject"
tool = ["npm", "run", "build"]
# ...
```

There is a similar set of tables at the package level at
[`package.metadata`][package-metadata]. While cargo does not specify a
format for the content of either of these tables, it is suggested that
external tools may wish to use them in a consistent fashion, such as referring
to the data in `workspace.metadata` if data is missing from `package.metadata`,
if that makes sense for the tool in question.

[package]: manifest.md#the-package-section
[`Cargo.lock`]: ../guide/cargo-toml-vs-cargo-lock.md
[package-metadata]: manifest.md#the-metadata-table
[package-edition]: manifest.md#the-edition-field
[output directory]: ../guide/build-cache.md
[patch]: overriding-dependencies.md#the-patch-section
[replace]: overriding-dependencies.md#the-replace-section
[profiles]: profiles.md
[`path` dependencies]: specifying-dependencies.md#specifying-path-dependencies
[`package.workspace`]: manifest.md#the-workspace-field
[globs]: https://docs.rs/glob/0.3.0/glob/struct.Pattern.html
[`cargo build`]: ../commands/cargo-build.md
[specifying-dependencies]: specifying-dependencies.md
[features]: features.md
[inheriting-a-dependency-from-a-workspace]: specifying-dependencies.md#inheriting-a-dependency-from-a-workspace

<script>
(function() {
    var fragments = {
        "#virtual-manifest": "workspaces.html#virtual-workspace",
    };
    var target = fragments[window.location.hash];
    if (target) {
        var url = window.location.toString();
        var base = url.substring(0, url.lastIndexOf('/'));
        window.location.replace(base + "/" + target);
    }
})();
</script>
