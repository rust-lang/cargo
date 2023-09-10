# cargo-clean(1)
{{~*set command="clean"}}
{{~*set actionverb="Clean"}}
{{~*set multitarget=true}}

## NAME

cargo-clean --- Remove generated artifacts

## SYNOPSIS

`cargo clean` [_options_]

## DESCRIPTION

Remove artifacts from the target directory that Cargo has generated in the
past.

With no options, `cargo clean` will delete the entire target directory.

## OPTIONS

### Package Selection

When no packages are selected, all packages and all dependencies in the
workspace are cleaned.

{{#options}}
{{#option "`-p` _spec_..." "`--package` _spec_..." }}
Clean only the specified packages. This flag may be specified
multiple times. See {{man "cargo-pkgid" 1}} for the SPEC format.
{{/option}}
{{/options}}

### Clean Options

{{#options}}

{{#option "`--dry-run`" }}
Displays a summary of what would be deleted without deleting anything.
Use with `--verbose` to display the actual files that would be deleted.
{{/option}}

{{#option "`--doc`" }}
This option will cause `cargo clean` to remove only the `doc` directory in
the target directory.
{{/option}}

{{#option "`--release`" }}
Remove all artifacts in the `release` directory.
{{/option}}

{{#option "`--profile` _name_" }}
Remove all artifacts in the directory with the given profile name.
{{/option}}

{{> options-target-dir }}

{{> options-target-triple }}

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

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Remove the entire target directory:

       cargo clean

2. Remove only the release artifacts:

       cargo clean --release

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-build" 1}}
