# cargo-report-future-incompatibilities(1)
{{~*set actionverb="Display a report for"}}

## NAME

cargo-report-future-incompatibilities --- Reports any crates which will eventually stop compiling

## SYNOPSIS

`cargo report future-incompatibilities` [_options_]

## DESCRIPTION

Displays a report of future-incompatible warnings that were emitted during
previous builds.
These are warnings for changes that may become hard errors in the future,
causing dependencies to stop building in a future version of rustc.

For more, see the chapter on [Future incompat report](../reference/future-incompat-report.html).

## OPTIONS

{{#options}}

{{#option "`--id` _id_" }}
Show the report with the specified Cargo-generated id.
If not specified, shows the most recent report.
{{/option}}

{{/options}}

{{> section-options-package }}

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

1. Display the latest future-incompat report:

       cargo report future-incompat

2. Display the latest future-incompat report for a specific package:

       cargo report future-incompat --package my-dep@0.0.1

## SEE ALSO

{{man "cargo" 1}}, {{man "cargo-report" 1}}, {{man "cargo-build" 1}}
