= cargo-check(1)
:idprefix: cargo_check_
:doctype: manpage
:actionverb: Check

== NAME

cargo-check - Check the current package

== SYNOPSIS

`cargo check [_OPTIONS_]`

== DESCRIPTION

Check a local package and all of its dependencies for errors. This will
essentially compile the packages without performing the final step of code
generation, which is faster than running `cargo build`. The compiler will save
metadata files to disk so that future runs will reuse them if the source has
not been modified.

== OPTIONS

=== Package Selection

include::options-packages.adoc[]

=== Target Selection

When no target selection options are given, `cargo check` will check all
binary and library targets of the selected packages. Binaries are skipped if
they have `required-features` that are missing.

include::options-targets.adoc[]

include::options-features.adoc[]

=== Compilation Options

include::options-target-triple.adoc[]

include::options-release.adoc[]

include::options-profile.adoc[]

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

. Check the local package for errors:

    cargo check

. Check all targets, including unit tests:

    cargo check --all-targets --profile=test

== SEE ALSO
man:cargo[1], man:cargo-build[1]
