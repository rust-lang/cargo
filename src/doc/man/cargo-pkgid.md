= cargo-pkgid(1)
:idprefix: cargo_pkgid_
:doctype: manpage

== NAME

cargo-pkgid - Print a fully qualified package specification

== SYNOPSIS

`cargo pkgid [_OPTIONS_] [_SPEC_]`

== DESCRIPTION

Given a _SPEC_ argument, print out the fully qualified package ID specifier
for a package or dependency in the current workspace. This command will
generate an error if _SPEC_ is ambiguous as to which package it refers to in
the dependency graph. If no _SPEC_ is given, then the specifier for the local
package is printed.

This command requires that a lockfile is available and dependencies have been
fetched.

A package specifier consists of a name, version, and source URL. You are
allowed to use partial specifiers to succinctly match a specific package as
long as it matches only one package. The format of a _SPEC_ can be one of the
following:

[%autowidth]
.SPEC Query Format
|===
|SPEC Structure |Example SPEC

|__NAME__
|`bitflags`

|__NAME__``:``__VERSION__
|`bitflags:1.0.4`

|__URL__
|`https://github.com/rust-lang/cargo`

|__URL__``#``__VERSION__
|`https://github.com/rust-lang/cargo#0.33.0`

|__URL__``#``__NAME__
|`https://github.com/rust-lang/crates.io-index#bitflags`

|__URL__``#``__NAME__``:``__VERSION__
|`https://github.com/rust-lang/cargo#crates-io:0.21.0`
|===

== OPTIONS

=== Package Selection

*-p* _SPEC_::
*--package* _SPEC_::
    Get the package ID for the given package instead of the current package.

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

. Retrieve package specification for `foo` package:

    cargo pkgid foo

. Retrieve package specification for version 1.0.0 of `foo`:

    cargo pkgid foo:1.0.0

. Retrieve package specification for `foo` from crates.io:

    cargo pkgid https://github.com/rust-lang/crates.io-index#foo

== SEE ALSO
man:cargo[1], man:cargo-generate-lockfile[1], man:cargo-metadata[1]
