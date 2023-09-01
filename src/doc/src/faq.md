# Frequently Asked Questions

## Is the plan to use GitHub as a package repository?

No. The plan for Cargo is to use [crates.io], like npm or Rubygems do with
[npmjs.com][1] and [rubygems.org][3].

We plan to support git repositories as a source of packages forever,
because they can be used for early development and temporary patches,
even when people use the registry as the primary source of packages.

## Why build crates.io rather than use GitHub as a registry?

We think that it’s very important to support multiple ways to download
packages, including downloading from GitHub and copying packages into
your package itself.

That said, we think that [crates.io] offers a number of important benefits, and
will likely become the primary way that people download packages in Cargo.

For precedent, both Node.js’s [npm][1] and Ruby’s [bundler][2] support both a
central registry model as well as a Git-based model, and most packages
are downloaded through the registry in those ecosystems, with an
important minority of packages making use of git-based packages.

[1]: https://www.npmjs.com
[2]: https://bundler.io
[3]: https://rubygems.org

Some of the advantages that make a central registry popular in other
languages include:

* **Discoverability**. A central registry provides an easy place to look
  for existing packages. Combined with tagging, this also makes it
  possible for a registry to provide ecosystem-wide information, such as a
  list of the most popular or most-depended-on packages.
* **Speed**. A central registry makes it possible to easily fetch just
  the metadata for packages quickly and efficiently, and then to
  efficiently download just the published package, and not other bloat
  that happens to exist in the repository. This adds up to a significant
  improvement in the speed of dependency resolution and fetching. As
  dependency graphs scale up, downloading all of the git repositories bogs
  down fast. Also remember that not everybody has a high-speed,
  low-latency Internet connection.

## Will Cargo work with C code (or other languages)?

Yes!

Cargo handles compiling Rust code, but we know that many Rust packages
link against C code. We also know that there are decades of tooling
built up around compiling languages other than Rust.

Our solution: Cargo allows a package to [specify a script](reference/build-scripts.md)
(written in Rust) to run before invoking `rustc`. Rust is leveraged to
implement platform-specific configuration and refactor out common build
functionality among packages.

## Can Cargo be used inside of `make` (or `ninja`, or ...)

Indeed. While we intend Cargo to be useful as a standalone way to
compile Rust packages at the top-level, we know that some people will
want to invoke Cargo from other build tools.

We have designed Cargo to work well in those contexts, paying attention
to things like error codes and machine-readable output modes. We still
have some work to do on those fronts, but using Cargo in the context of
conventional scripts is something we designed for from the beginning and
will continue to prioritize.

## Does Cargo handle multi-platform packages or cross-compilation?

Rust itself provides facilities for configuring sections of code based
on the platform. Cargo also supports [platform-specific
dependencies][target-deps], and we plan to support more per-platform
configuration in `Cargo.toml` in the future.

[target-deps]: reference/specifying-dependencies.md#platform-specific-dependencies

In the longer-term, we’re looking at ways to conveniently cross-compile
packages using Cargo.

## Does Cargo support environments, like `production` or `test`?

We support environments through the use of [profiles] to support:

[profiles]: reference/profiles.md

* environment-specific flags (like `-g --opt-level=0` for development
  and `--opt-level=3` for production).
* environment-specific dependencies (like `hamcrest` for test assertions).
* environment-specific `#[cfg]`
* a `cargo test` command

## Does Cargo work on Windows?

Yes!

All commits to Cargo are required to pass the local test suite on Windows.
If you encounter an issue while running on Windows, we consider it a bug, so [please file an
issue][cargo-issues].

[cargo-issues]: https://github.com/rust-lang/cargo/issues

## Why have `Cargo.lock` in version control?

While [`cargo new`] defaults to tracking `Cargo.lock` in version control,
whether you do is dependent on the needs of your package.

The purpose of a `Cargo.lock` lockfile is to describe the state of the world at
the time of a successful build.
Cargo uses the lockfile to provide deterministic builds at different times and
on different systems,
by ensuring that the exact same dependencies and versions are used as when the
`Cargo.lock` file was originally generated.

Deterministic builds help with
- Running `git bisect` to find the root cause of a bug
- Ensuring CI only fails due to new commits and not external factors
- Reducing confusion when contributors see different behavior as compared to
  other contributors or CI

Having this snapshot of dependencies can also help when projects need to be
verified against consistent versions of dependencies, like when
- Verifying a minimum-supported Rust version (MSRV) that is less than the latest
  version of a dependency supports
