# cargo-run(1)
{{~*set command="run"}}
{{~*set actionverb="Run"}}

## NAME

cargo-run --- Run the current package

## SYNOPSIS

`cargo run` [_options_] [`--` _args_]

## DESCRIPTION

Run a binary or example of the local package.

All the arguments following the two dashes (`--`) are passed to the binary to
run. If you're passing arguments to both Cargo and the binary, the ones after
`--` go to the binary, the ones before go to Cargo.

Unlike {{man "cargo-test" 1}} and {{man "cargo-bench" 1}}, `cargo run` sets the 
working directory of the binary executed to the current working directory, same 
as if it was executed in the shell directly.

## OPTIONS

{{> section-options-package }}

### Target Selection

When no target selection options are given, `cargo run` will run the binary
target. If there are multiple binary targets, you must pass a target flag to
choose one. Or, the `default-run` field may be specified in the `[package]`
section of `Cargo.toml` to choose the name of the binary to run by default.

{{#options}}

{{#option "`--bin` _name_" }}
Run the specified binary.
{{/option}}

{{#option "`--example` _name_" }}
Run the specified example.
{{/option}}

{{/options}}

{{> section-features }}

### Compilation Options

{{#options}}

{{> options-target-triple }}

{{> options-release }}

{{> options-profile }}

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
{{/options}}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Build the local package and run its main target (assuming only one binary):

       cargo run

2. Run an example with extra arguments:

       cargo run --example exname -- --exoption exarg1 exarg2

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-build" 1}}
