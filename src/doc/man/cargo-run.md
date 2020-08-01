= cargo-run(1)
:idprefix: cargo_run_
:doctype: manpage
:actionverb: Run

== NAME

cargo-run - Run the current package

== SYNOPSIS

`cargo run [_OPTIONS_] [-- _ARGS_]`

== DESCRIPTION

Run a binary or example of the local package.

All the arguments following the two dashes (`--`) are passed to the binary to
run. If you're passing arguments to both Cargo and the binary, the ones after
`--` go to the binary, the ones before go to Cargo.

== OPTIONS

=== Package Selection

include::options-package.adoc[]

=== Target Selection

When no target selection options are given, `cargo run` will run the binary
target. If there are multiple binary targets, you must pass a target flag to
choose one. Or, the `default-run` field may be specified in the `[package]`
section of `Cargo.toml` to choose the name of the binary to run by default.

*--bin* _NAME_::
    Run the specified binary.

*--example* _NAME_::
    Run the specified example.

include::options-features.adoc[]

=== Compilation Options

include::options-target-triple.adoc[]

include::options-release.adoc[]

=== Output Options

include::options-target-dir.adoc[]

=== Display Options

include::options-display.adoc[]

include::options-message-format.adoc[]

=== Manifest Options

include::options-manifest-path.adoc[]

include::options-locked.adoc[]

=== Common Options

include::options-common.adoc[]

=== Miscellaneous Options

include::options-jobs.adoc[]

include::section-profiles.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. Build the local package and run its main target (assuming only one binary):

    cargo run

. Run an example with extra arguments:

    cargo run --example exname -- --exoption exarg1 exarg2

== SEE ALSO
man:cargo[1], man:cargo-build[1]
