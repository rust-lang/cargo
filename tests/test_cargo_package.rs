use std::io::{File, MemReader};

use tar::Archive;
use flate2::reader::GzDecoder;

use support::{project, execs, cargo_dir, ResultTest};
use support::{PACKAGING, VERIFYING, COMPILING};
use hamcrest::{assert_that, existing_file};

fn setup() {
}

test!(simple {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            exclude = ["*.txt"]
        "#)
        .file("src/main.rs", r#"
            fn main() { println!("hello"); }
        "#)
        .file("src/bar.txt", ""); // should be ignored when packaging

    assert_that(p.cargo_process("package"),
                execs().with_status(0).with_stdout(format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url()).as_slice()));
    assert_that(&p.root().join("target/package/foo-0.0.1.crate"), existing_file());
    assert_that(p.process(cargo_dir().join("cargo")).arg("package").arg("-l"),
                execs().with_status(0).with_stdout("\
Cargo.toml
src[..]main.rs
"));
    assert_that(p.process(cargo_dir().join("cargo")).arg("package"),
                execs().with_status(0).with_stdout(""));

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).assert();
    let mut rdr = GzDecoder::new(f);
    let contents = rdr.read_to_end().assert();
    let ar = Archive::new(MemReader::new(contents));
    for f in ar.files().assert() {
        let f = f.assert();
        let fname = f.filename_bytes();
        assert!(fname == Path::new("foo-0.0.1/Cargo.toml").as_vec() ||
                fname == Path::new("foo-0.0.1/src/main.rs").as_vec(),
                "unexpected filename: {}", f.filename())
    }
})
