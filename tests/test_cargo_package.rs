use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use flate2::read::GzDecoder;
use git2;
use tar::Archive;

use support::{project, execs, paths, git, path2url};
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
                execs().with_status(0).with_stderr(&format!("\
[WARNING] manifest has no documentation[..]
[PACKAGING] foo v0.0.1 ({dir})
[VERIFYING] foo v0.0.1 ({dir})
[COMPILING] foo v0.0.1 ({dir}[..])
",
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
    let mut ar = Archive::new(&contents[..]);
    for f in ar.entries().unwrap() {
        let f = f.unwrap();
        let fname = f.header().path_bytes();
        let fname = &*fname;
        assert!(fname == b"foo-0.0.1/Cargo.toml" ||
                fname == b"foo-0.0.1/src/main.rs",
                "unexpected filename: {:?}", f.header().path())
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
                execs().with_status(0).with_stderr(&format!("\
warning: manifest has no description, license, license-file, documentation, \
homepage or repository. See \
http://doc.crates.io/manifest.html#package-metadata for more info.
[PACKAGING] foo v0.0.1 ({dir})
[VERIFYING] foo v0.0.1 ({dir})
[COMPILING] foo v0.0.1 ({dir}[..])
",
        dir = p.url())));

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
                execs().with_status(0).with_stderr(&format!("\
warning: manifest has no description, documentation, homepage or repository. See \
http://doc.crates.io/manifest.html#package-metadata for more info.
[PACKAGING] foo v0.0.1 ({dir})
[VERIFYING] foo v0.0.1 ({dir})
[COMPILING] foo v0.0.1 ({dir}[..])
",
        dir = p.url())));

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
                execs().with_status(0).with_stderr(&format!("\
[PACKAGING] foo v0.0.1 ({dir})
[VERIFYING] foo v0.0.1 ({dir})
[COMPILING] foo v0.0.1 ({dir}[..])
",
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
    let mut cargo = ::cargo_process();
    cargo.cwd(p.root());
    assert_that(cargo.clone().arg("build"), execs().with_status(0));

    println!("package main repo");
    assert_that(cargo.clone().arg("package").arg("-v").arg("--no-verify"),
                execs().with_status(0).with_stderr("\
[WARNING] manifest has no description[..]
[PACKAGING] foo v0.0.1 ([..])
[ARCHIVING] [..]
[ARCHIVING] [..]
"));

    println!("package sub-repo");
    assert_that(cargo.arg("package").arg("-v").arg("--no-verify")
                     .cwd(p.root().join("a")),
                execs().with_status(0).with_stderr("\
[WARNING] manifest has no description[..]
[PACKAGING] a v0.0.1 ([..])
[ARCHIVING] [..]
[ARCHIVING] [..]
"));
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
                execs().with_status(0).with_stderr(&format!("\
[WARNING] manifest has no description[..]
[PACKAGING] foo v0.0.1 ({dir})
[VERIFYING] foo v0.0.1 ({dir})
[COMPILING] foo v0.0.1 ({dir}[..])
",
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
                execs().with_status(0).with_stderr("\
[WARNING] manifest has no description[..]
[PACKAGING] foo v0.0.1 ([..])
[ARCHIVING] [..]
[ARCHIVING] [..]
"));
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
                execs().with_status(0).with_stderr("\
[WARNING] manifest has no description[..]
[PACKAGING] foo v0.0.1 ([..])
[ARCHIVING] [..]
[ARCHIVING] [..]
[ARCHIVING] [..]
"));
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

    assert_that(::cargo_process().arg("package").cwd(p.root())
                 .arg("--no-verify").arg("-v"),
                execs().with_status(0).with_stderr("\
[WARNING] manifest has no description[..]
[PACKAGING] foo v0.0.1 ([..])
[ARCHIVING] [..]
[ARCHIVING] [..]
"));
});

test!(package_git_submodule {
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

    assert_that(::cargo_process().arg("package").cwd(project.root())
                 .arg("--no-verify").arg("-v"),
                execs().with_status(0).with_stderr_contains("[ARCHIVING] bar/Makefile"));
});

test!(no_duplicates_from_modified_tracked_files {
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
        "#);
    p.build();
    File::create(p.root().join("src/main.rs")).unwrap().write_all(r#"
            fn main() { println!("A change!"); }
        "#.as_bytes()).unwrap();
    let mut cargo = ::cargo_process();
    cargo.cwd(p.root());
    assert_that(cargo.clone().arg("build"), execs().with_status(0));
    assert_that(cargo.arg("package").arg("--list"),
                execs().with_status(0).with_stdout("\
Cargo.toml
src/main.rs
"));
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
                execs().with_status(0).with_stderr(&format!("\
[WARNING] manifest has no documentation[..]
[PACKAGING] nested v0.0.1 ({dir})
[VERIFYING] nested v0.0.1 ({dir})
[COMPILING] nested v0.0.1 ({dir}[..])
",
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
    let mut ar = Archive::new(&contents[..]);
    for f in ar.entries().unwrap() {
        let f = f.unwrap();
        let fname = f.header().path_bytes();
        let fname = &*fname;
        assert!(fname == b"nested-0.0.1/Cargo.toml" ||
                fname == b"nested-0.0.1/src/main.rs",
                "unexpected filename: {:?}", f.header().path())
    }
});

#[cfg(unix)] // windows doesn't allow these characters in filenames
test!(package_weird_characters {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() { println!("hello"); }
        "#)
        .file("src/:foo", "");

    assert_that(p.cargo_process("package"),
                execs().with_status(101).with_stderr("\
warning: [..]
[PACKAGING] foo [..]
[ERROR] failed to prepare local package for uploading

Caused by:
  cannot package a filename with a special character `:`: src/:foo
"));
});
