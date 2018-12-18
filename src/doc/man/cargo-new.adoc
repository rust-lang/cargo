= cargo-new(1)
:idprefix: cargo_new_
:doctype: manpage

== NAME

cargo-new - Create a new Cargo package

== SYNOPSIS

`cargo new [_OPTIONS_] _PATH_`

== DESCRIPTION

This command will create a new Cargo package in the given directory. This
includes a simple template with a `Cargo.toml` manifest, sample source file,
and a VCS ignore file. If the directory is not already in a VCS repository,
then a new repository is created (see `--vcs` below).

include::description-new-authors.adoc[]

See man:cargo-init[1] for a similar command which will create a new manifest
in an existing directory.

== OPTIONS

=== New Options

include::options-new.adoc[]

=== Display Options

include::options-display.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. Create a binary Cargo package in the given directory:

    cargo new foo

== SEE ALSO
man:cargo[1], man:cargo-init[1]
