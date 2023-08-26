# cargo-remove(1)
{{~*set command="remove"}}
{{~*set actionverb="Remove"}}
{{~*set nouns="removes"}}

## NAME

cargo-remove --- Remove dependencies from a Cargo.toml manifest file

## SYNOPSIS

`cargo remove` [_options_] _dependency_...

## DESCRIPTION

Remove one or more dependencies from a `Cargo.toml` manifest.

## OPTIONS

### Section options

{{#options}}

{{#option "`--dev`" }}
Remove as a [development dependency](../reference/specifying-dependencies.html#development-dependencies).
{{/option}}

{{#option "`--build`" }}
Remove as a [build dependency](../reference/specifying-dependencies.html#build-dependencies).
{{/option}}

{{#option "`--target` _target_" }}
Remove as a dependency to the [given target platform](../reference/specifying-dependencies.html#platform-specific-dependencies).

To avoid unexpected shell expansions, you may use quotes around each target, e.g., `--target 'cfg(unix)'`.
{{/option}}

{{/options}}

### Miscellaneous Options

{{#options}}

{{#option "`--dry-run`" }}
Don't actually write to the manifest.
{{/option}}

{{/options}}

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

### Manifest Options

{{#options}}
{{> options-manifest-path }}

{{> options-locked }}
{{/options}}

### Package Selection

{{#options}}

{{#option "`-p` _spec_..." "`--package` _spec_..." }}
Package to remove from.
{{/option}}

{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Remove `regex` as a dependency

       cargo remove regex

2. Remove `trybuild` as a dev-dependency

       cargo remove --dev trybuild

3. Remove `nom` from the `x86_64-pc-windows-gnu` dependencies table

       cargo remove --target x86_64-pc-windows-gnu nom

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-add" 1}}
