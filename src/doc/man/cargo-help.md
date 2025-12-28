# cargo-help(1)

## NAME

cargo-help --- Get help for a Cargo command

## SYNOPSIS

`cargo help` [_subcommand_]

## DESCRIPTION

Prints a help message for the given command.

## OPTIONS

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

## EXAMPLES

1. Get help for a command:

       cargo help build

2. Help is also available with the `--help` flag:

       cargo build --help

## SEE ALSO
{{man "cargo" 1}}
