use crate::prelude::*;
use cargo_test_support::project;

#[cargo_test]
fn feature_gate_accepted() {
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
                "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .run();
}
