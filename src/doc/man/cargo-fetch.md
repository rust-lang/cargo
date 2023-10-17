# cargo-fetch(1)
{{~*set command="fetch"}}
{{~*set actionverb="Fetch"}}
{{~*set target-default-to-all-arch=true}}
{{~*set multitarget=true}}

## NAME

cargo-fetch --- Fetch dependencies of a package from the network

## SYNOPSIS

`cargo fetch` [_options_]

## DESCRIPTION

If a `Cargo.lock` file is available, this command will ensure that all of the
git dependencies and/or registry dependencies are downloaded and locally
available. Subsequent Cargo commands will be able to run offline after a `cargo
fetch` unless the lock file changes.

If the lock file is not available, then this command will generate the lock
file before fetching the dependencies.

If `--target` is not specified, then all target dependencies are fetched.

See also the [cargo-prefetch](https://crates.io/crates/cargo-prefetch)
plugin which adds a command to download popular crates. This may be useful if
you plan to use Cargo without a network with the `--offline` flag.

## OPTIONS

### Fetch options

{{#options}}
{{> options-target-triple }}
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

1. Fetch all dependencies:

       cargo fetch

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-update" 1}}, {{man "cargo-generate-lockfile" 1}}
