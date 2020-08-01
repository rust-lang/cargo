*--bin*::
    Create a package with a binary target (`src/main.rs`).
    This is the default behavior.

*--lib*::
    Create a package with a library target (`src/lib.rs`).

*--edition* _EDITION_::
    Specify the Rust edition to use. Default is 2018.
    Possible values: 2015, 2018

*--name* _NAME_::
    Set the package name. Defaults to the directory name.

*--vcs* _VCS_::
    Initialize a new VCS repository for the given version control system (git,
    hg, pijul, or fossil) or do not initialize any version control at all
    (none). If not specified, defaults to `git` or the configuration value
    `cargo-new.vcs`, or `none` if already inside a VCS repository.

*--registry* _REGISTRY_::
    This sets the `publish` field in `Cargo.toml` to the given registry name
    which will restrict publishing only to that registry.
+
Registry names are defined in linkcargo:reference/config.html[Cargo config files].
If not specified, the default registry defined by the `registry.default`
config key is used. If the default registry is not set and `--registry` is not
used, the `publish` field will not be set which means that publishing will not
be restricted.
