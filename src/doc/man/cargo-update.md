# cargo-update(1)

## NAME

cargo-update - Update dependencies as recorded in the local lock file

## SYNOPSIS

`cargo update` [_options_]

## DESCRIPTION

This command will update dependencies in the `Cargo.lock` file to the latest
version. It requires that the `Cargo.lock` file already exists as generated
by commands such as {{man "cargo-build" 1}} or {{man "cargo-generate-lockfile" 1}}.

## OPTIONS

### Update Options

{{#options}}

{{#option "`-p` _spec_..." "`--package` _spec_..." }}
Update only the specified packages. This flag may be specified
multiple times. See {{man "cargo-pkgid" 1}} for the SPEC format.

If packages are specified with the `-p` flag, then a conservative update of
the lockfile will be performed. This means that only the dependency specified
by SPEC will be updated. Its transitive dependencies will be updated only if
SPEC cannot be updated without updating dependencies.  All other dependencies
will remain locked at their currently recorded versions.

If `-p` is not specified, all dependencies are updated.
{{/option}}

{{#option "`--aggressive`" }}
When used with `-p`, dependencies of _spec_ are forced to update as well.
Cannot be used with `--precise`.
{{/option}}

{{#option "`--precise` _precise_" }}
When used with `-p`, allows you to specify a specific version number to set
the package to. If the package comes from a git repository, this can be a git
revision (such as a SHA hash or tag).
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

       cargo update -p foo -p bar

3. Set a specific dependency to a specific version:

       cargo update -p foo --precise 1.2.3

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-generate-lockfile" 1}}
