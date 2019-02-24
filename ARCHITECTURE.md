# Cargo Architecture

This document gives a high level overview of Cargo internals. You may
find it useful if you want to contribute to Cargo or if you are
interested in the inner workings of Cargo.

The purpose of Cargo is to formalize a canonical Rust workflow, by automating
the standard tasks associated with distributing software. Cargo simplifies
structuring a new project, adding dependencies, writing and running unit tests,
and more.


## Subcommands

Cargo is a single binary composed of a set of [`clap`] subcommands. All subcommands live in
`src/bin/cargo/commands` directory. `src/bin/cargo/main.rs` is the entry point.

Each subcommand, such as [`src/bin/cargo/commands/build.rs`], has its own API
interface, similarly to Git's, parsing command line options, reading the
configuration files, discovering the Cargo project in the current directory and
delegating the actual implementation to one
of the functions in [`src/cargo/ops/mod.rs`]. This short file is a good
place to find out about most of the things that Cargo can do.
Subcommands are designed to pipe to one another, and custom subcommands make
Cargo easy to extend and attach tools to.

[`clap`]: https://clap.rs/
[`src/bin/cargo/commands/build.rs`]: src/bin/cargo/commands/build.rs
[`src/cargo/ops/mod.rs`]: src/cargo/ops/mod.rs


## Important Data Structures

There are some important data structures which are used throughout
Cargo.

[`Config`] is available almost everywhere and holds "global"
information, such as `CARGO_HOME` or configuration from
`.cargo/config` files. The [`shell`] method of [`Config`] is the entry
point for printing status messages and other info to the console.

[`Workspace`] is the description of the workspace for the current
working directory. Each workspace contains at least one
[`Package`]. Each package corresponds to a single `Cargo.toml`, and may
define several [`Target`]s, such as the library, binaries, integration
test or examples. Targets are crates (each target defines a crate
root, like `src/lib.rs` or `examples/foo.rs`) and are what is actually
compiled by `rustc`.

A typical package defines the single library target and several
auxiliary ones. Packages are a unit of dependency in Cargo, and when
package `foo` depends on package `bar`, that means that each target
from `foo` needs the library target from `bar`.

[`PackageId`] is the unique identifier of a (possibly remote)
package. It consist of three components: name, version and source
id. Source is the place where the source code for package comes
from. Typical sources are crates.io, a git repository or a folder on
the local hard drive.

[`Resolve`] is the representation of a directed acyclic graph of package
dependencies, which uses [`PackageId`]s for nodes. This is the data
structure that is saved to the lock file. If there is no lock file,
Cargo constructs a resolve by finding a graph of packages which
matches declared dependency specification according to semver.

[`Config`]: https://docs.rs/cargo/latest/cargo/util/config/struct.Config.html
[`shell`]: https://docs.rs/cargo/latest/cargo/util/config/struct.Config.html#method.shell
[`Workspace`]: https://docs.rs/cargo/latest/cargo/core/struct.Workspace.html
[`Package`]: https://docs.rs/cargo/latest/cargo/core/package/struct.Package.html
[`Target`]: https://docs.rs/cargo/latest/cargo/core/manifest/struct.Target.html
[`PackageId`]: https://docs.rs/cargo/latest/cargo/core/package_id/struct.PackageId.html
[`Resolve`]: https://docs.rs/cargo/latest/cargo/core/struct.Resolve.html


## Persistence

Cargo is a non-daemon command line application, which means that all
the information used by Cargo must be persisted on the hard drive. The
main sources of information are `Cargo.toml` and `Cargo.lock` files,
`.cargo/config` configuration files and the globally shared registry
of packages downloaded from crates.io, usually located at
`~/.cargo/registry`. See [`src/cargo/sources/registry`] for the specifics of
the registry storage format.

[`src/cargo/sources/registry`]: src/cargo/sources/registry


## Concurrency

Cargo is mostly single threaded. The only concurrency inside a single
instance of Cargo happens during compilation, when several instances
of `rustc` are invoked in parallel to build independent
targets. However there can be several different instances of Cargo
process running concurrently on the system. Cargo guarantees that this
is always safe by using file locks when accessing potentially shared
data like the registry or the target directory.


## Tests

Cargo has an impressive test suite located in the `tests` folder. Most
of the test are integration: a project structure with `Cargo.toml` and
rust source code is created in a temporary directory, `cargo` binary
is invoked via `std::process::Command` and then stdout and stderr are
verified against the expected output. To simplify testing, several
macros of the form `[MACRO]` are used in the expected output. For
example, `[..]` matches any string.

To see stdout and stderr streams of the subordinate process, add `.stream()`
call to the built-up `Execs`:

```rust
// Before
p.cargo("run").run();

// After
p.cargo("run").stream().run();
```

Alternatively to build and run a custom version of cargo simply run `cargo build`
and execute `target/debug/cargo`. Note that `+nightly`/`+stable` (and variants),
being [rustup] features, won't work when executing the locally
built cargo binary directly, you have to instead build with `cargo +nightly build`
and run with `rustup run` (e.g `rustup run nightly
<path-to-cargo>/target/debug/cargo <args>..`) (or set the `RUSTC` env var to point
to nightly rustc).

[rustup]: https://rustup.rs/


## Logging

Cargo uses [`env_logger`], so you can set
`RUST_LOG` environment variable to get the logs. This is useful both for diagnosing
bugs in stable Cargo and for local development. Cargo also has internal hierarchical
profiling infrastructure, which is activated via `CARGO_PROFILE` variable

```
# Outputs all logs with levels debug and higher
$ RUST_LOG=debug cargo generate-lockfile

# Don't forget that you can filter by module as well
$ RUST_LOG=cargo::core::resolver=trace cargo generate-lockfile

# Output first three levels of profiling info
$ CARGO_PROFILE=3 cargo generate-lockfile
```

[`env_logger`]: https://docs.rs/env_logger/*/env_logger/
