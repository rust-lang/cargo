# Cargo Architecture

This document gives a high level overview of Cargo internals. You may
find it useful if you want to contribute to Cargo or if you are
interested in the inner workings of Cargo.


## Subcommands

Cargo is organized as a set of subcommands. All subcommands live in
`src/bin` directory. However, only `src/bin/cargo.rs` file produces an
executable, other files inside the `bin` directory are submodules. See
`src/bin/cargo.rs` for how these subcommands get wired up with the
main executable.

A typical subcommand, such as `src/bin/build.rs`, parses command line
options, reads the configuration files, discovers the Cargo project in
the current directory and delegates the actual implementation to one
of the functions in `src/cargo/ops/mod.rs`. This short file is a good
place to find out about most of the things that Cargo can do.


## Important Data Structures

There are some important data structures which are used throughout
Cargo.

`Config` is available almost everywhere and holds "global"
information, such as `CARGO_HOME` or configuration from
`.cargo/config` files. The `shell` method of `Config` is the entry
point for printing status messages and other info to the console.

`Workspace` is the description of the workspace for the current
working directory. Each workspace contains at least one
`Package`. Each package corresponds to a single `Cargo.toml`, and may
define several `Target`s, such as the library, binaries, integration
test or examples. Targets are crates (each target defines a crate
root, like `src/lib.rs` or `examples/foo.rs`) and are what is actually
compiled by `rustc`.

A typical package defines the single library target and several
auxiliary ones. Packages are a unit of dependency in Cargo, and when
package `foo` depends on package `bar`, that means that each target
from `foo` needs the library target from `bar`.

`PackageId` is the unique identifier of a (possibly remote)
package. It consist of three components: name, version and source
id. Source is the place where the source code for package comes
from. Typical sources are crates.io, a git repository or a folder on
the local hard drive.

`Resolve` is the representation of a directed acyclic graph of package
dependencies, which uses `PackageId`s for nodes. This is the data
structure that is saved to the lock file. If there is no lockfile,
Cargo constructs a resolve by finding a graph of packages which
matches declared dependency specification according to semver.


## Persistence

Cargo is a non-daemon command line application, which means that all
the information used by Cargo must be persisted on the hard drive. The
main sources of information are `Cargo.toml` and `Cargo.lock` files,
`.cargo/config` configuration files and the globally shared registry
of packages downloaded from crates.io, usually located at
`~/.cargo/registry`. See `src/sources/registry` for the specifics of
the registry storage format.


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
example, `[..]` matches any string and `[/]` matches `/` on Unixes and
`\` on windows.
