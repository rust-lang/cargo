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

{{> options-locked }}

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
