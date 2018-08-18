use support::{basic_bin_manifest, execs, main_file, project};
use filetime::FileTime;
use support::hamcrest::{assert_that, existing_file};

#[test]
fn build_dep_info() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(p.cargo("build"), execs());

    let depinfo_bin_path = &p.bin("foo").with_extension("d");

    assert_that(depinfo_bin_path, existing_file());
}

#[test]
fn build_dep_info_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["lib"]
        "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    assert_that(p.cargo("build --example=ex"), execs());
    assert_that(
        &p.example_lib("ex", "lib").with_extension("d"),
        existing_file(),
    );
}

#[test]
fn build_dep_info_rlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    assert_that(p.cargo("build --example=ex"), execs());
    assert_that(
        &p.example_lib("ex", "rlib").with_extension("d"),
        existing_file(),
    );
}

#[test]
fn build_dep_info_dylib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["dylib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    assert_that(p.cargo("build --example=ex"), execs());
    assert_that(
        &p.example_lib("ex", "dylib").with_extension("d"),
        existing_file(),
    );
}

#[test]
fn no_rewrite_if_no_change() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());
    let dep_info = p.root().join("target/debug/libfoo.d");
    let metadata1 = dep_info.metadata().unwrap();
    assert_that(p.cargo("build"), execs());
    let metadata2 = dep_info.metadata().unwrap();

    assert_eq!(
        FileTime::from_last_modification_time(&metadata1),
        FileTime::from_last_modification_time(&metadata2),
    );
}
