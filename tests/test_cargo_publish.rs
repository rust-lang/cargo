use std::io::prelude::*;
use std::fs::File;
use std::io::SeekFrom;

use flate2::read::GzDecoder;
use tar::Archive;

use support::{project, execs};
use support::{UPDATING, PACKAGING, UPLOADING, ERROR};
use support::registry;

use hamcrest::assert_that;

fn setup() {
}

test!(simple {
    registry::init();

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

    assert_that(p.cargo_process("publish").arg("--no-verify")
                 .arg("--host").arg(registry::registry().to_string()),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `{reg}`
{packaging} foo v0.0.1 ({dir})
{uploading} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        uploading = UPLOADING,
        packaging = PACKAGING,
        dir = p.url(),
        reg = registry::registry())));

    let mut f = File::open(&registry::dl_path().join("api/v1/crates/new")).unwrap();
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
});

test!(git_deps {
    registry::init();

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
                execs().with_status(101).with_stderr(&format!("\
{error} all dependencies must come from the same source.
dependency `foo` comes from git://path/to/nowhere instead
",
error = ERROR)));
});

test!(path_dependency_no_version {
    registry::init();

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
                execs().with_status(101).with_stderr(&format!("\
{error} all path dependencies must have a version specified when publishing.
dependency `bar` does not specify a version
",
error = ERROR)));
});

test!(unpublishable_crate {
    registry::init();

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
                execs().with_status(101).with_stderr(&format!("\
{error} some crates cannot be published.
`foo` is marked as unpublishable
",
error = ERROR)));
});
