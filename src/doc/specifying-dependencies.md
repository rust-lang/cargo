% Specifying Dependencies

Your crates can depend on other libraries from [crates.io], `git` repositories, or
subdirectories on your local file system. You can also temporarily override the
location of a dependency— for example, to be able to test out a bug fix in the
dependency that you are working on locally. You can have different
dependencies for different platforms, and dependencies that are only used during
development. Let's take a look at how to do each of these.

# Specifying dependencies from crates.io

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
non-zero digit in the major, minor, patch grouping. In this case, if we ran
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

This compatibility convention is different from SemVer in the way it treats
versions before 1.0.0. While SemVer says there is no compatibility before
1.0.0, Cargo considers `0.x.y` to be compatible with `0.x.z`, where `y ≥ z`
and `x > 0`.

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

# Specifying dependencies from `git` repositories

To depend on a library located in a `git` repository, the minimum information
you need to specify is the location of the repository with the `git` key:

```toml
[dependencies]
rand = { git = "https://github.com/rust-lang-nursery/rand" }
```

Cargo will fetch the `git` repository at this location then look for a
`Cargo.toml` for the requested crate anywhere inside the `git` repository
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
would need to publish a version of `hello_utils` to [crates.io](https://crates.io)
(or specify a `git` repository location) and specify its version in
the dependencies line as well:

```toml
[dependencies]
hello_utils = { path = "hello_utils", version = "0.1.0" }
```

# Overriding dependencies

There are a number of methods in Cargo to support overriding dependencies and
otherwise controlling the dependency graph. These options are typically, though,
only available at the workspace level and aren't propagated through
dependencies. In other words, "applications" have the ability to override
dependencies but "libraries" do not.

The desire to override a dependency or otherwise alter some dependencies can
arise through a number of scenarios. Most of them, however, boil down to the
ability to work with a crate before it's been published to crates.io. For
example:

* A crate you're working on is also used in a much larger application you're
  working on, and you'd like to test a bug fix to the library inside of the
  larger application.
* An upstream crate you don't work on has a new feature or a bug fix on the
  master branch of its git repository which you'd like to test out.
* You're about to publish a new major version of your crate, but you'd like to
  do integration testing across an entire project to ensure the new major
  version works.
* You've submitted a fix to an upstream crate for a bug you found, but you'd
  like to immediately have your application start depending on the fixed version
  of the crate to avoid blocking on the bug fix getting merged.

These scenarios are currently all solved with the [`[patch]` manifest
section][patch-section]. Note that the `[patch]` feature is not yet currently
stable and will be released on 2017-08-31. Historically some of these scenarios
have been solved with [the `[replace]` section][replace-section], but we'll
document the `[patch]` section here.

[patch-section]: manifest.html#the-patch-section
[replace-section]: manifest.html#the-replace-section

### Testing a bugfix

Let's say you're working with the [`uuid`] crate but while you're working on it
you discover a bug. You are, however, quite enterprising so you decide to also
try out to fix the bug! Originally your manifest will look like:

[`uuid`](https://crates.io/crates/uuid)

```toml
[package]
name = "my-library"
version = "0.1.0"
authors = ["..."]

[dependencies]
uuid = "1.0"
```

First thing we'll do is to clone the [`uuid` repository][uuid-repository]
locally via:

```shell
$ git clone https://github.com/rust-lang-nursery/uuid
```

Next we'll edit the manifest of `my-library` to contain:

```toml
[patch.crates-io]
uuid = { path = "../path/to/uuid" }
```

Here we declare that we're *patching* the source `crates-io` with a new
dependency. This will effectively add the local checked out version of `uuid` to
the crates.io registry for our local project.

Next up we need to ensure that our lock file is updated to use this new version
of `uuid` so our project uses the locally checked out copy instead of one from
crates.io. The way `[patch]` works is that it'll load the dependency at
`../path/to/uuid` and then whenever crates.io is queried for versions of `uuid`
it'll *also* return the local version.

This means that the version number of the local checkout is significant and will
affect whether the patch is used. Our manifest declared `uuid = "1.0"` which
means we'll only resolve to `>= 1.0.0, < 2.0.0`, and Cargo's greedy resolution
algorithm also means that we'll resolve to the maximum version within that
range. Typically this doesn't matter as the version of the git repository will
already be greater or match the maximum version published on crates.io, but it's
important to keep this in mind!

In any case, typically all you need to do now is:

```shell
$ cargo build
   Compiling uuid v1.0.0 (file://.../uuid)
   Compiling my-library v0.1.0 (file://.../my-library)
    Finished dev [unoptimized + debuginfo] target(s) in 0.32 secs
```

And that's it! You're now building with the local version of `uuid` (note the
`file://` in the build output). If you don't see the `file://` version getting
built then you may need to run `cargo update -p uuid --precise $version` where
`$version` is the version of the locally checked out copy of `uuid`.

Once you've fixed the bug you originally found the next thing you'll want to do
is to likely submit that as a pull request to the `uuid` crate itself. Once
you've done this then you can also update the `[patch]` section. The listing
inside of `[patch]` is just like the `[dependencies]` section, so once your pull
request is merged you could change your `path` dependency to:

```toml
[patch.crates-io]
uuid = { git = 'https://github.com/rust-lang-nursery/uuid' }
```

[uuid-repository]: https://github.com/rust-lang-nursery/uuid

### Working with an unpublished minor version

Let's now shift gears a bit from bug fixes to adding features. While working on
`my-library` you discover that a whole new feature is needed in the `uuid`
crate. You've implemented this feature, tested it locally above with `[patch]`,
and submitted a pull request. Let's go over how you continue to use and test it
before it's actually published.

Let's also say that the current version of `uuid` on crates.io is `1.0.0`, but
since then the master branch of the git repository has updated to `1.0.1`. This
branch includes your new feature you submitted previously. To use this
repository we'll edit our `Cargo.toml` to look like

```toml
[package]
name = "my-library"
version = "0.1.0"
authors = ["..."]

[dependencies]
uuid = "1.0.1"

[patch.crates-io]
uuid = { git = 'https://github.com/rust-lang-nursery/uuid' }
```

Note that our local dependency on `uuid` has been updated to `1.0.1` as it's
what we'll actually require once the crate is published. This version doesn't
exist on crates.io, though, so we provide it with the `[patch]` section of the
manifest.

Now when our library is built it'll fetch `uuid` from the git repository and
resolve to 1.0.1 inside the repository instead of trying to download a version
from crates.io. Once 1.0.1 is published on crates.io the `[patch]` section can
be deleted.

It's also worth nothing that `[patch]` applies *transitively*. Let's say you use
`my-library` in a larger project, such as:

```toml
[package]
name = "my-binary"
version = "0.1.0"
authors = ["..."]

[dependencies]
my-library = { git = 'https://example.com/git/my-library' }
uuid = "1.0"

[patch.crates-io]
uuid = { git = 'https://github.com/rust-lang-nursery/uuid' }
```

Remember that `[patch]` is only applicable at the *top level* so we consumers of
`my-library` have to repeat the `[patch]` section if necessary. Here, though,
the new `uuid` crate applies to *both* our dependency on `uuid` and the
`my-library -> uuid` dependency. The `uuid` crate will be resolved to one
version for this entire crate graph, 1.0.1, and it'll be pulled from the git
repository.

### Prepublishing a breaking change

As a final scenario, let's take a look at working with a new major version of a
crate, typically accompanied with breaking changes. Sticking with our previous
crates, this means that we're going to be creating version 2.0.0 of the `uuid`
crate. After we've submitted all changes upstream we can update our manifest for
`my-library` to look like:

```toml
[dependencies]
uuid = "2.0"

[patch.crates-io]
uuid = { git = "https://github.com/rust-lang-nursery/uuid", branch = "2.0.0" }
```

And that's it! Like with the previous example the 2.0.0 version doesn't actually
exist on crates.io but we can still put it in through a git dependency through
the usage of the `[patch]` section. As a thought exercise let's take another
look at the `my-binary` manifest from above again as well:

```toml
[package]
name = "my-binary"
version = "0.1.0"
authors = ["..."]

[dependencies]
my-library = { git = 'https://example.com/git/my-library' }
uuid = "1.0"

[patch.crates-io]
uuid = { git = 'https://github.com/rust-lang-nursery/uuid', version = '2.0.0' }
```

Note that this will actually resolve to two versions of the `uuid` crate. The
`my-binary` crate will continue to use the 1.x.y series of the `uuid` crate but
the `my-library` crate will use the 2.0.0 version of `uuid`. This will allow you
to gradually roll out breaking changes to a crate through a dependency graph
without being force to update everything all at once.

### Overriding with local dependencies

Sometimes you're only temporarily working on a crate and you don't want to have
to modify `Cargo.toml` like with the `[patch]` section above. For this use
case Cargo offers a much more limited version of overrides called **path
overrides**.

Path overrides are specified through `.cargo/config` instead of `Cargo.toml`,
and you can find [more documentation about this configuration][config-docs].
Inside of `.cargo/config` you'll specify a key called `paths`:

[config-docs]: config.html

```toml
paths = ["/path/to/uuid"]
```

This array should be filled with directories that contain a `Cargo.toml`. In
this instance, we’re just adding `uuid`, so it will be the only one that’s
overridden. This path can be either absolute or relative to the directory that
contains the `.cargo` folder.

Path overrides are more restricted than the `[patch]` section, however, in
that they cannot change the structure of the dependency graph. When a
path replacement is used then the previous set of dependencies
must all match exactly to the new `Cargo.toml` specification. For example this
means that path overrides cannot be used to test out adding a dependency to a
crate, instead `[patch]` must be used in that situation. As a result usage of a
path override is typically isolated to quick bug fixes rather than larger
changes.

Note: using a local configuration to override paths will only work for crates
that have been published to [crates.io]. You cannot use this feature to tell
Cargo how to find local unpublished crates.

# Platform specific dependencies


Platform-specific dependencies take the same format, but are listed under a
`target` section. Normally Rust-like `#[cfg]` syntax will be used to define
these sections:

```toml
[target.'cfg(windows)'.dependencies]
winhttp = "0.4.0"

[target.'cfg(unix)'.dependencies]
openssl = "1.0.1"

[target.'cfg(target_arch = "x86")'.dependencies]
native = { path = "native/i686" }

[target.'cfg(target_arch = "x86_64")'.dependencies]
native = { path = "native/x86_64" }
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

[crates.io]: https://crates.io/

# Build dependencies

You can depend on other Cargo-based crates for use in your build scripts.
Dependencies are declared through the `build-dependencies` section of the
manifest:

```toml
[build-dependencies]
gcc = "0.3"
```

The build script **does not** have access to the dependencies listed
in the `dependencies` or `dev-dependencies` section. Build
dependencies will likewise not be available to the package itself
unless listed under the `dependencies` section as well. A package
itself and its build script are built separately, so their
dependencies need not coincide. Cargo is kept simpler and cleaner by
using independent dependencies for independent purposes.

# Choosing features

If a package you depend on offers conditional features, you can
specify which to use:

```toml
[dependencies.awesome]
version = "1.3.5"
default-features = false # do not include the default features, and optionally
                         # cherry-pick individual features
features = ["secure-password", "civet"]
```

More information about features can be found in the
[manifest documentation](manifest.html#the-features-section).
