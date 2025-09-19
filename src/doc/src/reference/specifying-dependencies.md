# Specifying Dependencies

Your crates can depend on other libraries from [crates.io] or other
registries, `git` repositories, or subdirectories on your local file system.
You can also temporarily override the location of a dependency --- for example,
to be able to test out a bug fix in the dependency that you are working on
locally. You can have different dependencies for different platforms, and
dependencies that are only used during development. Let's take a look at how
to do each of these.

## Specifying dependencies from crates.io

Cargo is configured to look for dependencies on [crates.io] by default. Only
the name and a version string are required in this case. In [the cargo
guide](../guide/index.md), we specified a dependency on the `time` crate:

```toml
[dependencies]
time = "0.1.12"
```

The version string `"0.1.12"` is called a [version requirement](#version-requirement-syntax).
It specifies a range of versions that can be selected from when [resolving dependencies](resolver.md).
In this case, `"0.1.12"` represents the version range `>=0.1.12, <0.2.0`.
An update is allowed if it is within that range.
In this case, if we ran `cargo update time`, cargo should
update us to version `0.1.13` if it is the latest `0.1.z` release, but would not
update us to `0.2.0`.

## Version requirement syntax

### Default requirements

**Default requirements** specify a minimum version with the ability to update to [SemVer] compatible versions.
Versions are considered compatible if their left-most non-zero major/minor/patch component is the same.
This is different from [SemVer] which considers all pre-1.0.0 packages to be incompatible.

`1.2.3` is an example of a default requirement.

```notrust
1.2.3  :=  >=1.2.3, <2.0.0
1.2    :=  >=1.2.0, <2.0.0
1      :=  >=1.0.0, <2.0.0
0.2.3  :=  >=0.2.3, <0.3.0
0.2    :=  >=0.2.0, <0.3.0
0.0.3  :=  >=0.0.3, <0.0.4
0.0    :=  >=0.0.0, <0.1.0
0      :=  >=0.0.0, <1.0.0
```

### Caret requirements

**Caret requirements** are the default version requirement strategy. 
This version strategy allows [SemVer] compatible updates.
They are specified as version requirements with a leading caret (`^`).

`^1.2.3` is an example of a caret requirement.

Leaving off the caret is a simplified equivalent syntax to using caret requirements.
While caret requirements are the default, it is recommended to use the
simplified syntax when possible.

`log = "^1.2.3"` is exactly equivalent to `log = "1.2.3"`.

### Tilde requirements

**Tilde requirements** specify a minimal version with some ability to update.
If you specify a major, minor, and patch version or only a major and minor
version, only patch-level changes are allowed. If you only specify a major
version, then minor- and patch-level changes are allowed.

`~1.2.3` is an example of a tilde requirement.

```notrust
~1.2.3  := >=1.2.3, <1.3.0
~1.2    := >=1.2.0, <1.3.0
~1      := >=1.0.0, <2.0.0
```

### Wildcard requirements

**Wildcard requirements** allow for any version where the wildcard is
positioned.

`*`, `1.*` and `1.2.*` are examples of wildcard requirements.

```notrust
*     := >=0.0.0
1.*   := >=1.0.0, <2.0.0
1.2.* := >=1.2.0, <1.3.0
```

> **Note**: [crates.io] does not allow bare `*` versions.

### Comparison requirements

**Comparison requirements** allow manually specifying a version range or an
exact version to depend on.

Here are some examples of comparison requirements:

```notrust
>= 1.2.0
> 1
< 2
= 1.2.3
```

<span id="multiple-requirements"></span>
### Multiple version requirements

As shown in the examples above, multiple version requirements can be
separated with a comma, e.g., `>= 1.2, < 1.5`.
All requirements must be satisfied,
so non-overlapping requirements like `<1.2, ^1.2.2` result in no matching versions.

### Pre-releases

Version requirements exclude [pre-release versions](manifest.md#the-version-field), such as `1.0.0-alpha`,
unless specifically asked for.
For example, if `1.0.0-alpha` of package
`foo` is published, then a requirement of `foo = "1.0"` will *not* match, and
will return an error. The pre-release must be specified, such as `foo =
"1.0.0-alpha"`.
Similarly [`cargo install`] will avoid pre-releases unless
explicitly asked to install one.

Cargo allows "newer" pre-releases to be used automatically. For example, if
`1.0.0-beta` is published, then a requirement `foo = "1.0.0-alpha"` will allow
updating to the `beta` version. Note that this only works on the same release
version, `foo = "1.0.0-alpha"` will not allow updating to `foo = "1.0.1-alpha"`
or `foo = "1.0.1-beta"`.

Cargo will also upgrade automatically to semver-compatible released versions
from prereleases. The requirement `foo = "1.0.0-alpha"` will allow updating to
`foo = "1.0.0"` as well as `foo = "1.2.0"`.

Beware that pre-release versions can be unstable, and as such care should be
taken when using them. Some projects may choose to publish breaking changes
between pre-release versions. It is recommended to not use pre-release
dependencies in a library if your library is not also a pre-release. Care
should also be taken when updating your `Cargo.lock`, and be prepared if a
pre-release update causes issues.

[`cargo install`]: ../commands/cargo-install.md

### Version metadata

[Version metadata](manifest.md#the-version-field), such as `1.0.0+21AF26D3`,
is ignored and should not be used in version requirements.

> **Recommendation:** When in doubt, use the default version requirement operator.
>
> In rare circumstances, a package with a "public dependency"
> (re-exports the dependency or interoperates with it in its public API)
> that is compatible with multiple semver-incompatible versions
> (e.g. only uses a simple type that hasn't changed between releases, like an `Id`)
> may support users choosing which version of the "public dependency" to use.
> In this case, a version requirement like `">=0.4, <2"` may be of interest.
> *However* users of the package will likely run into errors and need to
> manually select a version of the "public dependency" via `cargo update` if
> they also depend on it as Cargo might pick different versions of the "public
> dependency" when [resolving dependency versions](resolver.md)  (see
> [#10599]).
>
> Avoid constraining the upper bound of a version to be anything less than the
> next semver incompatible version
> (e.g. avoid `">=2.0, <2.4"`, `"2.0.*"`, or `~2.0`),
> as other packages in the dependency tree may
> require a newer version, leading to an unresolvable error (see [#9029]).
> Consider whether controlling the version in your [`Cargo.lock`] would be more
> appropriate.
>
> In some instances this won't matter or the benefits might outweigh the cost, including:
> - When no one else depends on your package; e.g. it only has a `[[bin]]`
> - When depending on a pre-release package and wishing to avoid breaking
>   changes, then a fully specified `"=1.2.3-alpha.3"` might be warranted (see
>   [#2222])
> - When a library re-exports a proc-macro but the proc-macro generates code that
>   calls into the re-exporting library, then a fully specified `=1.2.3` might be
>   warranted to ensure the proc-macro isn't newer than the re-exporting library
>   and generating code that uses parts of the API that don't exist within the
>   current version

[`Cargo.lock`]: ../guide/cargo-toml-vs-cargo-lock.md
[#2222]: https://github.com/rust-lang/cargo/issues/2222
[#9029]: https://github.com/rust-lang/cargo/issues/9029
[#10599]: https://github.com/rust-lang/cargo/issues/10599

## Specifying dependencies from other registries

To specify a dependency from a registry other than [crates.io] set the `registry` key
to the name of the registry to use:

```toml
[dependencies]
some-crate = { version = "1.0", registry = "my-registry" }
```

where `my-registry` is the registry name configured in `.cargo/config.toml` file.
See the [registries documentation] for more information.

> **Note**: [crates.io] does not allow packages to be published with
> dependencies on code published outside of [crates.io].

[registries documentation]: registries.md

## Specifying dependencies from `git` repositories

To depend on a library located in a `git` repository, the minimum information
you need to specify is the location of the repository with the `git` key:

```toml
[dependencies]
regex = { git = "https://github.com/rust-lang/regex.git" }
```

Cargo fetches the `git` repository at that location and traverses the file tree to find
`Cargo.toml` file for the requested crate anywhere inside the `git` repository. 
For example, `regex-lite` and `regex-syntax` are members of `rust-lang/regex` repo
and can be referred to by the repo's root URL (`https://github.com/rust-lang/regex.git`)
regardless of where in the file tree they reside.

```toml
regex-lite   = { git = "https://github.com/rust-lang/regex.git" }
regex-syntax = { git = "https://github.com/rust-lang/regex.git" }
```

The above rule does not apply to [`path` dependencies](#specifying-path-dependencies).

### Choice of commit

Cargo assumes that we intend to use the latest commit on the default branch to build
our package if we only specify the repo URL, as in the examples above.

You can combine the `git` key with the `rev`, `tag`, or `branch` keys to be more specific about
which commit to use. Here's an example of using the latest commit on a branch named `next`:

```toml
[dependencies]
regex = { git = "https://github.com/rust-lang/regex.git", branch = "next" }
```

Anything that is not a branch or a tag falls under `rev` key. This can be a commit
hash like `rev = "4c59b707"`, or a named reference exposed by the remote
repository such as `rev = "refs/pull/493/head"`. 

What references are available for the `rev` key varies by where the repo is hosted.  
GitHub exposes a reference to the most recent commit of every pull request as in the example above.
Other git hosts may provide something equivalent under a different naming scheme.

**More `git` dependency examples:**

```toml
# .git suffix can be omitted if the host accepts such URLs - both examples work the same
regex = { git = "https://github.com/rust-lang/regex" }
regex = { git = "https://github.com/rust-lang/regex.git" }

# a commit with a particular tag
regex = { git = "https://github.com/rust-lang/regex.git", tag = "1.10.3" }

# a commit by its SHA1 hash
regex = { git = "https://github.com/rust-lang/regex.git", rev = "0c0990399270277832fbb5b91a1fa118e6f63dba" }

# HEAD commit of PR 493
regex = { git = "https://github.com/rust-lang/regex.git", rev = "refs/pull/493/head" }

# INVALID EXAMPLES

# specifying the commit after # ignores the commit ID and generates a warning
regex = { git = "https://github.com/rust-lang/regex.git#4c59b70" }

# git and path cannot be used at the same time
regex = { git = "https://github.com/rust-lang/regex.git#4c59b70", path = "../regex" }
```

Cargo locks the commits of `git` dependencies in `Cargo.lock` file at the time of their addition
and checks for updates only when you run `cargo update` command.

### The role of the `version` key

The `version` key always implies that the package is available in a registry,
regardless of the presence of `git` or `path` keys.

The `version` key does _not_ affect which commit is used when Cargo retrieves the `git` dependency,
but Cargo checks the version information in the dependency's `Cargo.toml` file 
against the `version` key and raises an error if the check fails.

In this example, Cargo retrieves the HEAD commit of the branch called `next` from Git and checks if the crate's version
is compatible with `version = "1.10.3"`:

```toml
[dependencies]
regex = { version = "1.10.3", git = "https://github.com/rust-lang/regex.git", branch = "next" }
```

`version`, `git`, and `path` keys are considered separate locations for resolving the dependency. 
See [Multiple locations](#multiple-locations) section below for detailed explanations.

> **Note**: [crates.io] does not allow packages to be published with
> dependencies on code published outside of [crates.io] itself
> ([dev-dependencies] are ignored). See the [Multiple
> locations](#multiple-locations) section for a fallback alternative for `git`
> and `path` dependencies.

### Git submodules

When cloning a `git` dependency,
Cargo automatically fetches its submodules recursively
so that all required code is available for the build.

To skip fetching submodules unrelated to the build,
you can set [`submodule.<name>.update = none`][submodule-update] in the dependency repo's `.gitmodules`.
This requires write access to the repo and will disable submodule updates more generally.

[submodule-update]: https://git-scm.com/docs/gitmodules#Documentation/gitmodules.txt-submodulenameupdate

### Accessing private Git repositories

See [Git Authentication](../appendix/git-authentication.md) for help with Git authentication for private repos.

## Specifying path dependencies

Over time, our `hello_world` package from [the guide](../guide/index.md) has
grown significantly in size! It’s gotten to the point that we probably want to
split out a separate crate for others to use. To do this Cargo supports **path
dependencies** which are typically sub-crates that live within one repository.
Let’s start by making a new crate inside of our `hello_world` package:

```console
# inside of hello_world/
$ cargo new hello_utils
```

This will create a new folder `hello_utils` inside of which a `Cargo.toml` and
`src` folder are ready to be configured. To tell Cargo about this, open
up `hello_world/Cargo.toml` and add `hello_utils` to your dependencies:

```toml
[dependencies]
hello_utils = { path = "hello_utils" }
```

This tells Cargo that we depend on a crate called `hello_utils` which is found
in the `hello_utils` folder, relative to the `Cargo.toml` file it’s written in.

The next `cargo build` will automatically build `hello_utils` and
all of its dependencies.

### No local path traversal

The local paths must point to the exact folder with the dependency's `Cargo.toml`.
Unlike with `git` dependencies, Cargo does not traverse local paths.
For example, if `regex-lite` and `regex-syntax` are members of a
locally cloned `rust-lang/regex` repo, they have to be referred to by the full path:

```toml
# git key accepts the repo root URL and Cargo traverses the tree to find the crate
[dependencies]
regex-lite   = { git = "https://github.com/rust-lang/regex.git" }
regex-syntax = { git = "https://github.com/rust-lang/regex.git" }

# path key requires the member name to be included in the local path
[dependencies]
regex-lite   = { path = "../regex/regex-lite" }
regex-syntax = { path = "../regex/regex-syntax" }
```

### Local paths in published crates

Crates that use dependencies specified with only a path are not
permitted on [crates.io].

If we wanted to publish our `hello_world` crate,
we would need to publish a version of `hello_utils` to [crates.io] as a separate crate
and specify its version in the dependencies line of `hello_world`:

```toml
[dependencies]
hello_utils = { path = "hello_utils", version = "0.1.0" }
```

The use of `path` and `version` keys together is explained in the [Multiple locations](#multiple-locations) section.

> **Note**: [crates.io] does not allow packages to be published with
> dependencies on code outside of [crates.io], except for [dev-dependencies].
> See the [Multiple locations](#multiple-locations) section
> for a fallback alternative for `git` and `path` dependencies.

## Multiple locations

It is possible to specify both a registry version and a `git` or `path`
location. The `git` or `path` dependency will be used locally (in which case
the `version` is checked against the local copy), and when published to a
registry like [crates.io], it will use the registry version. Other
combinations are not allowed. Examples:

```toml
[dependencies]
# Uses `my-bitflags` when used locally, and uses
# version 1.0 from crates.io when published.
bitflags = { path = "my-bitflags", version = "1.0" }

# Uses the given git repo when used locally, and uses
# version 1.0 from crates.io when published.
smallvec = { git = "https://github.com/servo/rust-smallvec.git", version = "1.0" }

# Note: if a version doesn't match, Cargo will fail to compile!
```

One example where this can be useful is when you have split up a library into
multiple packages within the same workspace. You can then use `path`
dependencies to point to the local packages within the workspace to use the
local version during development, and then use the [crates.io] version once it
is published. This is similar to specifying an
[override](overriding-dependencies.md), but only applies to this one
dependency declaration.

## Platform specific dependencies

Platform-specific dependencies take the same format, but are listed under a
`target` section. Normally Rust-like [`#[cfg]`
syntax](../../reference/conditional-compilation.html) will be used to define
these sections:

```toml
[target.'cfg(windows)'.dependencies]
winhttp = "0.4.0"

[target.'cfg(unix)'.dependencies]
openssl = "1.0.1"

[target.'cfg(target_arch = "x86")'.dependencies]
native-i686 = { path = "native/i686" }

[target.'cfg(target_arch = "x86_64")'.dependencies]
native-x86_64 = { path = "native/x86_64" }
```

Like with Rust, the syntax here supports the `not`, `any`, and `all` operators
to combine various cfg name/value pairs.

If you want to know which cfg targets are available on your platform, run
`rustc --print=cfg` from the command line. If you want to know which `cfg`
targets are available for another platform, such as 64-bit Windows,
run `rustc --print=cfg --target=x86_64-pc-windows-msvc`.

Unlike in your Rust source code, you cannot use
`[target.'cfg(feature = "fancy-feature")'.dependencies]` to add dependencies
based on optional features. Use [the `[features]` section](features.md)
instead:

```toml
[dependencies]
foo = { version = "1.0", optional = true }
bar = { version = "1.0", optional = true }

[features]
fancy-feature = ["foo", "bar"]
```

The same applies to `cfg(debug_assertions)`, `cfg(test)` and `cfg(proc_macro)`.
These values will not work as expected and will always have the default value
returned by `rustc --print=cfg`.
There is currently no way to add dependencies based on these configuration values.

In addition to `#[cfg]` syntax, Cargo also supports listing out the full target
the dependencies would apply to:

```toml
[target.x86_64-pc-windows-gnu.dependencies]
winhttp = "0.4.0"

[target.i686-unknown-linux-gnu.dependencies]
openssl = "1.0.1"
```

### Custom target specifications

If you’re using a custom target specification (such as `--target
foo/bar.json`), use the base filename without the `.json` extension:

```toml
[target.bar.dependencies]
winhttp = "0.4.0"

[target.my-special-i686-platform.dependencies]
openssl = "1.0.1"
native = { path = "native/i686" }
```

> **Note**: Custom target specifications are not usable on the stable channel.

## Development dependencies

You can add a `[dev-dependencies]` section to your `Cargo.toml` whose format
is equivalent to `[dependencies]`:

```toml
[dev-dependencies]
tempdir = "0.3"
```

Dev-dependencies are not used when compiling
a package for building, but are used for compiling tests, examples, and
benchmarks.

These dependencies are *not* propagated to other packages which depend on this
package.

You can also have target-specific development dependencies by using
`dev-dependencies` in the target section header instead of `dependencies`. For
example:

```toml
[target.'cfg(unix)'.dev-dependencies]
mio = "0.0.1"
```

> **Note**: When a package is published, only dev-dependencies that specify a
> `version` will be included in the published crate. For most use cases,
> dev-dependencies are not needed when published, though some users (like OS
> packagers) may want to run tests within a crate, so providing a `version` if
> possible can still be beneficial.

## Build dependencies

You can depend on other Cargo-based crates for use in your build scripts.
Dependencies are declared through the `build-dependencies` section of the
manifest:

```toml
[build-dependencies]
cc = "1.0.3"
```


You can also have target-specific build dependencies by using
`build-dependencies` in the target section header instead of `dependencies`. For
example:

```toml
[target.'cfg(unix)'.build-dependencies]
cc = "1.0.3"
```

In this case, the dependency will only be built when the host platform matches the
specified target.

The build script **does not** have access to the dependencies listed
in the `dependencies` or `dev-dependencies` section. Build
dependencies will likewise not be available to the package itself
unless listed under the `dependencies` section as well. A package
itself and its build script are built separately, so their
dependencies need not coincide. Cargo is kept simpler and cleaner by
using independent dependencies for independent purposes.

## Choosing features

If a package you depend on offers conditional features, you can
specify which to use:

```toml
[dependencies.awesome]
version = "1.3.5"
default-features = false # do not include the default features, and optionally
                         # cherry-pick individual features
features = ["secure-password", "civet"]
```

More information about features can be found in the [features
chapter](features.md#dependency-features).

## Renaming dependencies in `Cargo.toml`

When writing a `[dependencies]` section in `Cargo.toml` the key you write for a
dependency typically matches up to the name of the crate you import from in the
code. For some projects, though, you may wish to reference the crate with a
different name in the code regardless of how it's published on crates.io. For
example you may wish to:

* Avoid the need to  `use foo as bar` in Rust source.
* Depend on multiple versions of a crate.
* Depend on crates with the same name from different registries.

To support this Cargo supports a `package` key in the `[dependencies]` section
of which package should be depended on:

```toml
[package]
name = "mypackage"
version = "0.0.1"

[dependencies]
foo = "0.1"
bar = { git = "https://github.com/example/project.git", package = "foo" }
baz = { version = "0.1", registry = "custom", package = "foo" }
```

In this example, three crates are now available in your Rust code:

```rust,ignore
extern crate foo; // crates.io
extern crate bar; // git repository
extern crate baz; // registry `custom`
```

All three of these crates have the package name of `foo` in their own
`Cargo.toml`, so we're explicitly using the `package` key to inform Cargo that
we want the `foo` package even though we're calling it something else locally.
The `package` key, if not specified, defaults to the name of the dependency
being requested.

Note that if you have an optional dependency like:

```toml
[dependencies]
bar = { version = "0.1", package = 'foo', optional = true }
```

you're depending on the crate `foo` from crates.io, but your crate has a `bar`
feature instead of a `foo` feature. That is, names of features take after the
name of the dependency, not the package name, when renamed.

Enabling transitive dependencies works similarly, for example we could add the
following to the above manifest:

```toml
[features]
log-debug = ['bar/log-debug'] # using 'foo/log-debug' would be an error!
```

## Inheriting a dependency from a workspace

Dependencies can be inherited from a workspace by specifying the
dependency in the workspace's [`[workspace.dependencies]`][workspace.dependencies] table.
After that, add it to the `[dependencies]` table with `workspace = true`.

Along with the `workspace` key, dependencies can also include these keys:
- [`optional`][optional]: Note that the`[workspace.dependencies]` table is not allowed to specify `optional`.
- [`features`][features]: These are additive with the features declared in the `[workspace.dependencies]`

Other than `optional` and `features`, inherited dependencies cannot use any other
dependency key (such as `version` or `default-features`).

Dependencies in the `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, and
`[target."...".dependencies]` sections support the ability to reference the
`[workspace.dependencies]` definition of dependencies.

```toml
[package]
name = "bar"
version = "0.2.0"

[dependencies]
regex = { workspace = true, features = ["unicode"] }

[build-dependencies]
cc.workspace = true

[dev-dependencies]
rand = { workspace = true, optional = true }
```


[SemVer]: https://semver.org
[crates.io]: https://crates.io/
[dev-dependencies]: #development-dependencies
[workspace.dependencies]: workspaces.md#the-dependencies-table
[optional]: features.md#optional-dependencies
[features]: features.md
