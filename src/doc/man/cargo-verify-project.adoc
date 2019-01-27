= cargo-verify-project(1)
:idprefix: cargo_verify-project_
:doctype: manpage

== NAME

cargo-verify-project - Check correctness of crate manifest

== SYNOPSIS

`cargo verify-project [_OPTIONS_]`

== DESCRIPTION

This command will parse the local manifest and check its validity. It emits a
JSON object with the result. A successful validation will display:

    {"success":"true"}

An invalid workspace will display:

    {"invalid":"human-readable error message"}

== OPTIONS

=== Display Options

include::options-display.adoc[]

=== Manifest Options

include::options-manifest-path.adoc[]

include::options-locked.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

== Exit Status

0::
    The workspace is OK.

1::
    The workspace is invalid.

== EXAMPLES

. Check the current workspace for errors:

    cargo verify-project

== SEE ALSO
man:cargo[1], man:cargo-package[1]
