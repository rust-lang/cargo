use std::fs::File;
use std::io::Cursor;
use std::io::prelude::*;
use std::path::Path;

use cargo::util::process;
use flate2::read::GzDecoder;
use git2;
use tar::Archive;

use support::{project, execs, cargo_dir, paths, git, path2url};
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
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url())));
    assert_that(&p.root().join("target/package/foo-0.0.1.crate"), existing_file());
    assert_that(p.cargo("package").arg("-l"),
                execs().with_status(0).with_stdout("\
Cargo.toml
src[..]main.rs
"));
    assert_that(p.cargo("package"),
                execs().with_status(0).with_stdout(""));

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let mut rdr = GzDecoder::new(f).unwrap();
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let ar = Archive::new(Cursor::new(contents));
    for f in ar.files().unwrap() {
        let f = f.unwrap();
        let fname = f.filename_bytes();
        assert!(fname == b"foo-0.0.1/Cargo.toml" ||
                fname == b"foo-0.0.1/src/main.rs",
                "unexpected filename: {:?}", f.filename())
    }
});

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
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url()))
                .with_stderr("\
warning: manifest has no description, license, license-file, documentation, \
homepage or repository. See \
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
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url()))
                .with_stderr("\
warning: manifest has no description, documentation, homepage or repository. See \
http://doc.crates.io/manifest.html#package-metadata for more info."));

    let p = project("all")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            repository = "bar"
        "#))
        .file("src/main.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("package"),
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url())));
});

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
    let mut cargo = process(&cargo_dir().join("cargo")).unwrap();
    cargo.cwd(&root).env("HOME", &paths::home());
    assert_that(cargo.clone().arg("build"), execs().with_status(0));
    assert_that(cargo.arg("package").arg("-v").arg("--no-verify"),
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ([..])
{archiving} [..]
{archiving} [..]
",
        packaging = PACKAGING,
        archiving = ARCHIVING)));
});

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
    assert_that(p.cargo("package"),
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url())));
});

test!(exclude {
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
        .file("bar.txt", "")
        .file("src/bar.txt", "");

    assert_that(p.cargo_process("package").arg("--no-verify").arg("-v"),
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ([..])
{archiving} [..]
{archiving} [..]
", packaging = PACKAGING, archiving = ARCHIVING)));
});

test!(include {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            exclude = ["*.txt"]
            include = ["foo.txt", "**/*.rs", "Cargo.toml"]
        "#)
        .file("foo.txt", "")
        .file("src/main.rs", r#"
            fn main() { println!("hello"); }
        "#)
        .file("src/bar.txt", ""); // should be ignored when packaging

    assert_that(p.cargo_process("package").arg("--no-verify").arg("-v"),
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ([..])
{archiving} [..]
{archiving} [..]
{archiving} [..]
", packaging = PACKAGING, archiving = ARCHIVING)));
});

test!(package_lib_with_bin {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            extern crate foo;
            fn main() {}
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("package").arg("-v"),
                execs().with_status(0));
});

test!(package_new_git_repo {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();
    git2::Repository::init(&p.root()).unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("package")
                 .arg("--no-verify").arg("-v"),
                execs().with_status(0).with_stdout(&format!("\
{packaging} foo v0.0.1 ([..])
{archiving} [..]
{archiving} [..]
", packaging = PACKAGING, archiving = ARCHIVING)));
});

test!(package_git_submodule {
    use std::str::from_utf8;

    let project = git::new("foo", |project| {
        project.file("Cargo.toml", r#"
                    [project]
                    name = "foo"
                    version = "0.0.1"
                    authors = ["foo@example.com"]
                    license = "MIT"
                    description = "foo"
                    repository = "foo"
                "#)
                .file("src/lib.rs", "pub fn foo() {}")
    }).unwrap();
    let library = git::new("bar", |library| {
        library.file("Makefile", "all:")
    }).unwrap();

    let repository = git2::Repository::open(&project.root()).unwrap();
    let url = path2url(library.root()).to_string();
    git::add_submodule(&repository, &url, Path::new("bar"));
    git::commit(&repository);

    let repository = git2::Repository::open(&project.root().join("bar")).unwrap();
    repository.reset(&repository.revparse_single("HEAD").unwrap(),
                     git2::ResetType::Hard, None).unwrap();

    let result = project.cargo("package").arg("--no-verify").arg("-v").exec_with_output().unwrap();
    assert!(result.status.success());
    assert!(from_utf8(&result.stdout).unwrap().contains(&format!("{} bar/Makefile", ARCHIVING)));
});

test!(ignore_nested {
    let cargo_toml = r#"
            [project]
            name = "nested"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "nested"
        "#;
    let main_rs = r#"
            fn main() { println!("hello"); }
        "#;
    let p = project("nested")
        .file("Cargo.toml", cargo_toml)
        .file("src/main.rs", main_rs)
        // If a project happens to contain a copy of itself, we should
        // ignore it.
        .file("a_dir/nested/Cargo.toml", cargo_toml)
        .file("a_dir/nested/src/main.rs", main_rs);

    assert_that(p.cargo_process("package"),
                execs().with_status(0).with_stdout(&format!("\
{packaging} nested v0.0.1 ({dir})
{verifying} nested v0.0.1 ({dir})
{compiling} nested v0.0.1 ({dir}[..])
",
        packaging = PACKAGING,
        verifying = VERIFYING,
        compiling = COMPILING,
        dir = p.url())));
    assert_that(&p.root().join("target/package/nested-0.0.1.crate"), existing_file());
    assert_that(p.cargo("package").arg("-l"),
                execs().with_status(0).with_stdout("\
Cargo.toml
src[..]main.rs
"));
    assert_that(p.cargo("package"),
                execs().with_status(0).with_stdout(""));

    let f = File::open(&p.root().join("target/package/nested-0.0.1.crate")).unwrap();
    let mut rdr = GzDecoder::new(f).unwrap();
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let ar = Archive::new(Cursor::new(contents));
    for f in ar.files().unwrap() {
        let f = f.unwrap();
        let fname = f.filename_bytes();
        assert!(fname == b"nested-0.0.1/Cargo.toml" ||
                fname == b"nested-0.0.1/src/main.rs",
                "unexpected filename: {:?}", f.filename())
    }
});
