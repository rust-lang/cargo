# cargo-publish(1)
{{~*set command="publish"}}
{{~*set actionverb="Publish"}}
{{~*set multitarget=true}}

## NAME

cargo-publish --- Upload a package to the registry

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
3. Upload the crate to the registry. The server will perform additional
   checks on the crate. 
4. The client will poll waiting for the package to appear in the index,
   and may timeout. In that case, you will need to check for completion
   manually. This timeout does not affect the upload.

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

### Package Selection

By default, when no package selection options are given, the packages selected
depend on the selected manifest file (based on the current working directory if
`--manifest-path` is not given). If the manifest is the root of a workspace then
the workspaces default members are selected, otherwise only the package defined
by the manifest will be selected.

The default members of a workspace can be set explicitly with the
`workspace.default-members` key in the root manifest. If this is not set, a
virtual workspace will include all workspace members (equivalent to passing
`--workspace`), and a non-virtual workspace will include only the root crate itself.

Selecting more than one package is unstable and available only on the
[nightly channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html)
and requires the `-Z package-workspace` flag to enable.
See <https://github.com/rust-lang/cargo/issues/10948> for more information.


{{#options}}

{{#option "`-p` _spec_..." "`--package` _spec_..."}}
{{actionverb}} only the specified packages. See {{man "cargo-pkgid" 1}} for the
SPEC format. This flag may be specified multiple times and supports common Unix
glob patterns like `*`, `?` and `[]`. However, to avoid your shell accidentally 
expanding glob patterns before Cargo handles them, you must use single quotes or
double quotes around each pattern.
{{/option}}

Selecting more than one package with this option is unstable and available only
on the
[nightly channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html)
and requires the `-Z package-workspace` flag to enable.
See <https://github.com/rust-lang/cargo/issues/10948> for more information.

{{#option "`--workspace`" }}
{{actionverb}} all members in the workspace.

This option is unstable and available only on the
[nightly channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html)
and requires the `-Z package-workspace` flag to enable.
See <https://github.com/rust-lang/cargo/issues/10948> for more information.
{{/option}}

{{#option "`--exclude` _SPEC_..." }}
Exclude the specified packages. Must be used in conjunction with the
`--workspace` flag. This flag may be specified multiple times and supports
common Unix glob patterns like `*`, `?` and `[]`. However, to avoid your shell
accidentally expanding glob patterns before Cargo handles them, you must use
single quotes or double quotes around each pattern.

This option is unstable and available only on the
[nightly channel](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html)
and requires the `-Z package-workspace` flag to enable.
See <https://github.com/rust-lang/cargo/issues/10948> for more information.
{{/option}}

{{/options}}

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

{{> options-lockfile-path }}

{{/options}}

### Miscellaneous Options

{{#options}}
{{> options-jobs }}
{{> options-keep-going }}
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
