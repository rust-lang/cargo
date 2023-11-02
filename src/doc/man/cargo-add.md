# cargo-add(1)
{{~*set command="add"}}
{{~*set actionverb="Add"}}
{{~*set nouns="adds"}}

## NAME

cargo-add --- Add dependencies to a Cargo.toml manifest file

## SYNOPSIS

`cargo add` [_options_] _crate_...\
`cargo add` [_options_] `--path` _path_\
`cargo add` [_options_] `--git` _url_ [_crate_...]\


## DESCRIPTION

This command can add or modify dependencies.

The source for the dependency can be specified with:

* _crate_`@`_version_: Fetch from a registry with a version constraint of "_version_"
* `--path` _path_: Fetch from the specified _path_
* `--git` _url_: Pull from a git repo at _url_

If no source is specified, then a best effort will be made to select one, including:

* Existing dependencies in other tables (like `dev-dependencies`)
* Workspace members
* Latest release in the registry

When you add a package that is already present, the existing entry will be updated with the flags specified.

Upon successful invocation, the enabled (`+`) and disabled (`-`) [features] of the specified
dependency will be listed in the command's output.

[features]: ../reference/features.md

## OPTIONS

### Source options

{{#options}}

{{#option "`--git` _url_" }}
[Git URL to add the specified crate from](../reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories).
{{/option}}

{{#option "`--branch` _branch_" }}
Branch to use when adding from git.
{{/option}}

{{#option "`--tag` _tag_" }}
Tag to use when adding from git.
{{/option}}

{{#option "`--rev` _sha_" }}
Specific commit to use when adding from git.
{{/option}}

{{#option "`--path` _path_" }}
[Filesystem path](../reference/specifying-dependencies.html#specifying-path-dependencies) to local crate to add.
{{/option}}

{{> options-registry }}

{{/options}}

### Section options

{{#options}}

{{#option "`--dev`" }}
Add as a [development dependency](../reference/specifying-dependencies.html#development-dependencies).
{{/option}}

{{#option "`--build`" }}
Add as a [build dependency](../reference/specifying-dependencies.html#build-dependencies).
{{/option}}

{{#option "`--target` _target_" }}
Add as a dependency to the [given target platform](../reference/specifying-dependencies.html#platform-specific-dependencies).

To avoid unexpected shell expansions, you may use quotes around each target, e.g., `--target 'cfg(unix)'`.
{{/option}}

{{/options}}

### Dependency options

{{#options}}

{{#option "`--dry-run`" }}
Don't actually write the manifest
{{/option}}

{{#option "`--rename` _name_" }}
[Rename](../reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml) the dependency.
{{/option}}

{{#option "`--optional`" }}
Mark the dependency as [optional](../reference/features.html#optional-dependencies).
{{/option}}

{{#option "`--no-optional`" }}
Mark the dependency as [required](../reference/features.html#optional-dependencies).
{{/option}}

{{#option "`--no-default-features`" }}
Disable the [default features](../reference/features.html#dependency-features).
{{/option}}

{{#option "`--default-features`" }}
Re-enable the [default features](../reference/features.html#dependency-features).
{{/option}}

{{#option "`-F` _features_" "`--features` _features_" }}
Space or comma separated list of [features to
activate](../reference/features.html#dependency-features). When adding multiple
crates, the features for a specific crate may be enabled with
`package-name/feature-name` syntax. This flag may be specified multiple times,
which enables all specified features.
{{/option}}

{{#option "`--ignore-rust-version`" }}
Ignore `rust-version` specification in packages.

This option is unstable and available only on the
[nightly channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html)
and requires the `-Z unstable-options` flag to enable.
See <https://github.com/rust-lang/cargo/issues/5579> for more information.
{{/option}}

{{/options}}


### Display Options

{{#options}}
{{> options-display }}
{{/options}}

### Manifest Options

{{#options}}
{{> options-manifest-path }}

{{#option "`-p` _spec_" "`--package` _spec_" }}
Add dependencies to only the specified package.
{{/option}}

{{> options-locked }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Add `regex` as a dependency

       cargo add regex

2. Add `trybuild` as a dev-dependency

       cargo add --dev trybuild

3. Add an older version of `nom` as a dependency

       cargo add nom@5

4. Add support for serializing data structures to json with `derive`s

       cargo add serde serde_json -F serde/derive

5. Add `windows` as a platform specific dependency on `cfg(windows)`

       cargo add windows --target 'cfg(windows)'

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-remove" 1}}
