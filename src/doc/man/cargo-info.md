# cargo-info(1)

## NAME

cargo-info --- Display information about a package in the registry. Default registry is crates.io

## SYNOPSIS

`cargo info` [_options_] _crate_@_version_

## DESCRIPTION

This command displays information about a package in the registry. It fetches data from the package's Cargo.toml file
and presents it in a human-readable format.

_crate_`@`_version_: Fetch from a registry with a version constraint of "_version_"

If the specified package is part of the current workspace, the information from the local Cargo.toml file will be
displayed, and if no version is specified, it will select the appropriate version based on the Minimum Supported Rust
Version (MSRV).

## OPTIONS

### Info Options

{{#options}}
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
