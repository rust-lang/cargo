# cargo-yank(1)

## NAME

cargo-yank --- Remove a pushed crate from the index

## SYNOPSIS

`cargo yank` [_options_] _crate_@_version_\
`cargo yank` [_options_] `--version` _version_ [_crate_]

## DESCRIPTION

The yank command removes a previously published crate's version from the
server's index. This command does not delete any data, and the crate will
still be available for download via the registry's download link.

Cargo will not use a yanked version for any new project or checkout without a
pre-existing lockfile, and will generate an error if there are no longer
any compatible versions for your crate.

This command requires you to be authenticated with either the `--token` option
or using {{man "cargo-login" 1}}.

If the crate name is not specified, it will use the package name from the
current directory.

### How yank works

For example, the `foo` crate published version `1.5.0` and another crate `bar`
declared a dependency on version `foo = "1.5"`. Now `foo` releases a new, but
not semver compatible, version `2.0.0`, and finds a critical issue with `1.5.0`.
If `1.5.0` is yanked, no new project or checkout without an existing lockfile
will be able to use crate `bar` as it relies on `1.5`.

In this case, the maintainers of `foo` should first publish a semver compatible
version such as `1.5.1` prior to yanking `1.5.0` so that `bar` and all projects
that depend on `bar` will continue to work.

As another example, consider a crate `bar` with published versions `1.5.0`,
`1.5.1`, `1.5.2`, `2.0.0` and `3.0.0`. The following table identifies the
versions cargo could use in the absence of a lockfile for different SemVer
requirements, following a given release being yanked:

| Yanked Version / SemVer requirement | `bar = "1.5.0"`                         | `bar = "=1.5.0"` | `bar = "2.0.0"`  |
|-------------------------------------|-----------------------------------------|------------------|------------------|
| `1.5.0`                             | Use either `1.5.1` or `1.5.2`           | **Return Error** | Use `2.0.0`      |
| `1.5.1`                             | Use either `1.5.0` or `1.5.2`           | Use `1.5.0`      | Use `2.0.0`      |
| `2.0.0`                             | Use either `1.5.0`, `1.5.1` or `1.5.2`  | Use `1.5.0`      | **Return Error** |

### When to yank

Crates should only be yanked in exceptional circumstances, for example, an
accidental publish, an unintentional SemVer breakages, or a significantly
broken and unusable crate. In the case of security vulnerabilities, [RustSec]
is typically a less disruptive mechanism to inform users and encourage them
to upgrade, and avoids the possibility of significant downstream disruption
irrespective of susceptibility to the vulnerability in question.

A common workflow is to yank a crate having already published a semver
compatible version, to reduce the probability of preventing dependent
crates from compiling.

When addressing copyright, licensing, or personal data issues with a published
crate, simply yanking it may not suffice. In such cases, contact the maintainers
of the registry you used. For crates.io, refer to their [policies] and contact
them at <help@crates.io>.

If credentials have been leaked, the recommended course of action is to revoke
them immediately. Once a crate has been published, it is impossible to determine
if the leaked credentials have been copied. Yanking the crate only prevents new
users from downloading it, but cannot stop those who have already downloaded it
from keeping or even spreading the leaked credentials.

[RustSec]: https://rustsec.org/
[policies]: https://crates.io/policies

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
