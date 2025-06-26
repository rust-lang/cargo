//! Tests for workspace member discovery.

use crate::prelude::*;
use cargo::core::{Shell, Workspace};
use cargo::util::context::GlobalContext;
use cargo_test_support::paths;
use cargo_test_support::project;
use cargo_test_support::registry;

/// Tests exclusion of non-directory files from workspace member discovery using glob `*`.
#[cargo_test]
fn bad_file_member_exclusion() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = [ "crates/*" ]
            "#,
        )
        .file("crates/.DS_Store", "PLACEHOLDER")
        .file(
            "crates/bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("crates/bar/src/main.rs", "fn main() {}")
        .build();

    // Prevent this test from accessing the network by setting up .cargo/config.
    registry::init();
    let gctx = GlobalContext::new(
        Shell::from_write(Box::new(Vec::new())),
        paths::cargo_home(),
        paths::cargo_home(),
    );
    let ws = Workspace::new(&p.root().join("Cargo.toml"), &gctx).unwrap();
    assert_eq!(ws.members().count(), 1);
    assert_eq!(ws.members().next().unwrap().name(), "bar");
}
