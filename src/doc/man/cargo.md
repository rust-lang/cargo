# cargo(1)

## NAME

cargo --- The Rust package manager

## SYNOPSIS

`cargo` [_options_] _command_ [_args_]\
`cargo` [_options_] `--version`\
`cargo` [_options_] `--list`\
`cargo` [_options_] `--help`\
`cargo` [_options_] `--explain` _code_

## DESCRIPTION

This program is a package manager and build tool for the Rust language,
available at <https://rust-lang.org>.

## COMMANDS

### Build Commands

{{man "cargo-bench" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Execute benchmarks of a package.

{{man "cargo-build" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Compile a package.

{{man "cargo-check" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Check a local package and all of its dependencies for errors.

{{man "cargo-clean" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Remove artifacts that Cargo has generated in the past.

{{man "cargo-doc" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Build a package's documentation.

{{man "cargo-fetch" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Fetch dependencies of a package from the network.

{{man "cargo-fix" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Automatically fix lint warnings reported by rustc.

{{man "cargo-run" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Run a binary or example of the local package.

{{man "cargo-rustc" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Compile a package, and pass extra options to the compiler.

{{man "cargo-rustdoc" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Build a package's documentation, using specified custom flags.

{{man "cargo-test" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Execute unit and integration tests of a package.

### Manifest Commands

{{man "cargo-generate-lockfile" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Generate `Cargo.lock` for a project.

{{man "cargo-locate-project" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Print a JSON representation of a `Cargo.toml` file's location.

{{man "cargo-metadata" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Output the resolved dependencies of a package in machine-readable format.

{{man "cargo-pkgid" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Print a fully qualified package specification.

{{man "cargo-tree" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Display a tree visualization of a dependency graph.

{{man "cargo-update" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Update dependencies as recorded in the local lock file.

{{man "cargo-vendor" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Vendor all dependencies locally.

{{man "cargo-verify-project" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Check correctness of crate manifest.

### Package Commands

{{man "cargo-init" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Create a new Cargo package in an existing directory.

{{man "cargo-install" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Build and install a Rust binary.

{{man "cargo-new" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Create a new Cargo package.

{{man "cargo-search" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Search packages in crates.io.

{{man "cargo-uninstall" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Remove a Rust binary.

### Publishing Commands

{{man "cargo-login" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Save an API token from the registry locally.

{{man "cargo-logout" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Remove an API token from the registry locally.

{{man "cargo-owner" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Manage the owners of a crate on the registry.

{{man "cargo-package" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Assemble the local package into a distributable tarball.

{{man "cargo-publish" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Upload a package to the registry.

{{man "cargo-yank" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Remove a pushed crate from the index.

### General Commands

{{man "cargo-help" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Display help information about Cargo.

{{man "cargo-version" 1}}\
&nbsp;&nbsp;&nbsp;&nbsp;Show version information.

## OPTIONS

### Special Options

{{#options}}

{{#option "`-V`" "`--version`" }}
Print version info and exit. If used with `--verbose`, prints extra
information.
{{/option}}

{{#option "`--list`" }}
List all installed Cargo subcommands. If used with `--verbose`, prints extra
information.
{{/option}}

{{#option "`--explain` _code_" }}
Run `rustc --explain CODE` which will print out a detailed explanation of an
error message (for example, `E0004`).
{{/option}}

{{/options}}

### Display Options

{{#options}}

{{> options-display }}

{{/options}}

### Manifest Options

{{#options}}
{{> options-locked }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## FILES

`~/.cargo/`\
&nbsp;&nbsp;&nbsp;&nbsp;Default location for Cargo's "home" directory where it
stores various files. The location can be changed with the `CARGO_HOME`
environment variable.

`$CARGO_HOME/bin/`\
&nbsp;&nbsp;&nbsp;&nbsp;Binaries installed by {{man "cargo-install" 1}} will be located here. If using
[rustup], executables distributed with Rust are also located here.

`$CARGO_HOME/config.toml`\
&nbsp;&nbsp;&nbsp;&nbsp;The global configuration file. See [the reference](../reference/config.html)
for more information about configuration files.

`.cargo/config.toml`\
&nbsp;&nbsp;&nbsp;&nbsp;Cargo automatically searches for a file named `.cargo/config.toml` in the
current directory, and all parent directories. These configuration files
will be merged with the global configuration file.

`$CARGO_HOME/credentials.toml`\
&nbsp;&nbsp;&nbsp;&nbsp;Private authentication information for logging in to a registry.

`$CARGO_HOME/registry/`\
&nbsp;&nbsp;&nbsp;&nbsp;This directory contains cached downloads of the registry index and any
downloaded dependencies.

`$CARGO_HOME/git/`\
&nbsp;&nbsp;&nbsp;&nbsp;This directory contains cached downloads of git dependencies.

Please note that the internal structure of the `$CARGO_HOME` directory is not
stable yet and may be subject to change.

[rustup]: https://rust-lang.github.io/rustup/

## EXAMPLES

1. Build a local package and all of its dependencies:

       cargo build

2. Build a package with optimizations:

       cargo build --release

3. Run tests for a cross-compiled target:

       cargo test --target i686-unknown-linux-gnu

4. Create a new package that builds an executable:

       cargo new foobar

5. Create a package in the current directory:

       mkdir foo && cd foo
       cargo init .

6. Learn about a command's options and usage:

       cargo help clean

## BUGS

See <https://github.com/rust-lang/cargo/issues> for issues.

## SEE ALSO
{{man "rustc" 1}}, {{man "rustdoc" 1}}
