use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn direct_repository_in_explicit_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[package]
name = "member"
version = "0.1.0"
edition = "2015"
repository = "https://example.com/member"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("fetch").with_stderr_data(str![""]).run();
}
