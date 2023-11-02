# cargo-rustdoc(1)
{{~*set command="rustdoc"}}
{{~*set actionverb="Document"}}
{{~*set multitarget=true}}

## NAME

cargo-rustdoc --- Build a package's documentation, using specified custom flags

## SYNOPSIS

`cargo rustdoc` [_options_] [`--` _args_]

## DESCRIPTION

The specified target for the current package (or package specified by `-p` if
provided) will be documented with the specified _args_ being passed to the
final rustdoc invocation. Dependencies will not be documented as part of this
command. Note that rustdoc will still unconditionally receive arguments such
as `-L`, `--extern`, and `--crate-type`, and the specified _args_ will simply
be added to the rustdoc invocation.

See <https://doc.rust-lang.org/rustdoc/index.html> for documentation on rustdoc
flags.

{{> description-one-target }}
To pass flags to all rustdoc processes spawned by Cargo, use the
`RUSTDOCFLAGS` [environment variable](../reference/environment-variables.html)
or the `build.rustdocflags` [config value](../reference/config.html).

## OPTIONS

### Documentation Options

{{#options}}

{{#option "`--open`" }}
Open the docs in a browser after building them. This will use your default
browser unless you define another one in the `BROWSER` environment variable
or use the [`doc.browser`](../reference/config.html#docbrowser) configuration
option.
{{/option}}

{{/options}}

{{> section-options-package }}

### Target Selection

When no target selection options are given, `cargo rustdoc` will document all
binary and library targets of the selected package. The binary will be skipped
if its name is the same as the lib target. Binaries are skipped if they have
`required-features` that are missing.

{{> options-targets }}

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

1. Build documentation with custom CSS included from a given file:

       cargo rustdoc --lib -- --extend-css extra.css

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-doc" 1}}, {{man "rustdoc" 1}}
