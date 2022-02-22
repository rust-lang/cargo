## Specifying Dependencies

Your crates can depend on other libraries from [crates.io] or other
registries, `git` repositories, or subdirectories on your local file system.
You can also temporarily override the location of a dependency — for example,
to be able to test out a bug fix in the dependency that you are working on
locally. You can have different dependencies for different platforms, and
dependencies that are only used during development. Let's take a look at how
to do each of these.

### Specifying dependencies from crates.io

Cargo is configured to look for dependencies on [crates.io] by default. Only
the name and a version string are required in this case. In [the cargo
guide](../guide/index.md), we specified a dependency on the `time` crate:

```toml
[dependencies]
time = "0.1.12"
```

The string `"0.1.12"` is a version requirement. Although it looks like a
specific *version* of the `time` crate, it actually specifies a *range* of
versions and allows [SemVer] compatible updates. An update is allowed if the new
version number does not modify the left-most non-zero digit in the major, minor,
patch grouping. In this case, if we ran `cargo update -p time`, cargo should
update us to version `0.1.13` if it is the latest `0.1.z` release, but would not
update us to `0.2.0`. If instead we had specified the version string as `1.0`,
cargo should update to `1.1` if it is the latest `1.y` release, but not `2.0`.
The version `0.0.x` is not considered compatible with any other version.

[SemVer]: https://semver.org

Here are some more examples of version requirements and the versions that would
be allowed with them:

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

This compatibility convention is different from SemVer in the way it treats
versions before 1.0.0. While SemVer says there is no compatibility before
1.0.0, Cargo considers `0.x.y` to be compatible with `0.x.z`, where `y ≥ z`
and `x > 0`.

It is possible to further tweak the logic for selecting compatible versions
using special operators, though it shouldn't be necessary most of the time.

### Caret requirements

**Caret requirements** are an alternative syntax for the default strategy,
`^1.2.3` is exactly equivalent to `1.2.3`.

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

### Multiple requirements

As shown in the examples above, multiple version requirements can be
separated with a comma, e.g., `>= 1.2, < 1.5`.

### Specifying dependencies from other registries

To specify a dependency from a registry other than [crates.io], first the
registry must be configured in a `.cargo/config.toml` file. See the [registries
documentation] for more information. In the dependency, set the `registry` key
to the name of the registry to use.

```toml
[dependencies]
some-crate = { version = "1.0", registry = "my-registry" }
```

> **Note**: [crates.io] does not allow packages to be published with
> dependencies on other registries.

[registries documentation]: registries.md

### Specifying dependencies from `git` repositories

To depend on a library located in a `git` repository, the minimum information
you need to specify is the location of the repository with the `git` key:

```toml
[dependencies]
regex = { git = "https://github.com/rust-lang/regex" }
```

Cargo will fetch the `git` repository at this location then look for a
`Cargo.toml` for the requested crate anywhere inside the `git` repository
(not necessarily at the root - for example, specifying a member crate name
of a workspace and setting `git` to the repository containing the workspace).

Since we haven’t specified any other information, Cargo assumes that
we intend to use the latest commit on the main branch to build our package.
You can combine the `git` key with the `rev`, `tag`, or `branch` keys to
specify something else. Here's an example of specifying that you want to use
the latest commit on a branch named `next`:

```toml
[dependencies]
regex = { git = "https://github.com/rust-lang/regex", branch = "next" }
```

Anything that is not a branch or tag falls under `rev`. This can be a commit
hash like `rev = "4c59b707"`, or a named reference exposed by the remote
repository such as `rev = "refs/pull/493/head"`. What references are available
varies by where the repo is hosted; GitHub in particular exposes a reference to
the most recent commit of every pull request as shown, but other git hosts often
provide something equivalent, possibly under a different naming scheme.

Once a `git` dependency has been added, Cargo will lock that dependency to the
latest commit at the time. New commits will not be pulled down automatically
once the lock is in place. However, they can be pulled down manually with
`cargo update`.

See [Git Authentication] for help with git authentication for private repos.

> **Note**: [crates.io] does not allow packages to be published with `git`
> dependencies (`git` [dev-dependencies] are ignored). See the [Multiple
> locations](#multiple-locations) section for a fallback alternative.

[Git Authentication]: ../appendix/git-authentication.md

### Specifying path dependencies

Over time, our `hello_world` package from [the guide](../guide/index.md) has
grown significantly in size! It’s gotten to the point that we probably want to
split out a separate crate for others to use. To do this Cargo supports **path
dependencies** which are typically sub-crates that live within one repository.
Let’s start off by making a new crate inside of our `hello_world` package:

```console
# inside of hello_world/
$ cargo new hello_utils
```

This will create a new folder `hello_utils` inside of which a `Cargo.toml` and
`src` folder are ready to be configured. In order to tell Cargo about this, open
up `hello_world/Cargo.toml` and add `hello_utils` to your dependencies:

```toml
[dependencies]
hello_utils = { path = "hello_utils" }
```

This tells Cargo that we depend on a crate called `hello_utils` which is found
in the `hello_utils` folder (relative to the `Cargo.toml` it’s written in).

And that’s it! The next `cargo build` will automatically build `hello_utils` and
all of its own dependencies, and others can also start using the crate as well.
However, crates that use dependencies specified with only a path are not
permitted on [crates.io]. If we wanted to publish our `hello_world` crate, we
would need to publish a version of `hello_utils` to [crates.io]
and specify its version in the dependencies line as well:

```toml
[dependencies]
hello_utils = { path = "hello_utils", version = "0.1.0" }
```

> **Note**: [crates.io] does not allow packages to be published with `path`
> dependencies (`path` [dev-dependencies] are ignored). See the [Multiple
> locations](#multiple-locations) section for a fallback alternative.

### Multiple locations

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
smallvec = { git = "https://github.com/servo/rust-smallvec", version = "1.0" }

# N.B. that if a version doesn't match, Cargo will fail to compile!
```

One example where this can be useful is when you have split up a library into
multiple packages within the same workspace. You can then use `path`
dependencies to point to the local packages within the workspace to use the
local version during development, and then use the [crates.io] version once it
is published. This is similar to specifying an
[override](overriding-dependencies.md), but only applies to this one
dependency declaration.

### Platform specific dependencies

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

#### Custom target specifications

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

### Development dependencies

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

### Build dependencies

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

### Choosing features

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

### Renaming dependencies in `Cargo.toml`

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
bar = { git = "https://github.com/example/project", package = "foo" }
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

[crates.io]: https://crates.io/
[dev-dependencies]: #development-dependencies

<script>
(function() {
    var fragments = {
        "#overriding-dependencies": "overriding-dependencies.html",
        "#testing-a-bugfix": "overriding-dependencies.html#testing-a-bugfix",
        "#working-with-an-unpublished-minor-version": "overriding-dependencies.html#working-with-an-unpublished-minor-version",
        "#overriding-repository-url": "overriding-dependencies.html#overriding-repository-url",
        "#prepublishing-a-breaking-change": "overriding-dependencies.html#prepublishing-a-breaking-change",
        "#overriding-with-local-dependencies": "overriding-dependencies.html#paths-overrides",
    };
    var target = fragments[window.location.hash];
    if (target) {
        var url = window.location.toString();
        var base = url.substring(0, url.lastIndexOf('/'));
        window.location.replace(base + "/" + target);
    }
})();
</script>
