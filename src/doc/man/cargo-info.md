# cargo-info(1)

## NAME

cargo-info --- Display information about a package.

## SYNOPSIS

`cargo info` [_options_] _spec_

## DESCRIPTION

This command displays information about a package. It fetches data from the package's Cargo.toml file
and presents it in a human-readable format.

## OPTIONS

### Info Options

{{#options}}

{{#option "_spec_" }}

Fetch information about the specified package. The _spec_ can be a package ID, see {{man "cargo-pkgid" 1}} for the SPEC
format.
If the specified package is part of the current workspace, information from the local Cargo.toml file will be displayed.
If the `Cargo.lock` file does not exist, it will be created. If no version is specified, the appropriate version will be
selected based on the Minimum Supported Rust Version (MSRV).

{{/option}}
{{> options-index }}
{{> options-registry }}
{{/options}}

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

### Manifest Options

{{#options}}
{{> options-locked }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Inspect the `serde` package from crates.io:

        cargo info serde
2. Inspect the `serde` package with version `1.0.0`:

        cargo info serde@1.0.0
3. Inspect the `serde` package form the local registry:

        cargo info serde --registry my-registry

## SEE ALSO

{{man "cargo" 1}}, {{man "cargo-search" 1}}
