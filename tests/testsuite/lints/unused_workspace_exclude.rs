use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn unused_exclude_missing_directory() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crates/*"]
                exclude = ["crates/does-not-exist"]

                [workspace.lints.cargo]
                unused_workspace_exclude = "warn"
            "#,
        )
        .file(
            "crates/foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lints]
                workspace = true
            "#,
        )
        .file("crates/foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unused exclude pattern 'crates/does-not-exist'
 --> Cargo.toml:4:17
  |
4 |                 exclude = ["crates/does-not-exist"]
  |                 ^^^^^^^
  |
  = [NOTE] `cargo::unused_workspace_exclude` is set to `warn` in `[lints]`
[WARNING] workspace (manifest) generated 1 warning
[CHECKING] foo v0.0.1 ([ROOT]/foo/crates/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_exclude_directory_without_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crates/*"]
                exclude = ["crates/bar"]

                [workspace.lints.cargo]
                unused_workspace_exclude = "warn"
            "#,
        )
        .file(
            "crates/foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lints]
                workspace = true
            "#,
        )
        .file("crates/foo/src/lib.rs", "")
        .file("crates/bar/some_file.txt", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo/crates/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_exclude_glob_matches_nothing() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crates/*"]
                exclude = ["crates/not-*"]

                [workspace.lints.cargo]
                unused_workspace_exclude = "warn"
            "#,
        )
        .file(
            "crates/foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lints]
                workspace = true
            "#,
        )
        .file("crates/foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unused exclude pattern 'crates/not-*'
 --> Cargo.toml:4:17
  |
4 |                 exclude = ["crates/not-*"]
  |                 ^^^^^^^
  |
  = [NOTE] `cargo::unused_workspace_exclude` is set to `warn` in `[lints]`
[WARNING] workspace (manifest) generated 1 warning
[CHECKING] foo v0.0.1 ([ROOT]/foo/crates/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_exclude_valid_no_warning() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crates/*"]
                exclude = ["crates/bar"]

                [workspace.lints.cargo]
                unused_workspace_exclude = "warn"
            "#,
        )
        .file(
            "crates/foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lints]
                workspace = true
            "#,
        )
        .file("crates/foo/src/lib.rs", "")
        .file(
            "crates/bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"

                [lints]
                workspace = true
            "#,
        )
        .file("crates/bar/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo/crates/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
