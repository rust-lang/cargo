//! Tests for the `cargo package --list=json` format

use cargo_test_support::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"
                license = "MIT"
                description = "foo"
                documentation = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --list=json")
        .masquerade_as_nightly_cargo(&["package --list=json"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `--list=<FMT>` flag is unstable, pass `-Z unstable-options` to enable it
See https://github.com/rust-lang/cargo/issues/11666 for more information about the `--list=<FMT>` flag.

"#]])
        .run();
}

#[cargo_test]
fn single_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2015"
            license = "MIT"
            description = "foo"
            documentation = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --list=json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --list=json"])
        .with_stderr_data(str![""])
        .with_stdout_data(str![[r#"
{
  "path+[ROOTURL]/foo#0.0.0": ""
}
"#]].is_json())
        .run();
}

#[cargo_test]
fn workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["gondor", "rohan"]
            "#,
        )
        .file(
            "gondor/Cargo.toml",
            r#"
                [package]
                name = "gondor"
                edition = "2015"
                license = "MIT"
                description = "foo"
                documentation = "foo"
            "#,
        )
        .file("gondor/src/lib.rs", "")
        .file(
            "rohan/Cargo.toml",
            r#"
                [package]
                name = "rohan"
                edition = "2015"
                license = "MIT"
                description = "foo"
                documentation = "foo"
            "#,
        )
        .file("rohan/src/lib.rs", "")
        .build();

    p.cargo("package --list=json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --list=json"])
        .with_stderr_data(str![""])
        .with_stdout_data(str![[r#"
{
  "path+[ROOTURL]/foo/gondor#0.0.0": "",
  "path+[ROOTURL]/foo/rohan#0.0.0": ""
}
"#]].is_json())
        .run();
}
