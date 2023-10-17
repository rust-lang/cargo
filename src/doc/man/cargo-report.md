# cargo-report(1)

## NAME

cargo-report --- Generate and display various kinds of reports

## SYNOPSIS

`cargo report` _type_ [_options_]

### DESCRIPTION

Displays a report of the given _type_ --- currently, only `future-incompat` is supported

## OPTIONS

{{#options}}

{{#option "`--id` _id_" }}
Show the report with the specified Cargo-generated id
{{/option}}

{{#option "`-p` _spec_..." "`--package` _spec_..." }}
Only display a report for the specified package
{{/option}}

{{/options}}

## EXAMPLES

1. Display the latest future-incompat report:

       cargo report future-incompat

2. Display the latest future-incompat report for a specific package:

       cargo report future-incompat --package my-dep:0.0.1

## SEE ALSO
[Future incompat report](../reference/future-incompat-report.html)

{{man "cargo" 1}}
