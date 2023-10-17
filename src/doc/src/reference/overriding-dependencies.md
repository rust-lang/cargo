# Overriding Dependencies

The desire to override a dependency can arise through a number of scenarios.
Most of them, however, boil down to the ability to work with a crate before
it's been published to [crates.io]. For example:

* A crate you're working on is also used in a much larger application you're
  working on, and you'd like to test a bug fix to the library inside of the
  larger application.
* An upstream crate you don't work on has a new feature or a bug fix on the
  master branch of its git repository which you'd like to test out.
* You're about to publish a new major version of your crate, but you'd like to
  do integration testing across an entire package to ensure the new major
  version works.
* You've submitted a fix to an upstream crate for a bug you found, but you'd
  like to immediately have your application start depending on the fixed
  version of the crate to avoid blocking on the bug fix getting merged.

These scenarios can be solved with the [`[patch]` manifest
section](#the-patch-section).

This chapter walks through a few different use cases, and includes details
on the different ways to override a dependency.

* Example use cases
    * [Testing a bugfix](#testing-a-bugfix)
    * [Working with an unpublished minor version](#working-with-an-unpublished-minor-version)
        * [Overriding repository URL](#overriding-repository-url)
    * [Prepublishing a breaking change](#prepublishing-a-breaking-change)
    * [Using `[patch]` with multiple versions](#using-patch-with-multiple-versions)
* Reference
    * [The `[patch]` section](#the-patch-section)
    * [The `[replace]` section](#the-replace-section)
    * [`paths` overrides](#paths-overrides)

> **Note**: See also specifying a dependency with [multiple locations], which
> can be used to override the source for a single dependency declaration in a
> local package.

## Testing a bugfix

Let's say you're working with the [`uuid` crate] but while you're working on it
you discover a bug. You are, however, quite enterprising so you decide to also
try to fix the bug! Originally your manifest will look like:

[`uuid` crate]: https://crates.io/crates/uuid

```toml
[package]
name = "my-library"
version = "0.1.0"

[dependencies]
uuid = "1.0"
```

First thing we'll do is to clone the [`uuid` repository][uuid-repository]
locally via:

```console
$ git clone https://github.com/uuid-rs/uuid.git
```

Next we'll edit the manifest of `my-library` to contain:

```toml
[patch.crates-io]
uuid = { path = "../path/to/uuid" }
```

Here we declare that we're *patching* the source `crates-io` with a new
dependency. This will effectively add the local checked out version of `uuid` to
the crates.io registry for our local package.

Next up we need to ensure that our lock file is updated to use this new version
of `uuid` so our package uses the locally checked out copy instead of one from
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

```console
$ cargo build
   Compiling uuid v1.0.0 (.../uuid)
   Compiling my-library v0.1.0 (.../my-library)
    Finished dev [unoptimized + debuginfo] target(s) in 0.32 secs
```

And that's it! You're now building with the local version of `uuid` (note the
path in parentheses in the build output). If you don't see the local path version getting
built then you may need to run `cargo update uuid --precise $version` where
`$version` is the version of the locally checked out copy of `uuid`.

Once you've fixed the bug you originally found the next thing you'll want to do
is to likely submit that as a pull request to the `uuid` crate itself. Once
you've done this then you can also update the `[patch]` section. The listing
inside of `[patch]` is just like the `[dependencies]` section, so once your pull
request is merged you could change your `path` dependency to:

```toml
[patch.crates-io]
uuid = { git = 'https://github.com/uuid-rs/uuid.git' }
```

[uuid-repository]: https://github.com/uuid-rs/uuid

## Working with an unpublished minor version

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

[dependencies]
uuid = "1.0.1"

[patch.crates-io]
uuid = { git = 'https://github.com/uuid-rs/uuid.git' }
```

Note that our local dependency on `uuid` has been updated to `1.0.1` as it's
what we'll actually require once the crate is published. This version doesn't
exist on crates.io, though, so we provide it with the `[patch]` section of the
manifest.

Now when our library is built it'll fetch `uuid` from the git repository and
resolve to 1.0.1 inside the repository instead of trying to download a version
from crates.io. Once 1.0.1 is published on crates.io the `[patch]` section can
be deleted.

It's also worth noting that `[patch]` applies *transitively*. Let's say you use
`my-library` in a larger package, such as:

```toml
[package]
name = "my-binary"
version = "0.1.0"

[dependencies]
my-library = { git = 'https://example.com/git/my-library' }
uuid = "1.0"

[patch.crates-io]
uuid = { git = 'https://github.com/uuid-rs/uuid.git' }
```

Remember that `[patch]` is applicable *transitively* but can only be defined at
the *top level* so we consumers of `my-library` have to repeat the `[patch]` section
if necessary. Here, though, the new `uuid` crate applies to *both* our dependency on
`uuid` and the `my-library -> uuid` dependency. The `uuid` crate will be resolved to
one version for this entire crate graph, 1.0.1, and it'll be pulled from the git
repository.

### Overriding repository URL

In case the dependency you want to override isn't loaded from `crates.io`,
you'll have to change a bit how you use `[patch]`. For example, if the
dependency is a git dependency, you can override it to a local path with:

```toml
[patch."https://github.com/your/repository"]
my-library = { path = "../my-library/path" }
```

And that's it!

## Prepublishing a breaking change

Let's take a look at working with a new major version of a crate, typically
accompanied with breaking changes. Sticking with our previous crates, this
means that we're going to be creating version 2.0.0 of the `uuid` crate. After
we've submitted all changes upstream we can update our manifest for
`my-library` to look like:

```toml
[dependencies]
uuid = "2.0"

[patch.crates-io]
uuid = { git = "https://github.com/uuid-rs/uuid.git", branch = "2.0.0" }
```

And that's it! Like with the previous example the 2.0.0 version doesn't actually
exist on crates.io but we can still put it in through a git dependency through
the usage of the `[patch]` section. As a thought exercise let's take another
look at the `my-binary` manifest from above again as well:

```toml
[package]
name = "my-binary"
version = "0.1.0"

[dependencies]
my-library = { git = 'https://example.com/git/my-library' }
uuid = "1.0"

[patch.crates-io]
uuid = { git = 'https://github.com/uuid-rs/uuid.git', branch = '2.0.0' }
```

Note that this will actually resolve to two versions of the `uuid` crate. The
`my-binary` crate will continue to use the 1.x.y series of the `uuid` crate but
the `my-library` crate will use the `2.0.0` version of `uuid`. This will allow you
to gradually roll out breaking changes to a crate through a dependency graph
without being forced to update everything all at once.

## Using `[patch]` with multiple versions

You can patch in multiple versions of the same crate with the `package` key
used to rename dependencies. For example let's say that the `serde` crate has
a bugfix that we'd like to use to its `1.*` series but we'd also like to
prototype using a `2.0.0` version of serde we have in our git repository. To
configure this we'd do:

```toml
[patch.crates-io]
serde = { git = 'https://github.com/serde-rs/serde.git' }
serde2 = { git = 'https://github.com/example/serde.git', package = 'serde', branch = 'v2' }
```

The first `serde = ...` directive indicates that serde `1.*` should be used
from the git repository (pulling in the bugfix we need) and the second `serde2
= ...` directive indicates that the `serde` package should also be pulled from
the `v2` branch of `https://github.com/example/serde`. We're assuming here
that `Cargo.toml` on that branch mentions version `2.0.0`.

Note that when using the `package` key the `serde2` identifier here is actually
ignored. We simply need a unique name which doesn't conflict with other patched
crates.

## The `[patch]` section

The `[patch]` section of `Cargo.toml` can be used to override dependencies
with other copies. The syntax is similar to the
[`[dependencies]`][dependencies] section:

```toml
[patch.crates-io]
foo = { git = 'https://github.com/example/foo.git' }
bar = { path = 'my/local/bar' }

[dependencies.baz]
git = 'https://github.com/example/baz.git'

[patch.'https://github.com/example/baz']
baz = { git = 'https://github.com/example/patched-baz.git', branch = 'my-branch' }
```

> **Note**: The `[patch]` table can also be specified as a [configuration
> option](config.md), such as in a `.cargo/config.toml` file or a CLI option
> like `--config 'patch.crates-io.rand.path="rand"'`. This can be useful for
> local-only changes that you don't want to commit, or temporarily testing a
> patch.

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

Cargo only looks at the patch settings in the `Cargo.toml` manifest at the
root of the workspace. Patch settings defined in dependencies will be
ignored.

## The `[replace]` section

> **Note**: `[replace]` is deprecated. You should use the
> [`[patch]`](#the-patch-section) table instead.

This section of Cargo.toml can be used to override dependencies with other
copies. The syntax is similar to the `[dependencies]` section:

```toml
[replace]
"foo:0.1.0" = { git = 'https://github.com/example/foo.git' }
"bar:1.0.2" = { path = 'my/local/bar' }
```

Each key in the `[replace]` table is a [package ID
specification](pkgid-spec.md), which allows arbitrarily choosing a node in the
dependency graph to override (the 3-part version number is required). The
value of each key is the same as the `[dependencies]` syntax for specifying
dependencies, except that you can't specify features. Note that when a crate
is overridden the copy it's overridden with must have both the same name and
version, but it can come from a different source (e.g., git or a local path).

Cargo only looks at the replace settings in the `Cargo.toml` manifest at the
root of the workspace. Replace settings defined in dependencies will be
ignored.

## `paths` overrides

Sometimes you're only temporarily working on a crate and you don't want to have
to modify `Cargo.toml` like with the `[patch]` section above. For this use
case Cargo offers a much more limited version of overrides called **path
overrides**.

Path overrides are specified through [`.cargo/config.toml`](config.md) instead of
`Cargo.toml`. Inside of `.cargo/config.toml` you'll specify a key called `paths`:

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

> **Note**: using a local configuration to override paths will only work for
> crates that have been published to [crates.io]. You cannot use this feature
> to tell Cargo how to find local unpublished crates.


[crates.io]: https://crates.io/
[multiple locations]: specifying-dependencies.md#multiple-locations
[dependencies]: specifying-dependencies.md
