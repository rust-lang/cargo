use std::io::prelude::*;
use std::fs::{self, File};
use std::io::SeekFrom;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use tar::Archive;
use url::Url;

use support::{project, execs};
use support::paths;
use support::git::repo;

use hamcrest::assert_that;

fn registry_path() -> PathBuf { paths::root().join("registry") }
fn registry() -> Url { Url::from_file_path(&*registry_path()).ok().unwrap() }
fn upload_path() -> PathBuf { paths::root().join("upload") }
fn upload() -> Url { Url::from_file_path(&*upload_path()).ok().unwrap() }

fn setup() {
    let config = paths::root().join(".cargo/config");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    File::create(&config).unwrap().write_all(&format!(r#"
        [registry]
            index = "{reg}"
            token = "api-token"
    "#, reg = registry()).as_bytes()).unwrap();
    fs::create_dir_all(&upload_path().join("api/v1/crates")).unwrap();

    repo(&registry_path())
        .file("config.json", &format!(r#"{{
            "dl": "{0}",
            "api": "{0}"
        }}"#, upload()))
        .build();
}

#[test]
fn simple() {
    setup();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("publish").arg("--no-verify"),
                execs().with_status(0).with_stderr(&format!("\
[UPDATING] registry `{reg}`
[WARNING] manifest has no documentation, [..]
[PACKAGING] foo v0.0.1 ({dir})
[UPLOADING] foo v0.0.1 ({dir})
",
        dir = p.url(),
        reg = registry())));

    let mut f = File::open(&upload_path().join("api/v1/crates/new")).unwrap();
    // Skip the metadata payload and the size of the tarball
    let mut sz = [0; 4];
    assert_eq!(f.read(&mut sz).unwrap(), 4);
    let sz = ((sz[0] as u32) <<  0) |
             ((sz[1] as u32) <<  8) |
             ((sz[2] as u32) << 16) |
             ((sz[3] as u32) << 24);
    f.seek(SeekFrom::Current(sz as i64 + 4)).unwrap();

    // Verify the tarball
    let mut rdr = GzDecoder::new(f).unwrap();
    assert_eq!(rdr.header().filename().unwrap(), "foo-0.0.1.crate".as_bytes());
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    for file in ar.entries().unwrap() {
        let file = file.unwrap();
        let fname = file.header().path_bytes();
        let fname = &*fname;
        assert!(fname == b"foo-0.0.1/Cargo.toml" ||
                fname == b"foo-0.0.1/src/main.rs",
                "unexpected filename: {:?}", file.header().path());
    }
}

#[test]
fn git_deps() {
    setup();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"

            [dependencies.foo]
            git = "git://path/to/nowhere"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("publish").arg("-v").arg("--no-verify"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[ERROR] all dependencies must come from the same source.
dependency `foo` comes from git://path/to/nowhere instead
"));
}

#[test]
fn path_dependency_no_version() {
    setup();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "");

    assert_that(p.cargo_process("publish"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[ERROR] all path dependencies must have a version specified when publishing.
dependency `bar` does not specify a version
"));
}

#[test]
fn unpublishable_crate() {
    setup();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            publish = false
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("publish"),
                execs().with_status(101).with_stderr("\
[ERROR] some crates cannot be published.
`foo` is marked as unpublishable
"));
}
