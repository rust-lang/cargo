use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use crate::support::{cross_compile, project, publish};
use tar::Archive;

#[test]
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
        ).file(
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
        ).build();

    let target = cross_compile::alternate();

    p.cargo("package --target")
        .arg(&target)
        .with_stderr(
            "   Packaging foo v0.0.0 ([CWD])
   Verifying foo v0.0.0 ([CWD])
   Compiling foo v0.0.0 ([CWD]/target/package/foo-0.0.0)
    Finished dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();

    // Check that the tarball contains the files
    let f = File::open(&p.root().join("target/package/foo-0.0.0.crate")).unwrap();
    let mut rdr = GzDecoder::new(f);
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    let entries = ar.entries().unwrap();
    let entry_paths = entries
        .map(|entry| entry.unwrap().path().unwrap().into_owned())
        .collect::<Vec<PathBuf>>();
    assert!(entry_paths.contains(&PathBuf::from("foo-0.0.0/Cargo.toml")));
    assert!(entry_paths.contains(&PathBuf::from("foo-0.0.0/Cargo.toml.orig")));
    assert!(entry_paths.contains(&PathBuf::from("foo-0.0.0/src/main.rs")));
}

#[test]
fn publish_with_target() {
    if cross_compile::disabled() {
        return;
    }

    publish::setup();

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
        ).file(
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
        ).build();

    let target = cross_compile::alternate();

    p.cargo("publish --index")
        .arg(publish::registry().to_string())
        .arg("--target")
        .arg(&target)
        .with_stderr(&format!(
            "    Updating `{registry}` index
   Packaging foo v0.0.0 ([CWD])
   Verifying foo v0.0.0 ([CWD])
   Compiling foo v0.0.0 ([CWD]/target/package/foo-0.0.0)
    Finished dev [unoptimized + debuginfo] target(s) in [..]
   Uploading foo v0.0.0 ([CWD])
",
            registry = publish::registry_path().to_str().unwrap()
        )).run();
}
