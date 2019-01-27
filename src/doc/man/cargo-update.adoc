= cargo-update(1)
:idprefix: cargo_update_
:doctype: manpage

== NAME

cargo-update - Update dependencies as recorded in the local lock file

== SYNOPSIS

`cargo update [_OPTIONS_]`

== DESCRIPTION

This command will update dependencies in the `Cargo.lock` file to the latest
version. It requires that the `Cargo.lock` file already exists as generated
by commands such as man:cargo-build[1] or man:cargo-generate-lockfile[1].

== OPTIONS

=== Update Options

*-p* _SPEC_...::
*--package* _SPEC_...::
    Update only the specified packages. This flag may be specified
    multiple times. See man:cargo-pkgid[1] for the SPEC format.
+
If packages are specified with the `-p` flag, then a conservative update of
the lockfile will be performed. This means that only the dependency specified
by SPEC will be updated. Its transitive dependencies will be updated only if
SPEC cannot be updated without updating dependencies.  All other dependencies
will remain locked at their currently recorded versions.
+
If `-p` is not specified, all dependencies are updated.

*--aggressive*::
    When used with `-p`, dependencies of _SPEC_ are forced to update as well.
    Cannot be used with `--precise`.

*--precise* _PRECISE_::
    When used with `-p`, allows you to specify a specific version number to
    set the package to. If the package comes from a git repository, this can
    be a git revision (such as a SHA hash or tag).

*--dry-run*::
    Displays what would be updated, but doesn't actually write the lockfile.

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

. Update all dependencies in the lockfile:

    cargo update

. Update only specific dependencies:

    cargo update -p foo -p bar

. Set a specific dependency to a specific version:

    cargo update -p foo --precise 1.2.3

== SEE ALSO
man:cargo[1], man:cargo-generate-lockfile[1]
