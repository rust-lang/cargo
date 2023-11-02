# cargo-rustc(1)
{{~*set command="rustc"}}
{{~*set actionverb="Build"}}
{{~*set multitarget=true}}

## NAME

cargo-rustc --- Compile the current package, and pass extra options to the compiler

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

{{> options-targets-bin-auto-built }}

{{> options-targets }}

{{> section-features }}

### Compilation Options

{{#options}}

{{> options-target-triple }}

{{> options-release }}

{{#option "`--profile` _name_" }}
Build with the given profile.

The `rustc` subcommand will treat the following named profiles with special behaviors:

* `check` --- Builds in the same way as the {{man "cargo-check" 1}} command with
  the `dev` profile.
* `test` --- Builds in the same way as the {{man "cargo-test" 1}} command,
  enabling building in test mode which will enable tests and enable the `test`
  cfg option. See [rustc
  tests](https://doc.rust-lang.org/rustc/tests/index.html) for more detail.
* `bench` --- Builds in the same was as the {{man "cargo-bench" 1}} command,
  similar to the `test` profile.

See [the reference](../reference/profiles.html) for more details on profiles.
{{/option}}

{{> options-ignore-rust-version }}

{{> options-timings }}

{{#option "`--crate-type` _crate-type_"}}
Build for the given crate type. This flag accepts a comma-separated list of
1 or more crate types, of which the allowed values are the same as `crate-type`
field in the manifest for configuring a Cargo target. See
[`crate-type` field](../reference/cargo-targets.html#the-crate-type-field)
for possible values.

If the manifest contains a list, and `--crate-type` is provided,
the command-line argument value will override what is in the manifest.

This flag only works when building a `lib` or `example` library target.
{{/option}}

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

1. Check if your package (not including dependencies) uses unsafe code:

       cargo rustc --lib -- -D unsafe-code

2. Try an experimental flag on the nightly compiler, such as this which prints
   the size of every type:

       cargo rustc --lib -- -Z print-type-sizes

3. Override `crate-type` field in Cargo.toml with command-line option:

       cargo rustc --lib --crate-type lib,cdylib

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-build" 1}}, {{man "rustc" 1}}
