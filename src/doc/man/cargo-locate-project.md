= cargo-locate-project(1)
:idprefix: cargo_locate-project_
:doctype: manpage

== NAME

cargo-locate-project - Print a JSON representation of a Cargo.toml file's location

== SYNOPSIS

`cargo locate-project [_OPTIONS_]`

== DESCRIPTION

This command will print a JSON object to stdout with the full path to the
`Cargo.toml` manifest.

See also man:cargo-metadata[1] which is capable of returning the path to a
workspace root.

== OPTIONS

=== Display Options

include::options-display.adoc[]

=== Manifest Options

include::options-manifest-path.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. Display the path to the manifest based on the current directory:

    cargo locate-project

== SEE ALSO
man:cargo[1], man:cargo-metadata[1]
