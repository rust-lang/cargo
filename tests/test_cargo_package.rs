use std::io::{File, MemReader};

use tar::Archive;
use flate2::reader::GzDecoder;
use cargo::util::process;

use support::{project, execs, cargo_dir, ResultTest, paths, git};
use support::{PACKAGING, VERIFYING, COMPILING, ARCHIVING};
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
            license = "MIT"
            description = "foo"
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
        assert!(fname == b"foo-0.0.1/Cargo.toml" ||
                fname == b"foo-0.0.1/src/main.rs",
                "unexpected filename: {}", f.filename())
    }
})

test!(metadata_warning {
    let p = project("all")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("package"),
                execs().with_status(0).with_stdout(format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url()).as_slice())
                .with_stderr("Warning: manifest has no description or license. See \
                              http://doc.crates.io/manifest.html#package-metadata for more info."));

    let p = project("one")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("package"),
                execs().with_status(0).with_stdout(format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url()).as_slice())
                .with_stderr("Warning: manifest has no description. See \
                              http://doc.crates.io/manifest.html#package-metadata for more info."));

    let p = project("both")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
        "#))
        .file("src/main.rs", r#"
            fn main() {}
        "#);
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
})

test!(package_verbose {
    let root = paths::root().join("all");
    let p = git::repo(&root)
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    p.build();
    let cargo = process(cargo_dir().join("cargo")).unwrap()
                                                  .cwd(root)
                                                  .env("HOME", Some(paths::home()));
    assert_that(cargo.clone().arg("build"), execs().with_status(0));
    assert_that(cargo.arg("package").arg("-v")
                                                    .arg("--no-verify"),
                execs().with_status(0).with_stdout(format!("\
{packaging} foo v0.0.1 ([..])
{archiving} [..]
{archiving} [..]
",
        packaging = PACKAGING,
        archiving = ARCHIVING).as_slice()));
})

test!(package_verification {
    let p = project("all")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.process(cargo_dir().join("cargo")).arg("package"),
                execs().with_status(0).with_stdout(format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url()).as_slice()));
})
