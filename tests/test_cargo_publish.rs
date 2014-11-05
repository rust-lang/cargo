use std::io::{mod, fs, File, MemReader};

use flate2::reader::GzDecoder;
use tar::Archive;
use url::Url;

use support::{ResultTest, project, execs};
use support::{UPDATING, PACKAGING, UPLOADING};
use support::paths;
use support::git::repo;

use hamcrest::assert_that;

fn registry_path() -> Path { paths::root().join("registry") }
fn registry() -> Url { Url::from_file_path(&registry_path()).unwrap() }
fn upload_path() -> Path { paths::root().join("upload") }
fn upload() -> Url { Url::from_file_path(&upload_path()).unwrap() }

fn setup() {
    let config = paths::root().join(".cargo/config");
    fs::mkdir_recursive(&config.dir_path(), io::USER_DIR).assert();
    File::create(&config).write_str(format!(r#"
        [registry]
            index = "{reg}"
            token = "api-token"
    "#, reg = registry()).as_slice()).assert();
    fs::mkdir_recursive(&upload_path().join("api/v1/crates"), io::USER_DIR).assert();

    repo(&registry_path())
        .file("config.json", format!(r#"{{
            "dl": "{0}",
            "api": "{0}"
        }}"#, upload()))
        .build();
}

test!(simple {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("publish").arg("--no-verify"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
{packaging} foo v0.0.1 ({dir})
{uploading} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        uploading = UPLOADING,
        packaging = PACKAGING,
        dir = p.url(),
        reg = registry()).as_slice()));

    let mut f = File::open(&upload_path().join("api/v1/crates/new")).unwrap();
    // Skip the metadata payload and the size of the tarball
    let sz = f.read_le_u32().unwrap();
    f.seek(sz as i64 + 4, io::SeekCur).unwrap();

    // Verify the tarball
    let mut rdr = GzDecoder::new(f).unwrap();
    assert_eq!(rdr.header().filename(), Some(b"foo-0.0.1.crate"));
    let inner = MemReader::new(rdr.read_to_end().unwrap());
    let ar = Archive::new(inner);
    for file in ar.files().unwrap() {
        let file = file.unwrap();
        let fname = file.filename_bytes();
        assert!(fname == Path::new("foo-0.0.1/Cargo.toml").as_vec() ||
                fname == Path::new("foo-0.0.1/src/main.rs").as_vec(),
                "unexpected filename: {}", file.filename())
    }
})

test!(git_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.foo]
            git = "git://path/to/nowhere"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("publish").arg("-v").arg("--no-verify"),
                execs().with_status(101).with_stderr("\
all dependencies must come from the same registry
dependency `foo` comes from git://path/to/nowhere instead
"));
})

test!(path_dependency_no_version {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

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
all path dependencies must have a version specified when being uploaded \
to the registry
dependency `bar` does not specify a version
"));
})
