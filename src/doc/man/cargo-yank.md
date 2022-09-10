# cargo-yank(1)

## NAME

cargo-yank - Remove a pushed crate from the index

## SYNOPSIS

`cargo yank` [_options_] _crate_@_version_\
`cargo yank` [_options_] `--version` _version_ [_crate_]

## DESCRIPTION

The yank command removes a previously published crate's version from the
server's index. This command does not delete any data, and the crate will
still be available for download via the registry's download link.

However, yanking a release will prevent cargo from selecting that version
when determining the version of a dependency to use. If there are no longer
any compatible versions that haven't been yanked, cargo will return an error.

The only exception to this is crates locked to a specific version by a lockfile,
these will still be able to download the yanked version to use it.

For example, consider a crate `bar` with published versions `0.22.0`, `0.22.1`, 
`0.22.2`, `0.23.0` and `0.24.0`. The following table identifies the versions
cargo could use in the absence of a lockfile for different semver constraints,
following a given release being yanked 

| Yanked Version / Semver Constraint | `bar = "0.22.0"`                          | `bar = "=0.22.0"` | `bar = "0.23.0"` |
|------------------------------------|-------------------------------------------|-------------------|------------------|
| `0.22.0`                           | Use either `0.22.1` or `0.22.2`           | **Return Error**  | Use `0.23.0`     |
| `0.22.1`                           | Use either `0.22.0` or `0.22.2`           | Use `0.22.0`      | Use `0.23.0`     |
| `0.23.0`                           | Use either `0.22.0`, `0.21.0` or `0.22.2` | Use `0.22.0`      | **Return Error** |

A common workflow is to yank a crate having already published a semver compatible version,
to reduce the probability of preventing dependent crates from compiling

This command requires you to be authenticated with either the `--token` option
or using {{man "cargo-login" 1}}.

If the crate name is not specified, it will use the package name from the
current directory.

## OPTIONS

### Yank Options

{{#options}}

{{#option "`--vers` _version_" "`--version` _version_" }}
The version to yank or un-yank.
{{/option}}

{{#option "`--undo`" }}
Undo a yank, putting a version back into the index.
{{/option}}

{{> options-token }}

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

1. Yank a crate from the index:

       cargo yank foo@1.0.7

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-login" 1}}, {{man "cargo-publish" 1}}
