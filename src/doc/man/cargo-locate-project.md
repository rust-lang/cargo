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

### Formatting Options

{{#options}}

{{#option "`-f` _format_" "`--format` _format_" }}
Set the format string for the output representation. The default is a JSON
object holding all of the supported information.

This is an arbitrary string which will be used to display the project location.
The following strings will be replaced with the corresponding value:

- `{root}` — The absolute path of the `Cargo.toml` manifest.

For example you might use `--format '{root}'` or more concisely `-f{root}` to
output just the path and nothing else, or `--format 'root={root}'` to include a
prefix.
{{/option}}

{{/options}}

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
