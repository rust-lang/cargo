use std::env;

use support::{git, project, execs, main_file, basic_bin_manifest};
use support::{COMPILING, RUNNING};
use support::registry::Package;
use hamcrest::{assert_that, existing_dir, existing_file, is_not};

fn setup() {
}

test!(cargo_clean_simple {
    let p = project("foo")
              .file("Cargo.toml", &basic_bin_manifest("foo"))
              .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.build_dir(), existing_dir());

    assert_that(p.cargo("clean"),
                execs().with_status(0));
    assert_that(&p.build_dir(), is_not(existing_dir()));
});

test!(different_dir {
    let p = project("foo")
              .file("Cargo.toml", &basic_bin_manifest("foo"))
              .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
              .file("src/bar/a.rs", "");

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.build_dir(), existing_dir());

    assert_that(p.cargo("clean").cwd(&p.root().join("src")),
                execs().with_status(0).with_stdout(""));
    assert_that(&p.build_dir(), is_not(existing_dir()));
});

test!(clean_multiple_packages {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.d1]
                path = "d1"
            [dependencies.d2]
                path = "d2"

            [[bin]]
                name = "foo"
        "#)
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.1"
            authors = []

            [[bin]]
                name = "d1"
        "#)
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.1"
            authors = []

            [[bin]]
                name = "d2"
        "#)
        .file("d2/src/main.rs", "fn main() { println!(\"d2\"); }");
    p.build();

    assert_that(p.cargo_process("build").arg("-p").arg("d1").arg("-p").arg("d2")
                                        .arg("-p").arg("foo"),
                execs().with_status(0));

    let d1_path = &p.build_dir().join("debug").join("deps")
                                .join(format!("d1{}", env::consts::EXE_SUFFIX));
    let d2_path = &p.build_dir().join("debug").join("deps")
                                .join(format!("d2{}", env::consts::EXE_SUFFIX));


    assert_that(&p.bin("foo"), existing_file());
    assert_that(d1_path, existing_file());
    assert_that(d2_path, existing_file());

    assert_that(p.cargo("clean").arg("-p").arg("d1").arg("-p").arg("d2")
                                .cwd(&p.root().join("src")),
                execs().with_status(0).with_stdout(""));
    assert_that(&p.bin("foo"), existing_file());
    assert_that(d1_path, is_not(existing_file()));
    assert_that(d2_path, is_not(existing_file()));
});

test!(clean_release {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    p.build();

    assert_that(p.cargo_process("build").arg("--release"),
                execs().with_status(0));

    assert_that(p.cargo("clean").arg("-p").arg("foo"),
                execs().with_status(0));
    assert_that(p.cargo("build").arg("--release"),
                execs().with_status(0).with_stdout(""));

    assert_that(p.cargo("clean").arg("-p").arg("foo").arg("--release"),
                execs().with_status(0));
    assert_that(p.cargo("build").arg("--release"),
                execs().with_status(0).with_stdout(&format!("\
{compiling} foo v0.0.1 ([..])
", compiling = COMPILING)));
});

test!(build_script {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", r#"
            use std::path::PathBuf;
            use std::env;

            fn main() {
                let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
                if env::var("FIRST").is_ok() {
                    std::fs::File::create(out.join("out")).unwrap();
                } else {
                    assert!(!std::fs::metadata(out.join("out")).is_ok());
                }
            }
        "#)
        .file("a/src/lib.rs", "");
    p.build();

    assert_that(p.cargo_process("build").env("FIRST", "1"),
                execs().with_status(0));
    assert_that(p.cargo("clean").arg("-p").arg("foo"),
                execs().with_status(0));
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout(&format!("\
{compiling} foo v0.0.1 ([..])
{running} `rustc build.rs [..]`
{running} `[..]build-script-build[..]`
{running} `rustc src[..]main.rs [..]`
", compiling = COMPILING, running = RUNNING)));
});

test!(clean_git {
    let git = git::new("dep", |project| {
        project.file("Cargo.toml", r#"
            [project]
            name = "dep"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            dep = {{ git = '{}' }}
        "#, git.url()))
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.cargo("clean").arg("-p").arg("dep"),
                execs().with_status(0).with_stdout(""));
    assert_that(p.cargo("build"),
                execs().with_status(0));
});

test!(registry {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    Package::new("bar", "0.1.0").publish();

    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.cargo("clean").arg("-p").arg("bar"),
                execs().with_status(0).with_stdout(""));
    assert_that(p.cargo("build"),
                execs().with_status(0));
});
