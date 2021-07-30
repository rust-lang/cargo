# cargo-publish(1)
{{*set actionverb="Publish"}}

## NAME

cargo-publish - Upload a package to the registry

## SYNOPSIS

`cargo publish` [_options_]

## DESCRIPTION

This command will create a distributable, compressed `.crate` file with the
source code of the package in the current directory and upload it to a
registry. The default registry is <https://crates.io>. This performs the
following steps:

1. Performs a few checks, including:
   - Checks the `package.publish` key in the manifest for restrictions on
     which registries you are allowed to publish to.
2. Create a `.crate` file by following the steps in {{man "cargo-package" 1}}.
3. Upload the crate to the registry. Note that the server will perform
   additional checks on the crate.

This command requires you to be authenticated with either the `--token` option
or using {{man "cargo-login" 1}}.

See [the reference](../reference/publishing.html) for more details about
packaging and publishing.

## OPTIONS

### Publish Options

{{#options}}

{{#option "`--dry-run`" }}
Perform all checks without uploading.
{{/option}}

{{> options-token }}

{{#option "`--no-verify`" }}
Don't verify the contents by building them.
{{/option}}

{{#option "`--allow-dirty`" }}
Allow working directories with uncommitted VCS changes to be packaged.
{{/option}}

{{> options-index }}

{{#option "`--registry` _registry_"}}
Name of the registry to publish to. Registry names are defined in [Cargo
config files](../reference/config.html). If not specified, and there is a
[`package.publish`](../reference/manifest.html#the-publish-field) field in
`Cargo.toml` with a single registry, then it will publish to that registry.
Otherwise it will use the default registry, which is defined by the
[`registry.default`](../reference/config.html#registrydefault) config key
which defaults to `crates-io`.
{{/option}}

{{/options}}

{{> section-options-package }}

### Compilation Options

{{#options}}

{{> options-target-triple }}

{{> options-target-dir }}

{{/options}}

{{> section-features }}

### Manifest Options

{{#options}}

{{> options-manifest-path }}

{{> options-locked }}

{{/options}}

### Miscellaneous Options

{{#options}}
{{> options-jobs }}
{{/options}}

### Display Options

{{#options}}
{{> options-display }}
{{/options}}

{{> section-options-common }}

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Publish the current package:

       cargo publish

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-package" 1}}, {{man "cargo-login" 1}}
