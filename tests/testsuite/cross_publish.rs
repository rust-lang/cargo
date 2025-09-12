//! Tests for publishing using the `--target` flag.

use std::fs::File;

use crate::prelude::*;
use crate::utils::cross_compile::disabled as cross_compile_disabled;
use cargo_test_support::{cross_compile, project, publish, registry, str};

#[cargo_test]
fn simple_cross_package() {
    if cross_compile_disabled() {
        return;
    }
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.0 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.0 ([ROOT]/foo)
[COMPILING] foo v0.0.0 ([ROOT]/foo/target/package/foo-0.0.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Check that the tarball contains the files
    let f = File::open(&p.root().join("target/package/foo-0.0.0.crate")).unwrap();
    publish::validate_crate_contents(
        f,
        "foo-0.0.0.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        (),
    );
}

#[cargo_test]
fn publish_with_target() {
    if cross_compile_disabled() {
        return;
    }

    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
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

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .arg("--target")
        .arg(&target)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.0.0 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.0 ([ROOT]/foo)
[COMPILING] foo v0.0.0 ([ROOT]/foo/target/package/foo-0.0.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.0 ([ROOT]/foo)
[UPLOADED] foo v0.0.0 to registry `crates-io`
[NOTE] waiting for foo v0.0.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.0 at registry `crates-io`

"#]])
        .run();
}
