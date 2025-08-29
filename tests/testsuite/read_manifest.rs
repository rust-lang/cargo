//! Tests for the `cargo read-manifest` command.

use crate::prelude::*;
use cargo_test_support::{basic_bin_manifest, main_file, project, str};

pub fn basic_bin_manifest_with_readme(name: &str, readme_filename: &str) -> String {
    format!(
        r#"
            [package]

            name = "{}"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            readme = {}

            [[bin]]

            name = "{}"
        "#,
        name, readme_filename, name
    )
}

#[cargo_test]
fn cargo_read_manifest_path_to_cargo_toml_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest --manifest-path foo/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .with_stdout_data(
            str![[r#"
{
  "readme": null,
  "...": "{...}"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn cargo_read_manifest_path_to_cargo_toml_absolute() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest --manifest-path")
        .arg(p.root().join("Cargo.toml"))
        .cwd(p.root().parent().unwrap())
        .with_stdout_data(
            str![[r#"
{
  "readme": null,
  "...": "{...}"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn cargo_read_manifest_path_to_cargo_toml_parent_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest --manifest-path foo")
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]])
        .run();
}

#[cargo_test]
fn cargo_read_manifest_path_to_cargo_toml_parent_absolute() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest --manifest-path")
        .arg(p.root())
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]])
        .run();
}

#[cargo_test]
fn cargo_read_manifest_cwd() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest")
        .with_stdout_data(
            str![[r#"
{
  "readme": null,
  "...": "{...}"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn cargo_read_manifest_with_specified_readme() {
    let p = project()
        .file(
            "Cargo.toml",
            &basic_bin_manifest_with_readme("foo", r#""SomeReadme.txt""#),
        )
        .file("SomeReadme.txt", "Sample Project")
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest")
        .with_stdout_data(
            str![[r#"
{
  "readme": "SomeReadme.txt",
  "...": "{...}"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn cargo_read_manifest_default_readme() {
    let assert_output = |readme, expected| {
        let p = project()
            .file("Cargo.toml", &basic_bin_manifest("foo"))
            .file(readme, "Sample project")
            .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
            .build();

        p.cargo("read-manifest").with_stdout_data(expected).run();
    };

    assert_output(
        "README.md",
        str![[r#"
{
  "readme": "README.md",
  "...": "{...}"
}
"#]]
        .is_json(),
    );

    assert_output(
        "README.txt",
        str![[r#"
{
  "readme": "README.txt",
  "...": "{...}"
}
"#]]
        .is_json(),
    );

    assert_output(
        "README",
        str![[r#"
{
  "readme": "README",
  "...": "{...}"
}
"#]]
        .is_json(),
    );
}

#[cargo_test]
fn cargo_read_manifest_suppress_default_readme() {
    let p = project()
        .file(
            "Cargo.toml",
            &basic_bin_manifest_with_readme("foo", "false"),
        )
        .file("README.txt", "Sample project")
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest")
        .with_stdout_data(
            str![[r#"
{
  "readme": null,
  "...": "{...}"
}
"#]]
            .is_json(),
        )
        .run();
}

// If a file named README.md exists, and `readme = true`, the value `README.md` should be defaulted in.
#[cargo_test]
fn cargo_read_manifest_defaults_readme_if_true() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest_with_readme("foo", "true"))
        .file("README.md", "Sample project")
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("read-manifest")
        .with_stdout_data(
            str![[r#"
{
  "readme": "README.md",
  "...": "{...}"
}
"#]]
            .is_json(),
        )
        .run();
}
