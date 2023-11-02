# cargo-pkgid(1)

## NAME

cargo-pkgid --- Print a fully qualified package specification

## SYNOPSIS

`cargo pkgid` [_options_] [_spec_]

## DESCRIPTION

Given a _spec_ argument, print out the fully qualified package ID specifier
for a package or dependency in the current workspace. This command will
generate an error if _spec_ is ambiguous as to which package it refers to in
the dependency graph. If no _spec_ is given, then the specifier for the local
package is printed.

This command requires that a lockfile is available and dependencies have been
fetched.

A package specifier consists of a name, version, and source URL. You are
allowed to use partial specifiers to succinctly match a specific package as
long as it matches only one package. The format of a _spec_ can be one of the
following:

SPEC Structure             | Example SPEC
---------------------------|--------------
_name_                     | `bitflags`
_name_`@`_version_         | `bitflags@1.0.4`
_url_                      | `https://github.com/rust-lang/cargo`
_url_`#`_version_          | `https://github.com/rust-lang/cargo#0.33.0`
_url_`#`_name_             | `https://github.com/rust-lang/crates.io-index#bitflags`
_url_`#`_name_`@`_version_ | `https://github.com/rust-lang/cargo#crates-io@0.21.0`

## OPTIONS

### Package Selection

{{#options}}

{{#option "`-p` _spec_" "`--package` _spec_" }}
Get the package ID for the given package instead of the current package.
{{/option}}

{{/options}}

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

### Manifest Options

{{#options}}

{{> options-manifest-path }}

{{> options-locked }}

{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Retrieve package specification for `foo` package:

       cargo pkgid foo

2. Retrieve package specification for version 1.0.0 of `foo`:

       cargo pkgid foo@1.0.0

3. Retrieve package specification for `foo` from crates.io:

       cargo pkgid https://github.com/rust-lang/crates.io-index#foo

4. Retrieve package specification for `foo` from a local package:

       cargo pkgid file:///path/to/local/package#foo

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-generate-lockfile" 1}}, {{man "cargo-metadata" 1}}
