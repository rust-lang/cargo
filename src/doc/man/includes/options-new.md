{{#options}}

{{#option "`--bin`" }}
Create a package with a binary target (`src/main.rs`).
This is the default behavior.
{{/option}}

{{#option "`--lib`" }}
Create a package with a library target (`src/lib.rs`).
{{/option}}

{{#option "`--edition` _edition_" }}
Specify the Rust edition to use. Default is 2021.
Possible values: 2015, 2018, 2021, 2024
{{/option}}

{{#option "`--name` _name_" }}
Set the package name. Defaults to the directory name.
{{/option}}

{{#option "`--vcs` _vcs_" }}
Initialize a new VCS repository for the given version control system (git,
hg, pijul, or fossil) or do not initialize any version control at all
(none). If not specified, defaults to `git` or the configuration value
`cargo-new.vcs`, or `none` if already inside a VCS repository.
{{/option}}

{{#option "`--registry` _registry_" }}
This sets the `publish` field in `Cargo.toml` to the given registry name
which will restrict publishing only to that registry.

Registry names are defined in [Cargo config files](../reference/config.html).
If not specified, the default registry defined by the `registry.default`
config key is used. If the default registry is not set and `--registry` is not
used, the `publish` field will not be set which means that publishing will not
be restricted.
{{/option}}

{{/options}}
