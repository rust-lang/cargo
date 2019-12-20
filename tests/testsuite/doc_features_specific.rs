//! Tests for "doc"-specific features.

use cargo_test_support::project;

#[cargo_test]
fn bad1() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [package.metadata.doc]
            features = "foo"
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("doc -v")
        .with_status(1)
        .with_stderr("error: `features` should be an array")
        .run();
}

#[cargo_test]
fn bad2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [package.metadata.doc]
            features = [12]
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("doc -v")
        .with_status(1)
        .with_stderr("error: Only strings are allowed in `features` array")
        .run();
}

#[cargo_test]
fn bad3() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [package.metadata.doc]
            features = ["foo"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("doc -v")
        .with_status(101)
        .with_stderr(
            "error: Package `foo v0.1.0 ([..])` does not \
             have these features: `foo`",
        )
        .run();
}

#[cargo_test]
fn success() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [features]
            foo = []

            [package.metadata.doc]
            features = ["foo"]
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "foo")]
            compile_error!("foo is crazy!");
        "#,
        )
        .build();
    p.cargo("doc -v")
        .with_status(101)
        .with_stderr_contains(
            "\
3 |             compile_error!(\"foo is crazy!\");
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: Compilation failed, aborting rustdoc

error: aborting due to 2 previous errors",
        )
        .run();
}

#[cargo_test]
fn success2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [features]
            foo = []

            [package.metadata.doc]
            features = ["foo"]
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(feature = "foo")]
            compile_error!("foo is crazy!");
        "#,
        )
        .build();
    p.cargo("build -v").with_status(0).run();
}
