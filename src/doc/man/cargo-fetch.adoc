= cargo-fetch(1)
:idprefix: cargo_fetch_
:doctype: manpage
:actionverb: Fetch

== NAME

cargo-fetch - Fetch dependencies of a package from the network

== SYNOPSIS

`cargo fetch [_OPTIONS_]`

== DESCRIPTION

If a `Cargo.lock` file is available, this command will ensure that all of the
git dependencies and/or registry dependencies are downloaded and locally
available. Subsequent Cargo commands never touch the network after a `cargo
fetch` unless the lock file changes.

If the lock file is not available, then this command will generate the lock
file before fetching the dependencies.

If `--target` is not specified, then all target dependencies are fetched.

See also the link:https://crates.io/crates/cargo-prefetch[cargo-prefetch]
plugin which adds a command to download popular crates. This may be useful if
you plan to use Cargo without a network with the `--offline` flag.

== OPTIONS

=== Fetch options

include::options-target-triple.adoc[]

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

. Fetch all dependencies:

    cargo fetch

== SEE ALSO
man:cargo[1], man:cargo-update[1], man:cargo-generate-lockfile[1]
