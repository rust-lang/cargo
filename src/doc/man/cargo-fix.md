# cargo-fix(1)
{{*set actionverb="Fix"}}

## NAME

cargo-fix - Automatically fix lint warnings reported by rustc

## SYNOPSIS

`cargo fix` [_options_]

## DESCRIPTION

This Cargo subcommand will automatically take rustc's suggestions from
diagnostics like warnings and apply them to your source code. This is intended
to help automate tasks that rustc itself already knows how to tell you to fix!
The `cargo fix` subcommand is also being developed for the Rust 2018 edition
to provide code the ability to easily opt-in to the new edition without having
to worry about any breakage.

Executing `cargo fix` will under the hood execute {{man "cargo-check" 1}}. Any warnings
applicable to your crate will be automatically fixed (if possible) and all
remaining warnings will be displayed when the check process is finished. For
example if you'd like to prepare for the 2018 edition, you can do so by
executing:

    cargo fix --edition

which behaves the same as `cargo check --all-targets`.

`cargo fix` is only capable of fixing code that is normally compiled with
`cargo check`. If code is conditionally enabled with optional features, you
will need to enable those features for that code to be analyzed:

    cargo fix --edition --features foo

Similarly, other `cfg` expressions like platform-specific code will need to
pass `--target` to fix code for the given target.

    cargo fix --edition --target x86_64-pc-windows-gnu

If you encounter any problems with `cargo fix` or otherwise have any questions
or feature requests please don't hesitate to file an issue at
<https://github.com/rust-lang/cargo>

## OPTIONS

### Fix options

{{#options}}

{{#option "`--broken-code`" }}
Fix code even if it already has compiler errors. This is useful if `cargo fix`
fails to apply the changes. It will apply the changes and leave the broken
code in the working directory for you to inspect and manually fix.
{{/option}}

{{#option "`--edition`" }}
Apply changes that will update the code to the latest edition. This will not
update the edition in the `Cargo.toml` manifest, which must be updated
manually.
{{/option}}

{{#option "`--edition-idioms`" }}
Apply suggestions that will update code to the preferred style for the current
edition.
{{/option}}

{{#option "`--allow-no-vcs`" }}
Fix code even if a VCS was not detected.
{{/option}}

{{#option "`--allow-dirty`" }}
Fix code even if the working directory has changes.
{{/option}}

{{#option "`--allow-staged`" }}
Fix code even if the working directory has staged changes.
{{/option}}

{{/options}}

{{> section-package-selection }}

### Target Selection

When no target selection options are given, `cargo fix` will fix all targets
(`--all-targets` implied). Binaries are skipped if they have
`required-features` that are missing.

{{> options-targets }}

{{> section-features }}

### Compilation Options

{{#options}}

{{> options-target-triple }}

{{> options-release }}

{{> options-profile }}

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

1. Apply compiler suggestions to the local package:

       cargo fix

2. Convert a 2015 edition to 2018:

       cargo fix --edition

3. Apply suggested idioms for the current edition:

       cargo fix --edition-idioms

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-check" 1}}
