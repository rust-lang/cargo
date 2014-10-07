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
            host = "{reg}"
            token = "api-token"
    "#, reg = registry()).as_slice()).assert();

    repo(&registry_path())
        .file("config.json", format!(r#"{{
            "dl": "",
            "upload": "{}"
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

    assert_that(p.cargo_process("upload"),
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

    let mut rdr = GzDecoder::new(File::open(&upload_path()).unwrap()).unwrap();
    assert_eq!(rdr.header().filename(), Some(b"foo-0.0.1.tar.gz"));
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

    assert_that(p.cargo_process("upload").arg("-v"),
                execs().with_status(101).with_stderr("\
failed to upload package to registry: [..]

Caused by:
  All dependencies must come from the same registry.
Dependency `foo` comes from git://path/to/nowhere instead
"));
})
