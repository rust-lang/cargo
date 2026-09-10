//! Tests for shell completion candidates.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

fn workspace_with_shared_feature() -> cargo_test_support::Project {
    project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crate-a", "crate-b"]
            "#,
        )
        .file(
            "crate-a/Cargo.toml",
            r#"
                [package]
                name = "crate-a"
                version = "0.1.0"
                edition = "2015"

                [features]
                shared = []
                only-a = []
            "#,
        )
        .file("crate-a/src/lib.rs", "")
        .file(
            "crate-b/Cargo.toml",
            r#"
                [package]
                name = "crate-b"
                version = "0.1.0"
                edition = "2015"

                [features]
                shared = []
                only-b = []
            "#,
        )
        .file("crate-b/src/lib.rs", "")
        .build()
}

#[cargo_test]
fn features_shared_across_workspace_members_are_one_candidate() {
    let p = workspace_with_shared_feature();

    // `shared` is defined by both members. It must appear once, not once per
    // package, because candidates are deduplicated by value downstream.
    p.cargo("--")
        .arg("cargo")
        .arg("build")
        .arg("--features")
        .arg("")
        .env("CARGO_COMPLETE", "bash")
        .env("_CLAP_COMPLETE_INDEX", "3")
        .masquerade_as_nightly_cargo(&["native-completions"])
        .with_stdout_data(str![[r#"
only-a
shared
only-b
shared
"#]])
        .run();
}

#[cargo_test]
fn features_shared_across_workspace_members_name_every_package() {
    let p = workspace_with_shared_feature();

    // The help for a shared feature has to name both packages; naming only the
    // first is what made the duplicate candidate misleading.
    p.cargo("--")
        .arg("cargo")
        .arg("build")
        .arg("--features")
        .arg("")
        .env("CARGO_COMPLETE", "fish")
        .env("_CLAP_COMPLETE_INDEX", "3")
        .masquerade_as_nightly_cargo(&["native-completions"])
        .with_stdout_data(str![[r#"
only-a	from crate-a
shared	from crate-a
only-b	from crate-b
shared	from crate-b

"#]])
        .run();
}
