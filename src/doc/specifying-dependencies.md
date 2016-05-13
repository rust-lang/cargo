% Specifying Dependencies

Your crates can depend on other libraries from [crates.io], git repositories, or
subdirectories on your local file system. You can also temporarily override the
location of a dependency-- for example, to be able to test out a bug fix in the
dependency that you are working on locally. You can have different
dependencies for different platforms, and dependencies that are only used during
development. Let's take a look at how to do each of these.

# Specifying Dependencies from crates.io

Cargo is configured to look for dependencies on [crates.io] by default. Only
the name and a version string are required in this case. In [the cargo
guide](guide.html), we specified a dependency on the `time` crate:

```toml
[dependencies]
time = "0.1.12"
```

The string `"0.1.12"` is a [semver] version requirement. Since this
string does not have any operators in it, it is interpreted the same way as
if we had specified `"^0.1.12"`, which is called a caret requirement.

[semver]: https://github.com/steveklabnik/semver#requirements

## Caret requirements

**Caret requirements** allow SemVer compatible updates to a specified version.
An update is allowed if the new version number does not modify the left-most
non-zero digit in the [major, minor, patch] tuple. In this case, if we ran
`cargo update -p time`, cargo would update us to version `0.1.13` if it was
available, but would not update us to `0.2.0`. If instead we had specified the
version string as `^1.0`, cargo would update to `1.1` but not `2.0`. `0.0.x` is
not considered compatible with any other version.

Here are some more examples of caret requirements and the versions that would
be allowed with them:

```notrust
^1.2.3 := >=1.2.3 <2.0.0
^1.2 := >=1.2.0 <2.0.0
^1 := >=1.0.0 <2.0.0
^0.2.3 := >=0.2.3 <0.3.0
^0.0.3 := >=0.0.3 <0.0.4
^0.0 := >=0.0.0 <0.1.0
^0 := >=0.0.0 <1.0.0
```

While SemVer says that there is no compatibility before 1.0.0, many programmers
treat a `0.x.y` release in the same way as a `1.x.y` release: that is, `y` is
incremented for bugfixes, and `x` is incremented for new features.

As such, Cargo considers a `0.x.y` and `0.x.z` version, where `z > y`, to be
compatible.

## Tilde requirements

**Tilde requirements** specify a minimal version with some ability to update.
If you specify a major, minor, and patch version or only a major and minor
version, only patch-level changes are allowed. If you only specify a major
version, then minor- and patch-level changes are allowed.

`~1.2.3` is an example of a tilde requirement.

```notrust
~1.2.3 := >=1.2.3 <1.3.0
~1.2 := >=1.2.0 <1.3.0
~1 := >=1.0.0 <2.0.0
```

## Wildcard requirements

**Wildcard requirements** allow for any version where the wildcard is
positioned.

`*`, `1.*` and `1.2.*` are examples of wildcard requirements.

```notrust
* := >=0.0.0
1.* := >=1.0.0 <2.0.0
1.2.* := >=1.2.0 <1.3.0
```

## Inequality requirements

**Inequality requirements** allow manually specifying a version range or an
exact version to depend on.

Here are some examples of inequality requirements:

```notrust
>= 1.2.0
> 1
< 2
= 1.2.3
```

## Multiple requirements

Multiple version requirements can also be separated with a comma, e.g. `>= 1.2,
< 1.5`.

# Specifying dependencies from git repositories

To depend on a library located in a git repository, the minimum information
you need to specify is the location of the repository with the `git` key:

```toml
[dependencies]
rand = { git = "https://github.com/rust-lang-nursery/rand" }
```

Cargo will fetch the git repository at this location then look for a
`Cargo.toml` for the requested crate anywhere inside the git repository
(not necessarily at the root).

Since we haven’t specified any other information, Cargo assumes that
we intend to use the latest commit on the `master` branch to build our project.
You can combine the `git` key with the `rev`, `tag`, or `branch` keys to
specify something else. Here's an example of specifying that you want to use
the latest commit on a branch named `next`:

```toml
[dependencies]
rand = { git = "https://github.com/rust-lang-nursery/rand", branch = "next" }
```

# Specifying path dependencies

