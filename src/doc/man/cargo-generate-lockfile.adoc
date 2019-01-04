= cargo-generate-lockfile(1)
:idprefix: cargo_generate-lockfile_
:doctype: manpage

== NAME

cargo-generate-lockfile - Generate the lockfile for a package

== SYNOPSIS

`cargo generate-lockfile [_OPTIONS_]`

== DESCRIPTION

This command will create the `Cargo.lock` lockfile for the current package or
workspace. If the lockfile already exists, it will be rebuilt if there are any
manifest changes or dependency updates.

See also man:cargo-update[1] which is also capable of creating a `Cargo.lock`
lockfile and has more options for controlling update behavior.

== OPTIONS

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

. Create or update the lockfile for the current package or workspace:

    cargo generate-lockfile

== SEE ALSO
man:cargo[1], man:cargo-update[1]
