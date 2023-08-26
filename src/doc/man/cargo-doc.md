# cargo-doc(1)
{{~*set command="doc"}}
{{~*set actionverb="Document"}}
{{~*set multitarget=true}}

## NAME

cargo-doc --- Build a package's documentation

## SYNOPSIS

`cargo doc` [_options_]

## DESCRIPTION

Build the documentation for the local package and all dependencies. The output
is placed in `target/doc` in rustdoc's usual format.

## OPTIONS

### Documentation Options

{{#options}}

{{#option "`--open`" }}
Open the docs in a browser after building them. This will use your default
browser unless you define another one in the `BROWSER` environment variable
or use the [`doc.browser`](../reference/config.html#docbrowser) configuration
option.
{{/option}}

{{#option "`--no-deps`" }}
Do not build documentation for dependencies.
{{/option}}

{{#option "`--document-private-items`" }}
Include non-public items in the documentation. This will be enabled by default if documenting a binary target.
{{/option}}

{{/options}}

{{> section-package-selection }}

### Target Selection

When no target selection options are given, `cargo doc` will document all
binary and library targets of the selected package. The binary will be skipped
if its name is the same as the lib target. Binaries are skipped if they have
`required-features` that are missing.

The default behavior can be changed by setting `doc = false` for the target in
the manifest settings. Using target selection options will ignore the `doc`
flag and will always document the given target.

{{#options}}
{{> options-targets-lib-bin }}

{{#option "`--example` _name_..." }}
{{actionverb}} the specified example. This flag may be specified multiple times
and supports common Unix glob patterns.
{{/option}}

{{#option "`--examples`" }}
{{actionverb}} all example targets.
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

1. Build the local package documentation and its dependencies and output to
   `target/doc`.

       cargo doc

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-rustdoc" 1}}, {{man "rustdoc" 1}}
