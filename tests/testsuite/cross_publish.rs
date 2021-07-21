//! Tests for publishing using the `--target` flag.

use std::fs::File;

use cargo_test_support::{cross_compile, project, publish, registry};

#[cargo_test]
fn simple_cross_package() {
    if cross_compile::disabled() {
        return;
    }
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                license = "MIT"
                description = "foo"
                repository = "bar"
            "#,
        )
        .file(
            "src/main.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();

    p.cargo("package --target")
        .arg(&target)
        .with_stderr(
            "\
[PACKAGING] foo v0.0.0 ([CWD])
[VERIFYING] foo v0.0.0 ([CWD])
[COMPILING] foo v0.0.0 ([CWD]/target/package/foo-0.0.0)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // Check that the tarball contains the files
    let f = File::open(&p.root().join("target/package/foo-0.0.0.crate")).unwrap();
    publish::validate_crate_contents(
        f,
        "foo-0.0.0.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        &[],
    );
}

#[cargo_test]
fn publish_with_target() {
    if cross_compile::disabled() {
        return;
    }

    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                license = "MIT"
                description = "foo"
                repository = "bar"
            "#,
        )
        .file(
            "src/main.rs",
            &format!(
                r#"
                    use std::env;
                    fn main() {{
                        assert_eq!(env::consts::ARCH, "{}");
                    }}
                "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();

    p.cargo("publish --token sekrit")
        .arg("--target")
        .arg(&target)
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[PACKAGING] foo v0.0.0 ([CWD])
[VERIFYING] foo v0.0.0 ([CWD])
[COMPILING] foo v0.0.0 ([CWD]/target/package/foo-0.0.0)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[UPLOADING] foo v0.0.0 ([CWD])
",
        )
        .run();
}
