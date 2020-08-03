# cargo-locate-project(1)

## NAME

cargo-locate-project - Print a JSON representation of a Cargo.toml file's location

## SYNOPSIS

`cargo locate-project` [_options_]

## DESCRIPTION

This command will print a JSON object to stdout with the full path to the
`Cargo.toml` manifest.

See also {{man "cargo-metadata" 1}} which is capable of returning the path to a
workspace root.

## OPTIONS

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

### Manifest Options

{{#options}}
{{> options-manifest-path }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Display the path to the manifest based on the current directory:

       cargo locate-project

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-metadata" 1}}