Over time, our `hello_world` project from [the guide](guide.html) has grown
significantly in size! It’s gotten to the point that we probably want to
split out a separate crate for others to use. To do this Cargo supports
**path dependencies** which are typically sub-crates that live within one
repository. Let’s start off by making a new crate inside of our `hello_world`
project:

```shell
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
would need to publish a version of `hello_utils` to [crates.io] (or specify a git
repository location) and specify its version in the dependencies line as well:

```toml
[dependencies]
hello_utils = { path = "hello_utils", version = "0.1.0" }
```

# Overriding Dependencies

Sometimes you may want to override one of Cargo’s dependencies. For example,
let’s say you’re working on a project, `conduit-static`, which depends on
the package `conduit`. You find a bug in `conduit`, and you want to write a
patch and be able to test out your patch by using your version of `conduit`
in `conduit-static`. Here’s what `conduit-static`’s `Cargo.toml` looks like:

```toml
[package]
name = "conduit-static"
version = "0.1.0"
authors = ["Yehuda Katz <wycats@example.com>"]

[dependencies]
conduit = "0.7"
```

You check out a local copy of `conduit`, let’s say in your `~/src` directory:

```shell
$ cd ~/src
$ git clone https://github.com/conduit-rust/conduit.git
```

You’d like to have `conduit-static` use your local version of `conduit`,
rather than the one on [crates.io], while you fix the bug.

Cargo solves this problem by allowing you to have a local configuration
that specifies an **override**. If Cargo finds this configuration when
building your package, it will use the override on your local machine
instead of the source specified in your `Cargo.toml`.

Cargo looks for a directory named `.cargo` up the directory hierarchy of
your project. If your project is in `/path/to/project/conduit-static`,
it will search for a `.cargo` in:

* `/path/to/project/conduit-static`
* `/path/to/project`
* `/path/to`
* `/path`
* `/`

This allows you to specify your overrides in a parent directory that
includes commonly used packages that you work on locally and share them
with all projects.

To specify overrides, create a `.cargo/config` file in some ancestor of
your project’s directory (common places to put it is in the root of
your code directory or in your home directory).

Inside that file, put this:

```toml
paths = ["/path/to/project/conduit"]
```

This array should be filled with directories that contain a `Cargo.toml`. In
this instance, we’re just adding `conduit`, so it will be the only one that’s
overridden. This path must be an absolute path.

Note: using a local configuration to override paths will only work for crates
that have been published to [crates.io]. You cannot use this feature to tell Cargo
how to find local unpublished crates.

More information about local configuration can be found in the [configuration
documentation](config.html).

# Platform specific dependencies


Platform-specific dependencies take the same format, but are listed under a
`target` section. Normally Rust-like `#[cfg]` syntax will be used to define
these sections:

```toml
[target.'cfg(windows)'.dependencies]
winhttp = "0.4.0"

[target.'cfg(unix)'.dependencies]
openssl = "1.0.1"

[target.'cfg(target_pointer_width = "32")'.dependencies]
native = { path = "native/i686" }

[target.'cfg(target_pointer_width = "64")'.dependencies]
native = { path = "native/i686" }
```

Like with Rust, the syntax here supports the `not`, `any`, and `all` operators
to combine various cfg name/value pairs. Note that the `cfg` syntax has only
been available since Cargo 0.9.0 (Rust 1.8.0).

In addition to `#[cfg]` syntax, Cargo also supports listing out the full target
the dependencies would apply to:

```toml
[target.x86_64-pc-windows-gnu.dependencies]
winhttp = "0.4.0"

[target.i686-unknown-linux-gnu.dependencies]
openssl = "1.0.1"
```

If you’re using a custom target specification, quote the full path and file
name:

```toml
[target."x86_64/windows.json".dependencies]
winhttp = "0.4.0"

[target."i686/linux.json".dependencies]
openssl = "1.0.1"
native = { path = "native/i686" }

[target."x86_64/linux.json".dependencies]
openssl = "1.0.1"
native = { path = "native/x86_64" }
```

# Development dependencies

You can add a `[dev-dependencies]` section to your `Cargo.toml` whose format
is equivalent to `[dependencies]`. Dev-dependencies are not used when compiling
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

[crates.io]: https://crates.io/
