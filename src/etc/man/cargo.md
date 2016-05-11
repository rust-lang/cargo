% CARGO(1)
%
% May 2016


# NAME
cargo - The Rust package manager


# SYNOPSIS
*cargo* \<COMMAND> [\<ARGS>...]


# DESCRIPTION
This program is a package manager for the Rust language, available at
<http://rust-lang.org>.


# OPTIONS
-h, --help
:   Display a help message.

-V, --version
:   Print version information and exit.

--list
:   List all available cargo commands.

-v, --verbose
:   Use verbose output.

--color
:   Configure coloring of output.


# COMMANDS
To get extended information about commands, run *cargo help \<command>* or
*man cargo-command*

cargo-build(1)
:   Compile the current project.

cargo-clean(1)
:   Remove the target directory with build output.

cargo-doc(1)
:   Build this project's and its dependencies' documentation.

cargo-init(1)
:   Create a new cargo project in the current directory.

cargo-install(1)
:   Install a Rust binary.

cargo-new(1)
:   Create a new cargo project.

cargo-run(1)
:   Build and execute src/main.rs.

cargo-test(1)
:   Run the tests for the package.

cargo-bench(1)
:   Run the benchmarks for the package.

cargo-update(1)
:   Update dependencies in Cargo.lock.

cargo-package(1)
:   Generate a source tarball for the current package.

cargo-publish(1)
:   Package and upload this project to the registry.

cargo-uninstall(1)
:   Remove a Rust binary.

cargo-search(1)
:   Search registry for crates.

cargo-version(1)
:   Print cargo's version and exit.


# FILES
~/.cargo
:   Directory in which Cargo stores repository data. Cargo can be instructed to
    use a *.cargo* subdirectory in a different location by setting the
    **CARGO_HOME** environment variable.


# EXAMPLES
Build a local package and all of its dependencies

    $ cargo build

Build a package with optimizations

    $ cargo build --release

Run tests for a cross-compiled target

    $ cargo test --target i686-unknown-linux-gnu

Create a new project that builds an executable

    $ cargo new --init foobar

Create a project in the current directory

    $ mkdir foo && cd foo
    $ cargo init .

Learn about a command's options and usage

    $ cargo help clean


# SEE ALSO
rustc(1), rustdoc(1)


# BUGS
See <https://github.com/rust-lang/cargo/issues> for issues.


# COPYRIGHT
This work is dual-licensed under Apache 2.0 and MIT terms.  See *COPYRIGHT*
file in the cargo source distribution.
