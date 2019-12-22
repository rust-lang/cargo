= cargo-rustdoc(1)
:idprefix: cargo_rustdoc_
:doctype: manpage
:actionverb: Document

== NAME

cargo-rustdoc - Build a package's documentation, using specified custom flags

== SYNOPSIS

`cargo rustdoc [_OPTIONS_] [-- _ARGS_]`

== DESCRIPTION

The specified target for the current package (or package specified by `-p` if
provided) will be documented with the specified _ARGS_ being passed to the
final rustdoc invocation. Dependencies will not be documented as part of this
command. Note that rustdoc will still unconditionally receive arguments such
as `-L`, `--extern`, and `--crate-type`, and the specified _ARGS_ will simply
be added to the rustdoc invocation.

See https://doc.rust-lang.org/rustdoc/index.html for documentation on rustdoc
flags.

include::description-one-target.adoc[]
To pass flags to all rustdoc processes spawned by Cargo, use the
`RUSTDOCFLAGS` linkcargo:reference/environment-variables.html[environment variable]
or the `build.rustdocflags` linkcargo:reference/config.html[config value].

== OPTIONS

=== Documentation Options

*--open*::
    Open the docs in a browser after building them. This will use your default
    browser unless you define another one in the `BROWSER` environment
    variable.

=== Package Selection

include::options-package.adoc[]

=== Target Selection

When no target selection options are given, `cargo rustdoc` will document all
binary and library targets of the selected package. The binary will be skipped
if its name is the same as the lib target. Binaries are skipped if they have
`required-features` that are missing.

include::options-targets.adoc[]

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

. Build documentation with custom CSS included from a given file:

    cargo rustdoc --lib -- --extend-css extra.css

== SEE ALSO
man:cargo[1], man:cargo-doc[1], man:rustdoc[1]
