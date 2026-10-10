//! Tests for shell completion.

use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn complete_features_duplicate_in_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crate_a", "crate_b"]
            "#,
        )
        .file(
            "crate_a/Cargo.toml",
            r#"
                [package]
                name = "crate_a"
                version = "0.1.0"
                edition = "2021"

                [features]
                shared_feature = []
                feature_a = []
            "#,
        )
        .file("crate_a/src/lib.rs", "")
        .file(
            "crate_b/Cargo.toml",
            r#"
                [package]
                name = "crate_b"
                version = "0.1.0"
                edition = "2021"

                [features]
                shared_feature = []
                feature_b = []
            "#,
        )
        .file("crate_b/src/lib.rs", "")
        .build();

    p.cargo("-- cargo check --features")
        .arg("")
        .masquerade_as_nightly_cargo(&["completions"])
        .env("CARGO_COMPLETE", "fish")
        .with_stdout_data(str![[r#"
feature_a	from crate_a
shared_feature	from crate_a, crate_b
feature_b	from crate_b

"#]])
        .run();
}

#[cargo_test]
fn complete_features_duplicate_in_workspace_from_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crate_a", "crate_b"]
            "#,
        )
        .file(
            "crate_a/Cargo.toml",
            r#"
                [package]
                name = "crate_a"
                version = "0.1.0"
                edition = "2021"

                [features]
                shared_feature = []
                feature_a = []
            "#,
        )
        .file("crate_a/src/lib.rs", "")
        .file(
            "crate_b/Cargo.toml",
            r#"
                [package]
                name = "crate_b"
                version = "0.1.0"
                edition = "2021"

                [features]
                shared_feature = []
                feature_b = []
            "#,
        )
        .file("crate_b/src/lib.rs", "")
        .build();

    p.cargo("-- cargo check --features")
        .arg("")
        .cwd("crate_b")
        .masquerade_as_nightly_cargo(&["completions"])
        .env("CARGO_COMPLETE", "fish")
        .with_stdout_data(str![[r#"
feature_a	from crate_a
shared_feature	from crate_a, crate_b
feature_b	from crate_b

"#]])
        .run();
}

#[cargo_test]
fn complete_features_multiple_members_same_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crate_a", "crate_b", "crate_c"]
            "#,
        )
        .file(
            "crate_a/Cargo.toml",
            r#"
                [package]
                name = "crate_a"
                version = "0.1.0"
                edition = "2021"

                [features]
                shared_feature = []
            "#,
        )
        .file("crate_a/src/lib.rs", "")
        .file(
            "crate_b/Cargo.toml",
            r#"
                [package]
                name = "crate_b"
                version = "0.1.0"
                edition = "2021"

                [features]
                shared_feature = []
            "#,
        )
        .file("crate_b/src/lib.rs", "")
        .file(
            "crate_c/Cargo.toml",
            r#"
                [package]
                name = "crate_c"
                version = "0.1.0"
                edition = "2021"

                [features]
                shared_feature = []
            "#,
        )
        .file("crate_c/src/lib.rs", "")
        .build();

    p.cargo("-- cargo check --features")
        .arg("")
        .masquerade_as_nightly_cargo(&["completions"])
        .env("CARGO_COMPLETE", "fish")
        .with_stdout_data(str![[r#"
shared_feature	from crate_a, crate_b, crate_c

"#]])
        .run();
}
