use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn builtin_dep_accepted() {
    let p = project()
        .file("src/lib.rs", "use core;")
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

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
        .with_stderr_data(str![[r#"

thread [..] panicked at [..]
not yet implemented: SourceKind::Builtin
[NOTE] run with `RUST_BACKTRACE=1` environment variable to display a backtrace

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

thread [..] panicked at [..]
not yet implemented: SourceKind::Builtin
[NOTE] run with `RUST_BACKTRACE=1` environment variable to display a backtrace

"#]])
            .run();
    }
}
