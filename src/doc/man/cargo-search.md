= cargo-search(1)
:idprefix: cargo_search_
:doctype: manpage

== NAME

cargo-search - Search packages in crates.io

== SYNOPSIS

`cargo search [_OPTIONS_] [_QUERY_...]`

== DESCRIPTION

This performs a textual search for crates on https://crates.io. The matching
crates will be displayed along with their description in TOML format suitable
for copying into a `Cargo.toml` manifest.

== OPTIONS

=== Search Options

*--limit* _LIMIT_::
    Limit the number of results (default: 10, max: 100).

include::options-index.adoc[]

include::options-registry.adoc[]

=== Display Options

include::options-display.adoc[]

=== Common Options

include::options-common.adoc[]

include::section-environment.adoc[]

include::section-exit-status.adoc[]

== EXAMPLES

. Search for a package from crates.io:

    cargo search serde

== SEE ALSO
man:cargo[1], man:cargo-install[1], man:cargo-publish[1]
