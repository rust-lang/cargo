# cargo-search(1)

## NAME

cargo-search --- Search packages in crates.io

## SYNOPSIS

`cargo search` [_options_] [_query_...]

## DESCRIPTION

This performs a textual search for crates on <https://crates.io>. The matching
crates will be displayed along with their description in TOML format suitable
for copying into a `Cargo.toml` manifest.

## OPTIONS

### Search Options

{{#options}}

{{#option "`--limit` _limit_" }}
Limit the number of results (default: 10, max: 100).
{{/option}}

{{> options-index }}

{{> options-registry }}

{{/options}}

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Search for a package from crates.io:

       cargo search serde

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-install" 1}}, {{man "cargo-publish" 1}}
