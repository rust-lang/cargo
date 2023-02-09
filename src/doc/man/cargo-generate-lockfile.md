# cargo-generate-lockfile(1)

## NAME

cargo-generate-lockfile --- Generate the lockfile for a package

## SYNOPSIS

`cargo generate-lockfile` [_options_]

## DESCRIPTION

This command will create the `Cargo.lock` lockfile for the current package or
workspace. If the lockfile already exists, it will be rebuilt with the latest
available version of every package.

See also {{man "cargo-update" 1}} which is also capable of creating a `Cargo.lock`
lockfile and has more options for controlling update behavior.

## OPTIONS

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

1. Create or update the lockfile for the current package or workspace:

       cargo generate-lockfile

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-update" 1}}
