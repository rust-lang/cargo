# cargo-help(1)

## NAME

cargo-help --- Get help for a Cargo command

## SYNOPSIS

`cargo help` [_subcommand_]

## DESCRIPTION

Prints a help message for the given command.

For nested commands (commands with subcommands), separate the command levels
with spaces. For example, `cargo help report future-incompatibilities` displays
help for the `cargo report future-incompatibilities` command.

The dash-joined form (`cargo help report-future-incompatibilities`) also works
for compatibility.

Note that spaces only separate hierarchy levels between a parent command and its
subcommands. Dashes that are part of a command's name (like `generate-lockfile`)
must always be preserved.

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

2. Get help for a nested command:

       cargo help report future-incompatibilities

3. The dash-joined form also works:

       cargo help report-future-incompatibilities

4. Help is also available with the `--help` flag:

       cargo build --help

## SEE ALSO
{{man "cargo" 1}}
