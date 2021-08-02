# cargo-rustc(1)
{{*set actionverb="Build"}}

## NAME

cargo-rustc - Compile the current package, and pass extra options to the compiler

## SYNOPSIS

`cargo rustc` [_options_] [`--` _args_]

## DESCRIPTION

The specified target for the current package (or package specified by `-p` if
provided) will be compiled along with all of its dependencies. The specified
_args_ will all be passed to the final compiler invocation, not any of the
dependencies. Note that the compiler will still unconditionally receive
arguments such as `-L`, `--extern`, and `--crate-type`, and the specified
_args_ will simply be added to the compiler invocation.

See <https://doc.rust-lang.org/rustc/index.html> for documentation on rustc
flags.

{{> description-one-target }}
To pass flags to all compiler processes spawned by Cargo, use the `RUSTFLAGS`
[environment variable](../reference/environment-variables.html) or the
`build.rustflags` [config value](../reference/config.html).

## OPTIONS

{{> section-options-package }}

### Target Selection

When no target selection options are given, `cargo rustc` will build all
binary and library targets of the selected package.

{{> options-targets }}

{{> section-features }}

### Compilation Options

{{#options}}

{{> options-target-triple }}

{{> options-release }}

{{> options-ignore-rust-version }}

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
{{/options}}

{{> section-profiles }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Check if your package (not including dependencies) uses unsafe code:

       cargo rustc --lib -- -D unsafe-code

2. Try an experimental flag on the nightly compiler, such as this which prints
   the size of every type:

       cargo rustc --lib -- -Z print-type-sizes

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-build" 1}}, {{man "rustc" 1}}
