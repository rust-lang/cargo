# cargo-package(1)
{{~*set command="package"}}
{{~*set actionverb="Package"}}
{{~*set noall=true}}
{{~*set multitarget=true}}

## NAME

cargo-package --- Assemble the local package into a distributable tarball

## SYNOPSIS

`cargo package` [_options_]

## DESCRIPTION

This command will create a distributable, compressed `.crate` file with the
source code of the package in the current directory. The resulting file will
be stored in the `target/package` directory. This performs the following
steps:

1. Load and check the current workspace, performing some basic checks.
    - Path dependencies are not allowed unless they have a version key. Cargo
      will ignore the path key for dependencies in published packages.
      `dev-dependencies` do not have this restriction.
2. Create the compressed `.crate` file.
    - The original `Cargo.toml` file is rewritten and normalized.
    - `[patch]`, `[replace]`, and `[workspace]` sections are removed from the
      manifest.
    - `Cargo.lock` is automatically included if the package contains an
      executable binary or example target. {{man "cargo-install" 1}} will use the
      packaged lock file if the `--locked` flag is used.
    - A `.cargo_vcs_info.json` file is included that contains information
      about the current VCS checkout hash if available (not included with
      `--allow-dirty`).
3. Extract the `.crate` file and build it to verify it can build.
    - This will rebuild your package from scratch to ensure that it can be
      built from a pristine state. The `--no-verify` flag can be used to skip
      this step.
4. Check that build scripts did not modify any source files.

The list of files included can be controlled with the `include` and `exclude`
fields in the manifest.

See [the reference](../reference/publishing.html) for more details about
packaging and publishing.

### .cargo_vcs_info.json format

Will generate a `.cargo_vcs_info.json` in the following format

```javascript
{
 "git": {
   "sha1": "aac20b6e7e543e6dd4118b246c77225e3a3a1302"
 },
 "path_in_vcs": ""
}
```

`path_in_vcs` will be set to a repo-relative path for packages
in subdirectories of the version control repository.

## OPTIONS

### Package Options

{{#options}}

{{#option "`-l`" "`--list`" }}
Print files included in a package without making one.
{{/option}}

{{#option "`--no-verify`" }}
Don't verify the contents by building them.
{{/option}}

{{#option "`--no-metadata`" }}
Ignore warnings about a lack of human-usable metadata (such as the description
or the license).
{{/option}}

{{#option "`--allow-dirty`" }}
Allow working directories with uncommitted VCS changes to be packaged.
{{/option}}

{{/options}}

{{> section-package-selection }}

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

1. Create a compressed `.crate` file of the current package:

       cargo package

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-publish" 1}}
