= cargo-version(1)
:idprefix: cargo_version_
:doctype: manpage

== NAME

cargo-version - Show version information

== SYNOPSIS

`cargo version [_OPTIONS_]`

== DESCRIPTION

Displays the version of Cargo.

== OPTIONS

*-v*::
*--verbose*::
    Display additional version information.

== EXAMPLES

. Display the version:

    cargo version

. The version is also available via flags:

    cargo --version
    cargo -V

. Display extra version information:

    cargo -Vv

== SEE ALSO
man:cargo[1]
