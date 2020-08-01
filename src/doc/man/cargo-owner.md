= cargo-owner(1)
:idprefix: cargo_owner_
:doctype: manpage

== NAME

cargo-owner - Manage the owners of a crate on the registry

== SYNOPSIS

[%hardbreaks]
`cargo owner [_OPTIONS_] --add _LOGIN_ [_CRATE_]`
`cargo owner [_OPTIONS_] --remove _LOGIN_ [_CRATE_]`
`cargo owner [_OPTIONS_] --list [_CRATE_]`

== DESCRIPTION

This command will modify the owners for a crate on the registry. Owners of a
crate can upload new versions and yank old versions. Non-team owners can also
modify the set of owners, so take care!

This command requires you to be authenticated with either the `--token` option
or using man:cargo-login[1].

If the crate name is not specified, it will use the package name from the
current directory.

See linkcargo:reference/publishing.html#cargo-owner[the reference] for more
information about owners and publishing.

== OPTIONS

=== Owner Options

*-a*::
*--add* _LOGIN_...::
    Invite the given user or team as an owner.

*-r*::
*--remove* _LOGIN_...::
    Remove the given user or team as an owner.

*-l*::
*--list*::
    List owners of a crate.

include::options-token.adoc[]

include::options-index.adoc[]

include::options-registry.adoc[]

=== Display Options

include::options-display.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. List owners of a package:

    cargo owner --list foo

. Invite an owner to a package:

    cargo owner --add username foo

. Remove an owner from a package:

    cargo owner --remove username foo

== SEE ALSO
man:cargo[1], man:cargo-login[1], man:cargo-publish[1]
