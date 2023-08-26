# cargo-check(1)
{{~*set command="check"}}
{{~*set actionverb="Check"}}
{{~*set multitarget=true}}

## NAME

cargo-check --- Check the current package

## SYNOPSIS

`cargo check` [_options_]

## DESCRIPTION

Check a local package and all of its dependencies for errors. This will
essentially compile the packages without performing the final step of code
generation, which is faster than running `cargo build`. The compiler will save
metadata files to disk so that future runs will reuse them if the source has
not been modified. Some diagnostics and errors are only emitted during code
generation, so they inherently won't be reported with `cargo check`.

## OPTIONS

{{> section-package-selection }}

### Target Selection

When no target selection options are given, `cargo check` will check all
binary and library targets of the selected packages. Binaries are skipped if
they have `required-features` that are missing.

{{> options-targets }}

{{> section-features }}

### Compilation Options

{{#options}}

{{> options-target-triple }}

{{> options-release }}

{{> options-profile-legacy-check }}

{{> options-ignore-rust-version }}

{{> options-timings }}

{{/options}}

### Output Options

{{#options}}
{{> options-target-dir }}
{{/options}}

### Display Options

{{#options}}
{{> options-display }}

{{> options-message-format }}
{{/options}}

### Manifest Options

{{#options}}
{{> options-manifest-path }}

{{> options-locked }}
{{/options}}

{{> section-options-common }}

### Miscellaneous Options

{{#options}}
{{> options-jobs }}
{{> options-keep-going }}
{{> options-future-incompat }}
{{/options}}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Check the local package for errors:

       cargo check

2. Check all targets, including unit tests:

       cargo check --all-targets --profile=test

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-build" 1}}
