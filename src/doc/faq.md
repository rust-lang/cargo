% Frequently Asked Questions

# Is the plan to use Github as a package repository?

No. The plan for Cargo is to have a central registry of packages, like
npm or Rubygems.

We plan to support git repositories as a source of packages forever,
because they can be used for early development and temporary patches,
even when people use the registry as the primary source of packages.

At the moment, the Rust team is still making regular changes to the
language, and the Rust project recommends using nightly builds. This
means that for now, package authors make regular changes in order to
track the latest Rust. This makes downloading the latest `master` from
Github the best approach to getting packages at the current point in the
community's lifecycle.

# Why build a package registry rather than use Github as a registry?

We think that it's very important to support multiple ways to download
packages, including downloading from Github and copying packages into
your project itself.

That said, we think that a central registry offers a number of important
benefits, and will likely become the primary way that people download
packages in Cargo.

For precedent, both Node.js's [npm][1] and Ruby's [bundler][2] support both a
central registry model as well as a Git-based model, and most packages
are downloaded through the registry in those ecosystems, with an
important minority of packages making use of git-based packages.

[1]: https://www.npmjs.org
[2]: https://bundler.io

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

# Will Cargo work with C code (or other languages)?

Yes!

Cargo handles compiling Rust code, but we know that many Rust projects
link against C code. We also know that there are decades of tooling
built up around compiling languages other than Rust.

Our solution: Cargo allows a package to specify a script to run
before invoking `rustc`. We plan to add support for platform-specific
configuration, so you can use `make` on Linux and `cmake` on BSD, for
example.

# Can Cargo be used inside of `make` (or `ninja`, or ...)

Indeed. While we intend Cargo to be useful as a standalone way to
compile Rust projects at the top-level, we know that some people will
want to invoke Cargo from other build tools.

We have designed Cargo to work well in those contexts, paying attention
to things like error codes and machine-readable output modes. We still
have some work to do on those fronts, but using Cargo in the context of
conventional scripts is something we designed for from the beginning and
will continue to prioritize.

# Does Cargo handle multi-platform projects or cross-compilation?

Rust itself provides facilities for configuring sections of code based
on the platform. We plan to support per-platform configuration in
`Cargo.toml`, including platform-specific dependencies, in the near
future.

In the longer-term, we're looking at ways to conveniently cross-compile
projects using Cargo.

# Does Cargo support environments, like `production` or `test`?

We are planning on support environments in the near future, that can
support:

* environment-specific flags (like `-g --opt-level=0` for development
  and `--opt-level=3` for production).
* environment-specific dependencies (like `hamcrest` for test assertions).
* environment-specific `#[cfg]`
* a `cargo test` command

We also plan to make it possible to specify "profiles", which can
specify flags or dependencies for a combination of multiple environments
and platforms ("use `fsevents`, but only in OSX in `development` or
`test`").

# Does Cargo work on Windows?

Yes!

All commits to Cargo are required to pass the local test suite on Windows.
If, however, you find a Windows issue, we consider it a bug, so [please file an
issue][3].

[3]: https://github.com/rust-lang/cargo/issues

# Why do binaries have `Cargo.lock` in version control, but not libraries?

The purpose of a `Cargo.lock` is to describe the state of the world at the time
of a successful build. It is then used to provide deterministic builds across
whatever machine is building the project by ensuring that the exact same
dependencies are being compiled.

This property is most desirable from applications and projects which are at the
very end of the dependency chain (binaries). As a result, it is recommended that
all binaries check in their `Cargo.lock`.

For libraries the situation is somewhat different. A library is not only used by
the library developers, but also any downstream consumers of the library. Users
dependent on the library will not inspect the library's `Cargo.lock` (even if it
exists). This is precisely because a library should **not** be deterministically
recompiled for all users of the library.

If a library ends up being used transitively by several dependencies, it's
likely that just a single copy of the library is desired (based on semver
compatibility). If all libraries were to check in their `Cargo.lock`, then
multiple copies of the library would be used, and perhaps even a version
conflict.

In other words, libraries specify semver requirements for their dependencies but
cannot see the full picture. Only end products like binaries have a full
picture to decide what versions of dependencies should be used.
