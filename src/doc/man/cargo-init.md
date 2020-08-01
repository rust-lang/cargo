= cargo-init(1)
:idprefix: cargo_init_
:doctype: manpage

== NAME

cargo-init - Create a new Cargo package in an existing directory

== SYNOPSIS

`cargo init [_OPTIONS_] [_PATH_]`

== DESCRIPTION

This command will create a new Cargo manifest in the current directory. Give a
path as an argument to create in the given directory.

If there are typically-named Rust source files already in the directory, those
will be used. If not, then a sample `src/main.rs` file will be created, or
`src/lib.rs` if `--lib` is passed.

If the directory is not already in a VCS repository, then a new repository
is created (see `--vcs` below).

include::description-new-authors.adoc[]

See man:cargo-new[1] for a similar command which will create a new package in
a new directory.

== OPTIONS

=== Init Options

include::options-new.adoc[]

=== Display Options

include::options-display.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. Create a binary Cargo package in the current directory:

    cargo init

== SEE ALSO
man:cargo[1], man:cargo-new[1]
