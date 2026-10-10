use std::path::Path;

use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn builtin_dep_accepted() {
    let p = project()
        .file("src/lib.rs", "#![no_std]")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                core = { builtin = true }
                "#,
        )
        .build();

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testsuite/mock-std/library");
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .env("__CARGO_TESTS_ONLY_SRC_ROOT", &root)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn builtin_feature_gate() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [dependencies]

                core = { builtin = true }
                "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  resolving builtin dependency core

Caused by:
  feature `builtin-dependencies` is required

  The package requires the Cargo feature called `builtin-dependencies`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#builtin-dependencies for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
fn resolve_contents() {
    crate::standard_lib::publish_mock_std_registry_packages();

    let p = project()
        .file("src/lib.rs", "#![no_std]")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                core = { builtin = true }
                "#,
        )
        .build();

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testsuite/mock-std/library");
    p.cargo("metadata")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .env("__CARGO_TESTS_ONLY_SRC_ROOT", &root)
        .with_stdout_data(
            str![[r#"
{
  "build_directory": "[ROOT]/foo/target",
  "metadata": null,
  "packages": [
    {
      "authors": [
        "Alex Crichton <alex@alexcrichton.com>"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2018",
      "features": {},
      "homepage": null,
      "id": "builtin://.#core",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[..]/tests/testsuite/mock-std/library/core/Cargo.toml",
      "metadata": null,
      "name": "core",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "builtin://.",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2018",
          "kind": [
            "lib"
          ],
          "name": "core",
          "src_path": "[..]/tests/testsuite/mock-std/library/core/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "core",
          "optional": false,
          "registry": null,
          "rename": null,
          "req": "*",
          "source": "builtin://.",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2021",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2021",
          "kind": [
            "lib"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.1.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "builtin://.#core"
      },
      {
        "dependencies": [
          "builtin://.#core"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "core",
            "pkg": "builtin://.#core"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn patching_with_builtins_requires_feature_gate() {
    let p = project()
        .file("src/lib.rs", "use core;")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [patch.crates-io]
                core.builtin = true
                "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  resolving patch for `core`

Caused by:
  feature `builtin-dependencies` is required

  The package requires the Cargo feature called `builtin-dependencies`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["builtin-dependencies"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#builtin-dependencies for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
fn patching_with_builtins_in_config_requires_feature_gate() {
    let p = project()
        .file("src/lib.rs", "use core;")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"
                "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [patch.crates-io]
                core.builtin = true
                "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid patch for `core`

Caused by:
  feature `builtin-dependencies` is required

  The package requires the Cargo feature called `builtin-dependencies`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["builtin-dependencies"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#builtin-dependencies for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
fn replacing_with_builtins_requires_feature_gate() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [replace]
                "core:0.0.0" = { builtin = true }
            "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  invalid replacement for `core:0.0.0`

Caused by:
  feature `builtin-dependencies` is required

  The package requires the Cargo feature called `builtin-dependencies`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["builtin-dependencies"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#builtin-dependencies for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
fn unused_workspace_builtin_dependency_requires_feature_gate() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [workspace.dependencies]
                core = { builtin = true }
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  resolving workspace dependency `core`

Caused by:
  feature `builtin-dependencies` is required

  The package requires the Cargo feature called `builtin-dependencies`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["builtin-dependencies"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#builtin-dependencies for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
fn builtin_false_rejected() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                core = { version = "0.0.0", builtin = false }
            "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `builtin` cannot be false
       
  --> Cargo.toml:10:24
   |
10 |                 core = { version = "0.0.0", builtin = false }
   |                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

"#]])
        .run();
}

#[cargo_test]
fn builtin_in_inherited_dependency_rejected() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                core = { workspace = true, builtin = true}
            "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `builtin` cannot be combined with `workspace = true`
  --> Cargo.toml:10:24
   |
10 |                 core = { workspace = true, builtin = true}
   |                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

"#]])
        .run();
}

#[cargo_test]
fn builtin_dependency_combined_with_sources() {
    let other_sources = [
        "git = \"https://example.com/custom/core.git\"",
        "path = \"my/custom/core\"",
        "registry = \"dummy-registry\"",
        "registry-index = \"https://www.example.com/index/\"",
    ];
    for source in other_sources.into_iter() {
        let p = project()
            .file("src/lib.rs", "")
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                    cargo-features = ["builtin-dependencies"]

                    [package]
                    name = "foo"
                    version = "0.1.0"
                    [dependencies]

                    core = {{ builtin = true, {} }}
                    "#,
                    source
                ),
            )
            .build();

        p.cargo("check")
            .masquerade_as_nightly_cargo(&["builtin-dependencies"])
            .with_status(101)
            .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dependency (core) specification is ambiguous. `builtin = true` cannot be combined with any other dependency source

"#]])
            .run();
    }
}

#[cargo_test]
fn builtin_combined_with_version_specifier() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                [dependencies]

                core = { builtin = true, version = "0.0.0" }
                "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  builtin dependency `core` cannot be combined with a version requirement
  Builtin dependencies are unversioned.

"#]])
        .run();
}

#[cargo_test]
fn build_dependencies() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [build-dependencies]
                core.builtin = true
                "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  builtin dependency `core` cannot be used as a build dependency

"#]])
        .run();
}

#[cargo_test]
fn patching_builtins_is_invalid() {
    let p = project()
        .file("src/lib.rs", "use core;")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [patch.builtin]
                core = { path = "core" }
                "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  [patch] entry `builtin` should be a URL or registry name

Caused by:
  invalid url `builtin`: relative URL without a base

"#]])
        .run();
}

#[cargo_test]
fn target_specific_build_dependencies() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [target.'cfg(all())'.build-dependencies]
                core = { builtin = true }
            "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  builtin dependency `core` cannot be used as a build dependency

"#]])
        .run();
}

#[cargo_test]
fn inherited_build_dependencies() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [workspace.dependencies]
                core = { builtin = true }

                [build-dependencies]
                core.workspace = true
            "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  builtin dependency `core` cannot be used as a build dependency

"#]])
        .run();
}
