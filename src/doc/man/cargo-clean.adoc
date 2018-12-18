= cargo-clean(1)
:idprefix: cargo_clean_
:doctype: manpage
:actionverb: Clean

== NAME

cargo-clean - Remove generated artifacts

== SYNOPSIS

`cargo clean [_OPTIONS_]`

== DESCRIPTION

Remove artifacts from the target directory that Cargo has generated in the
past.

With no options, `cargo clean` will delete the entire target directory.

== OPTIONS

=== Package Selection

When no packages are selected, all packages and all dependencies in the
workspace are cleaned.

*-p* _SPEC_...::
*--package* _SPEC_...::
    Clean only the specified packages. This flag may be specified
    multiple times. See man:cargo-pkgid[1] for the SPEC format.

=== Clean Options

*--doc*::
    This option will cause `cargo clean` to remove only the `doc` directory in
    the target directory.

*--release*::
    Clean all artifacts that were built with the `release` or `bench`
    profiles.

include::options-target-dir.adoc[]

include::options-target-triple.adoc[]

=== Display Options

include::options-display.adoc[]

=== Manifest Options

include::options-manifest-path.adoc[]

include::options-locked.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. Remove the entire target directory:

    cargo clean

. Remove only the release artifacts:

    cargo clean --release

== SEE ALSO
man:cargo[1], man:cargo-build[1]
