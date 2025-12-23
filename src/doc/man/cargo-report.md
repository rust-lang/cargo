# cargo-report(1)

## NAME

cargo-report --- Generate and display various kinds of reports

## SYNOPSIS

`cargo report` _type_ [_options_]

## DESCRIPTION

Displays a report of the given _type_ --- currently, only `future-incompat` is supported

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

1. Display the latest future-incompat report:

       cargo report future-incompat

2. Display the latest future-incompat report for a specific package:

       cargo report future-incompat --package my-dep@0.0.1

## SEE ALSO

{{man "cargo" 1}}, [Future incompat report](../reference/future-incompat-report.html)
