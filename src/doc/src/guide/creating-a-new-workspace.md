# Creating a New Workspace

A [workspace][def-workspace] is a collection of one or more packages, 
called workspace members, that are managed together.

In this chapter, we will create a workspace `new_workspace` containing 
binary member `foo` and library member `bar`.

As mentioned in [`[workspace]` section][workspace-section], the workspace must 
have at least one member, either the [root package] or a [virtual manifest].

Next we create a workspace containing [root package].
For convenience, you can first create a package using the command `cargo new new_workspace`.
Then add the `[workspace]` section to the `Cargo.toml` file in the root directory 
to make it a manifest of the workspace:

```toml
# [new_workspace]/Cargo.toml
[workspace]

[package]
name = "new_workspace"
version = "0.1.0"
edition = "2021"

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
```

Then, continue adding members `foo` and `bar` to the workspace:

```console
$ cd new_workspace
$ cargo new foo
$ cargo new bar --lib
```

Cargo will automatically add members to `Cargo.toml`。
At this point, the workspace will contain three members: `foo` and `bar` and 
the default member `new_workspace`.

```toml
# [new_workspace]/Cargo.toml
[workspace]
members = [ "bar", "foo" ]

[package]
name = "new_workspace"
version = "0.1.0"
edition = "2021"

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
```

The package at this point contains the following files:

```console
$ cd new_workspace
$ tree .
.
├── bar
│   ├── Cargo.toml
│   └── src
│       └── lib.rs
├── Cargo.toml
├── foo
│   ├── Cargo.toml
│   └── src
│       └── main.rs
└── src
    └── main.rs

5 directories, 6 files
```

Let's move on and create a virtual workspace.

In the another `new_workspace` empty directory, create a new `Cargo.toml` file and 
add the `[workspace]` section:

```toml
# [new_workspace]/Cargo.toml
[workspace]
```

If using a virtual workspace, then the version of [resolver] needs to be specified 
in the table (if not, the default version of resolver for a workspace is `1`, 
even if the default resolver version for workspace members is `2`), for example:

```toml
# [new_workspace]/Cargo.toml
[workspace]
resolver = "2"
```

Likewise, you can then use the `cargo new <package>` command to create 
binary member `foo` and library member `bar`.


```toml
# [new_workspace]/Cargo.toml
[workspace]
resolver = "2"
members = [ "bar","foo"]

```

The package at this point contains the following files:

```console
$ cd new_workspace
$ tree .
.
├── bar
│   ├── Cargo.toml
│   └── src
│       └── lib.rs
├── Cargo.toml
└── foo
    ├── Cargo.toml
    └── src
        └── main.rs

4 directories, 5 files
```

Up to this point, we have a workspace with two members.
Whenever you run `cargo build` under the workspace root directory, Cargo builds 
all member at once.
Instead of building the entire workspace, you could use the `--package`/`-p` flag 
to select certain packages.
For example, `cargo build -p foo` will build only `foo` package.

[workspace-section]: ../reference/workspaces.md#the-workspace-section
[root package]:      ../reference/workspaces.md#root-package
[virtual manifest]:  ../reference/workspaces.md#virtual-workspace
[def-workspace]:     ../appendix/glossary.md#workspace  '"workspace" (glossary entry)'
[resolver]:          ../reference/resolver.md