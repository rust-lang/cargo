extern crate cargo;
extern crate cargotest;
extern crate hamcrest;
extern crate flate2;
extern crate tar;

use std::fs::File;
use std::path::PathBuf;
use std::io::prelude::*;

use cargotest::support::{project, execs, cross_compile, publish};
use hamcrest::{assert_that, contains};
use flate2::read::GzDecoder;
use tar::Archive;

#[test]
fn simple_cross_package() {
    if cross_compile::disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            license = "MIT"
            description = "foo"
            repository = "bar"
        "#)
        .file("src/main.rs", &format!(r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#, cross_compile::alternate_arch()));

    let target = cross_compile::alternate();

    assert_that(p.cargo_process("package").arg("--target").arg(&target),
                execs().with_status(0).with_status(0).with_stderr(&format!(
"   Packaging foo v0.0.0 ({dir})
   Verifying foo v0.0.0 ({dir})
   Compiling foo v0.0.0 ({dir}/target/package/foo-0.0.0)
    Finished dev [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));

    // Check that the tarball contains the files
    let f = File::open(&p.root().join("target/package/foo-0.0.0.crate")).unwrap();
    let mut rdr = GzDecoder::new(f).unwrap();
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    let entries = ar.entries().unwrap();
    let entry_paths = entries.map(|entry| {
        entry.unwrap().path().unwrap().into_owned()
    }).collect::<Vec<PathBuf>>();
    assert_that(&entry_paths, contains(vec![PathBuf::from("foo-0.0.0/Cargo.toml")]));
    assert_that(&entry_paths, contains(vec![PathBuf::from("foo-0.0.0/Cargo.toml.orig")]));
    assert_that(&entry_paths, contains(vec![PathBuf::from("foo-0.0.0/src/main.rs")]));
}

#[test]
fn publish_with_target() {
    if cross_compile::disabled() { return }

    publish::setup();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            license = "MIT"
            description = "foo"
            repository = "bar"
        "#)
        .file("src/main.rs", &format!(r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#, cross_compile::alternate_arch()));

    p.build();

    let target = cross_compile::alternate();

    assert_that(p.cargo("publish")
                 .arg("--index").arg(publish::registry().to_string())
                 .arg("--target").arg(&target),
                execs().with_status(0).with_stderr(&format!(
"    Updating registry `{registry}`
   Packaging foo v0.0.0 ({dir})
   Verifying foo v0.0.0 ({dir})
   Compiling foo v0.0.0 ({dir}/target/package/foo-0.0.0)
    Finished dev [unoptimized + debuginfo] target(s) in [..]
   Uploading foo v0.0.0 ({dir})
", dir = p.url(), registry = publish::registry())));
}