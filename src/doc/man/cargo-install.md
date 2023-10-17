# cargo-install(1)
{{~*set command="install"}}
{{~*set actionverb="Install"}}
{{~*set temp-target-dir=true}}

## NAME

cargo-install --- Build and install a Rust binary

## SYNOPSIS

`cargo install` [_options_] _crate_[@_version_]...\
`cargo install` [_options_] `--path` _path_\
`cargo install` [_options_] `--git` _url_ [_crate_...]\
`cargo install` [_options_] `--list`

## DESCRIPTION

This command manages Cargo's local set of installed binary crates. Only
packages which have executable `[[bin]]` or `[[example]]` targets can be
installed, and all executables are installed into the installation root's
`bin` folder. By default only binaries, not examples, are installed.

{{> description-install-root }}

There are multiple sources from which a crate can be installed. The default
location is crates.io but the `--git`, `--path`, and `--registry` flags can
change this source. If the source contains more than one package (such as
crates.io or a git repository with multiple crates) the _crate_ argument is
required to indicate which crate should be installed.

Crates from crates.io can optionally specify the version they wish to install
via the `--version` flags, and similarly packages from git repositories can
optionally specify the branch, tag, or revision that should be installed. If a
crate has multiple binaries, the `--bin` argument can selectively install only
one of them, and if you'd rather install examples the `--example` argument can
be used as well.

If the package is already installed, Cargo will reinstall it if the installed
version does not appear to be up-to-date. If any of the following values
change, then Cargo will reinstall the package:

- The package version and source.
- The set of binary names installed.
- The chosen features.
- The profile (`--profile`).
- The target (`--target`).

Installing with `--path` will always build and install, unless there are
conflicting binaries from another package. The `--force` flag may be used to
force Cargo to always reinstall the package.

If the source is crates.io or `--git` then by default the crate will be built
in a temporary target directory. To avoid this, the target directory can be
specified by setting the `CARGO_TARGET_DIR` environment variable to a relative
path. In particular, this can be useful for caching build artifacts on
continuous integration systems.

### Dealing with the Lockfile

By default, the `Cargo.lock` file that is included with the package will be
ignored. This means that Cargo will recompute which versions of dependencies
to use, possibly using newer versions that have been released since the
package was published. The `--locked` flag can be used to force Cargo to use
the packaged `Cargo.lock` file if it is available. This may be useful for
ensuring reproducible builds, to use the exact same set of dependencies that
were available when the package was published. It may also be useful if a
newer version of a dependency is published that no longer builds on your
system, or has other problems. The downside to using `--locked` is that you
will not receive any fixes or updates to any dependency. Note that Cargo did
not start publishing `Cargo.lock` files until version 1.37, which means
packages published with prior versions will not have a `Cargo.lock` file
available.

### Configuration Discovery

This command operates on system or user level, not project level.
This means that the local [configuration discovery] is ignored.
Instead, the configuration discovery begins at `$CARGO_HOME/config.toml`. 
If the package is installed with `--path $PATH`, the local configuration 
will be used, beginning discovery at `$PATH/.cargo/config.toml`.

[configuration discovery]: ../reference/config.html#hierarchical-structure

## OPTIONS

### Install Options

{{#options}}

{{#option "`--vers` _version_" "`--version` _version_" }}
Specify a version to install. This may be a [version
requirement](../reference/specifying-dependencies.md), like `~1.2`, to have Cargo
select the newest version from the given requirement. If the version does not
have a requirement operator (such as `^` or `~`), then it must be in the form
_MAJOR.MINOR.PATCH_, and will install exactly that version; it is *not*
treated as a caret requirement like Cargo dependencies are.
{{/option}}

{{#option "`--git` _url_" }}
Git URL to install the specified crate from.
{{/option}}

{{#option "`--branch` _branch_" }}
Branch to use when installing from git.
{{/option}}

{{#option "`--tag` _tag_" }}
Tag to use when installing from git.
{{/option}}

{{#option "`--rev` _sha_" }}
Specific commit to use when installing from git.
{{/option}}

{{#option "`--path` _path_" }}
Filesystem path to local crate to install.
{{/option}}

{{#option "`--list`" }}
List all installed packages and their versions.
{{/option}}

{{#option "`-f`" "`--force`" }}
Force overwriting existing crates or binaries. This can be used if a package
has installed a binary with the same name as another package. This is also
useful if something has changed on the system that you want to rebuild with,
such as a newer version of `rustc`.
{{/option}}

{{#option "`--no-track`" }}
By default, Cargo keeps track of the installed packages with a metadata file
stored in the installation root directory. This flag tells Cargo not to use or
create that file. With this flag, Cargo will refuse to overwrite any existing
files unless the `--force` flag is used. This also disables Cargo's ability to
protect against multiple concurrent invocations of Cargo installing at the
same time.
{{/option}}

{{#option "`--bin` _name_..." }}
Install only the specified binary.
{{/option}}

{{#option "`--bins`" }}
Install all binaries. This is the default behavior.
{{/option}}

{{#option "`--example` _name_..." }}
Install only the specified example.
{{/option}}

{{#option "`--examples`" }}
Install all examples.
{{/option}}

{{#option "`--root` _dir_" }}
Directory to install packages into.
{{/option}}

{{> options-registry }}

{{> options-index }}

{{/options}}

{{> section-features }}

### Compilation Options

{{#options}}

{{> options-target-triple }}

{{> options-target-dir }}

{{#option "`--debug`" }}
Build with the `dev` profile instead of the `release` profile.
See also the `--profile` option for choosing a specific profile by name.
{{/option}}

{{> options-profile }}

{{> options-ignore-rust-version }}

{{> options-timings }}

{{/options}}

### Manifest Options

{{#options}}
{{> options-locked }}
{{/options}}

### Miscellaneous Options

{{#options}}
{{> options-jobs }}
{{> options-keep-going }}
{{/options}}

### Display Options

{{#options}}
{{> options-display }}

{{> options-message-format }}

{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Install or upgrade a package from crates.io:

       cargo install ripgrep

2. Install or reinstall the package in the current directory:

       cargo install --path .

3. View the list of installed packages:

       cargo install --list

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-uninstall" 1}}, {{man "cargo-search" 1}}, {{man "cargo-publish" 1}}
