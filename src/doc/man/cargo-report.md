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

1. Display the available kinds of reports:

       cargo report --help

## SEE ALSO

{{man "cargo" 1}}, {{man "cargo-report-future-incompatibilities" 1}}
