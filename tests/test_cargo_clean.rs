use std::env;

use support::{project, execs, main_file, basic_bin_manifest};
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