- Verifying human readable output which won't have compatibility guarantees
  (e.g. snapshot testing error messages to ensure they are "understandable", a
  metric too fuzzy to automate)

However, this determinism can give a false sense of security because
`Cargo.lock` does not affect the consumers of your package, only `Cargo.toml` does that.
For example:
- [`cargo install`] will select the latest dependencies unless `--locked` is
  passed in.
- New dependencies, like those added with [`cargo add`], will be locked to the latest version

The lockfile can also be a source of merge conflicts.

For strategies to verify newer versions of dependencies via CI,
see [Verifying Latest Dependencies](guide/continuous-integration.md#verifying-latest-dependencies).

[`cargo new`]: commands/cargo-new.md
[`cargo add`]: commands/cargo-add.md
[`cargo install`]: commands/cargo-install.md

## Can libraries use `*` as a version for their dependencies?

**As of January 22nd, 2016, [crates.io] rejects all packages (not just libraries)
with wildcard dependency constraints.**

While libraries _can_, strictly speaking, they should not. A version requirement
of `*` says “This will work with every version ever”, which is never going
to be true. Libraries should always specify the range that they do work with,
even if it’s something as general as “every 1.x.y version”.

## Why `Cargo.toml`?

As one of the most frequent interactions with Cargo, the question of why the
configuration file is named `Cargo.toml` arises from time to time. The leading
capital-`C` was chosen to ensure that the manifest was grouped with other
similar configuration files in directory listings. Sorting files often puts
capital letters before lowercase letters, ensuring files like `Makefile` and
`Cargo.toml` are placed together. The trailing `.toml` was chosen to emphasize
the fact that the file is in the [TOML configuration
format](https://toml.io/).

Cargo does not allow other names such as `cargo.toml` or `Cargofile` to
emphasize the ease of how a Cargo repository can be identified. An option of
many possible names has historically led to confusion where one case was handled
but others were accidentally forgotten.

[crates.io]: https://crates.io/

## How can Cargo work offline?

Cargo is often used in situations with limited or no network access such as
airplanes, CI environments, or embedded in large production deployments. Users
are often surprised when Cargo attempts to fetch resources from the network, and
hence the request for Cargo to work offline comes up frequently.

Cargo, at its heart, will not attempt to access the network unless told to do
so. That is, if no crates come from crates.io, a git repository, or some other
network location, Cargo will never attempt to make a network connection. As a
result, if Cargo attempts to touch the network, then it's because it needs to
fetch a required resource.

Cargo is also quite aggressive about caching information to minimize the amount
of network activity. It will guarantee, for example, that if `cargo build` (or
an equivalent) is run to completion then the next `cargo build` is guaranteed to
not touch the network so long as `Cargo.toml` has not been modified in the
meantime. This avoidance of the network boils down to a `Cargo.lock` existing
and a populated cache of the crates reflected in the lock file. If either of
these components are missing, then they're required for the build to succeed and
must be fetched remotely.

As of Rust 1.11.0, Cargo understands a new flag, `--frozen`, which is an
assertion that it shouldn't touch the network. When passed, Cargo will
immediately return an error if it would otherwise attempt a network request.
The error should include contextual information about why the network request is
being made in the first place to help debug as well. Note that this flag *does
not change the behavior of Cargo*, it simply asserts that Cargo shouldn't touch
the network as a previous command has been run to ensure that network activity
shouldn't be necessary.

The `--offline` flag was added in Rust 1.36.0. This flag tells Cargo to not
access the network, and try to proceed with available cached data if possible.
You can use [`cargo fetch`] in one project to download dependencies before
going offline, and then use those same dependencies in another project with
the `--offline` flag (or [configuration value][offline config]).

For more information about vendoring, see documentation on [source
replacement][replace].

[replace]: reference/source-replacement.md
[`cargo fetch`]: commands/cargo-fetch.md
[offline config]: reference/config.md#netoffline

## Why is Cargo rebuilding my code?

Cargo is responsible for incrementally compiling crates in your project. This
means that if you type `cargo build` twice the second one shouldn't rebuild your
crates.io dependencies, for example. Nevertheless bugs arise and Cargo can
sometimes rebuild code when you're not expecting it!

We've long [wanted to provide better diagnostics about
this](https://github.com/rust-lang/cargo/issues/2904) but unfortunately haven't
been able to make progress on that issue in quite some time. In the meantime,
however, you can debug a rebuild at least a little by setting the `CARGO_LOG`
environment variable:

```sh
$ CARGO_LOG=cargo::core::compiler::fingerprint=info cargo build
```

This will cause Cargo to print out a lot of information about diagnostics and
rebuilding. This can often contain clues as to why your project is getting
rebuilt, although you'll often need to connect some dots yourself since this
output isn't super easy to read just yet. Note that the `CARGO_LOG` needs to be
set for the command that rebuilds when you think it should not. Unfortunately
Cargo has no way right now of after-the-fact debugging "why was that rebuilt?"

Some issues we've seen historically which can cause crates to get rebuilt are:

* A build script prints `cargo:rerun-if-changed=foo` where `foo` is a file that
  doesn't exist and nothing generates it. In this case Cargo will keep running
  the build script thinking it will generate the file but nothing ever does. The
  fix is to avoid printing `rerun-if-changed` in this scenario.

* Two successive Cargo builds may differ in the set of features enabled for some
  dependencies. For example if the first build command builds the whole
  workspace and the second command builds only one crate, this may cause a
  dependency on crates.io to have a different set of features enabled, causing
  it and everything that depends on it to get rebuilt. There's unfortunately not
  really a great fix for this, although if possible it's best to have the set of
  features enabled on a crate constant regardless of what you're building in
  your workspace.

* Some filesystems exhibit unusual behavior around timestamps. Cargo primarily
  uses timestamps on files to govern whether rebuilding needs to happen, but if
  you're using a nonstandard filesystem it may be affecting the timestamps
  somehow (e.g. truncating them, causing them to drift, etc). In this scenario,
  feel free to open an issue and we can see if we can accommodate the filesystem
  somehow.

* A concurrent build process is either deleting artifacts or modifying files.
  Sometimes you might have a background process that either tries to build or
  check your project. These background processes might surprisingly delete some
  build artifacts or touch files (or maybe just by accident), which can cause
  rebuilds to look spurious! The best fix here would be to wrangle the
  background process to avoid clashing with your work.

If after trying to debug your issue, however, you're still running into problems
then feel free to [open an
issue](https://github.com/rust-lang/cargo/issues/new)!

## What does "version conflict" mean and how to resolve it?

> failed to select a version for `x` which could resolve this conflict

Have you seen the error message above?

This is one of the most annoying error message for Cargo users. There are several 
situations may lead us to a version conflict. Below we'll walk through possible 
causes and provide diagnostic techniques to help you out there:

- The project and its dependencies use [links] to repeatedly link the local 
  library. Cargo forbids linking two packages with the same native library, so 
  even with multiple layers of dependencies it is not allowed. In this case, the 
  error message will prompt: `Only one package in the dependency graph may specify 
  the same links value`, you may need to manually check and delete duplicate link 
  values. The community also have [conventions in place] to alleviate this.

- When depending on different crates in the project, if these crates use the same 
  dependent library, but the version used is restricted, making it impossible to 
  determine the correct version, it will also cause conflicts. The error message 
  will prompt: `all possible versions conflict with previously selected packages`. 
  You may need to modify the version requirements to make them consistent.

- If there are multiple versions of dependencies in the project, when using 
  [`direct-minimal-versions`], the minimum version requirements cannot be met, 
  which will cause conflicts. You may need to modify version requirements of your
  direct dependencies to meet the minimum SemVer version accordingly.

- If the dependent crate does not have the features you choose, it will also 
  cause conflicts. At this time, you need to check the dependent version and its 
  features.

- Conflicts may occur when merging branches or PRs, if there are non-trivial 
  conflicts, you can reset all "yours" changes, fix all other conflicts in the 
  branch, and then run some cargo command (like `cargo tree` or `cargo check`), 
  which should re-update the lockfile with your own local changes. If you previously 
  ran some `cargo update` commands in your branch, you can re-run them that this 
  time. The community has been looking to resolve merge conflicts with `Cargo.lock` 
  and `Cargo.toml` using a [custom merge tool].


[links]: https://doc.rust-lang.org/cargo/reference/resolver.html#links
[conventions in place]: https://doc.rust-lang.org/cargo/reference/build-scripts.html#-sys-packages
[`direct-minimal-versions`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#direct-minimal-versions
[custom merge tool]: https://github.com/rust-lang/cargo/issues/1818
