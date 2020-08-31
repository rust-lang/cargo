## Workspaces

A *workspace* is a collection of one or more packages that share common
dependency resolution (with a shared `Cargo.lock`), output directory, and
various settings such as profiles. Packages that are part of a workspaces are
called *workspace members*. There are two flavours of workspaces: as root
package or as virtual manifest.

### Root package

A workspace can be created by adding a [`[workspace]`
section](#the-workspace-section) to `Cargo.toml`. This can be added to a
`Cargo.toml` that already defines a `[package]`, in which case the package is
the *root package* of the workspace. The *workspace root* is the directory
where the workspace's `Cargo.toml` is located.

### Virtual manifest

Alternatively, a `Cargo.toml` file can be created with a `[workspace]` section
but without a [`[package]` section][package]. This is called a *virtual
manifest*. This is typically useful when there isn't a "primary" package, or
you want to keep all the packages organized in separate directories.

### Key features

The key points of workspaces are:

* All packages share a common `Cargo.lock` file which resides in the
  *workspace root*.
* All packages share a common [output directory], which defaults to a
  directory named `target` in the *workspace root*.
* The [`[patch]`][patch], [`[replace]`][replace] and [`[profile.*]`][profiles]
  sections in `Cargo.toml` are only recognized in the *root* manifest, and
  ignored in member crates' manifests.

### The `[workspace]` section

The `[workspace]` table in `Cargo.toml` defines which packages are members of
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

An empty `[workspace]` table can be used with a `[package]` to conveniently
create a workspace with the package and all of its path dependencies.

### Workspace selection

When inside a subdirectory within the workspace, Cargo will automatically
search the parent directories for a `Cargo.toml` file with a `[workspace]`
definition to determine which workspace to use. The [`package.workspace`]
manifest key can be used in member crates to point at a workspace's root to
override this automatic search. The manual setting can be useful if the member
is not inside a subdirectory of the workspace root.

### Package selection

In a workspace, package-related cargo commands like [`cargo build`] can use
the `-p` / `--package` or `--workspace` command-line flags to determine which
packages to operate on. If neither of those flags are specified, Cargo will
use the package in the current working directory. If the current directory is
a virtual workspace, it will apply to all members (as if `--workspace` were
specified on the command-line).

The optional `default-members` key can be specified to set the members to
operate on when in the workspace root and the package selection flags are not
used:

```toml
[workspace]
members = ["path/to/member1", "path/to/member2", "path/to/member3/*"]
default-members = ["path/to/member2", "path/to/member3/foo"]
```

When specified, `default-members` must expand to a subset of `members`.

### The `workspace.metadata` table

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
[package-metadata]: manifest.md#the-metadata-table
[output directory]: ../guide/build-cache.md
[patch]: overriding-dependencies.md#the-patch-section
[replace]: overriding-dependencies.md#the-replace-section
[profiles]: profiles.md
[`path` dependencies]: specifying-dependencies.md#specifying-path-dependencies
[`package.workspace`]: manifest.md#the-workspace-field
[globs]: https://docs.rs/glob/0.3.0/glob/struct.Pattern.html
[`cargo build`]: ../commands/cargo-build.md
