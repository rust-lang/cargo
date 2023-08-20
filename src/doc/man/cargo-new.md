# cargo-new(1)

## NAME

cargo-new --- Create a new Cargo package

## SYNOPSIS

`cargo new` [_options_] _path_

## DESCRIPTION

This command will create a new Cargo package in the given directory. This
includes a simple template with a `Cargo.toml` manifest, sample source file,
and a VCS ignore file. If the directory is not already in a VCS repository,
then a new repository is created (see `--vcs` below).

See {{man "cargo-init" 1}} for a similar command which will create a new manifest
in an existing directory.

## OPTIONS

### New Options

{{> options-new }}

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Create a binary Cargo package in the given directory:

       cargo new foo

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-init" 1}}
