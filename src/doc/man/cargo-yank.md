= cargo-yank(1)
:idprefix: cargo_yank_
:doctype: manpage

== NAME

cargo-yank - Remove a pushed crate from the index

== SYNOPSIS

`cargo yank [_OPTIONS_] --vers _VERSION_ [_CRATE_]`

== DESCRIPTION

The yank command removes a previously published crate's version from the
server's index. This command does not delete any data, and the crate will
still be available for download via the registry's download link.

Note that existing crates locked to a yanked version will still be able to
download the yanked version to use it. Cargo will, however, not allow any new
crates to be locked to any yanked version.

This command requires you to be authenticated with either the `--token` option
or using man:cargo-login[1].

If the crate name is not specified, it will use the package name from the
current directory.

== OPTIONS

=== Yank Options

*--vers* _VERSION_::
    The version to yank or un-yank.

*--undo*::
    Undo a yank, putting a version back into the index.

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

. Yank a crate from the index:

    cargo yank --vers 1.0.7 foo

== SEE ALSO
man:cargo[1], man:cargo-login[1], man:cargo-publish[1]
