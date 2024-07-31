# cargo-update(1)

## NAME

cargo-update --- Update dependencies as recorded in the local lock file

## SYNOPSIS

`cargo update` [_options_] _spec_

## DESCRIPTION

This command will update dependencies in the `Cargo.lock` file to the latest
version. If the `Cargo.lock` file does not exist, it will be created with the
latest available versions.

## OPTIONS

### Update Options

{{#options}}

{{#option "_spec_..." }}
Update only the specified packages. This flag may be specified
multiple times. See {{man "cargo-pkgid" 1}} for the SPEC format.

If packages are specified with _spec_, then a conservative update of
the lockfile will be performed. This means that only the dependency specified
by SPEC will be updated. Its transitive dependencies will be updated only if
SPEC cannot be updated without updating dependencies.  All other dependencies
will remain locked at their currently recorded versions.

If _spec_ is not specified, all dependencies are updated.
{{/option}}

{{#option "`--recursive`" }}
When used with _spec_, dependencies of _spec_ are forced to update as well.
Cannot be used with `--precise`.
{{/option}}

{{#option "`--precise` _precise_" }}
When used with _spec_, allows you to specify a specific version number to set
the package to. If the package comes from a git repository, this can be a git
revision (such as a SHA hash or tag).

While not recommended, you can specify a yanked version of a package.
When possible, try other non-yanked SemVer-compatible versions or seek help
from the maintainers of the package.

A compatible `pre-release` version can also be specified even when the version
requirement in `Cargo.toml` doesn't contain any pre-release identifier (nightly only).
{{/option}}

{{#option "`--breaking` _directory_" }}
Update _spec_ to latest SemVer-breaking version.

Version requirements will be modified to allow this update.

This only applies to dependencies when
- The package is a dependency of a workspace member
- The dependency is not renamed
- A SemVer-incompatible version is available
- The "SemVer operator" is used (`^` which is the default)

This option is unstable and available only on the
[nightly channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html)
and requires the `-Z unstable-options` flag to enable.
See <https://github.com/rust-lang/cargo/issues/12425> for more information.
{{/option}}

{{#option "`-w`" "`--workspace`" }}
Attempt to update only packages defined in the workspace. Other packages
are updated only if they don't already exist in the lockfile. This
option is useful for updating `Cargo.lock` after you've changed version
numbers in `Cargo.toml`.
{{/option}}

{{#option "`--dry-run`" }}
Displays what would be updated, but doesn't actually write the lockfile.
{{/option}}

{{/options}}

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

### Manifest Options

{{#options}}

{{> options-manifest-path }}

{{> options-ignore-rust-version }}

{{> options-locked }}

{{> options-lockfile-path }}

{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Update all dependencies in the lockfile:

       cargo update

2. Update only specific dependencies:

       cargo update foo bar

3. Set a specific dependency to a specific version:

       cargo update foo --precise 1.2.3

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-generate-lockfile" 1}}
