extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{basic_bin_manifest, execs, project, ProjectBuilder};
use hamcrest::{assert_that};

fn verbose_output_for_lib(p: &ProjectBuilder) -> String {
    format!("\
[COMPILING] {name} v{version} ({url})
[RUNNING] `rustc --crate-name {name} src[/]lib.rs --crate-type lib \
        --emit=dep-info,link -g \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.root().display(), url = p.url(),
            name = "foo", version = "0.0.1")
}

#[test]
fn build_lib_only() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]

            name = "foo"
            version = "0.0.1"
            authors = ["wycats@example.com"]
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("build").arg("--lib").arg("-v"),
                execs()
                .with_status(0)
                .with_stderr(verbose_output_for_lib(&p)));
}


#[test]
fn build_with_no_lib() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("build").arg("--lib"),
                execs().with_status(101)
                       .with_stderr("[ERROR] no library targets found"));
}

#[test]
fn build_with_relative_cargo_home_path() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]

            name = "foo"
            version = "0.0.1"
            authors = ["wycats@example.com"]

            [dependencies]

            "test-dependency" = { path = "src/test_dependency" }
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("src/test_dependency/src/lib.rs", r#" "#)
        .file("src/test_dependency/Cargo.toml", r#"
            [package]

            name = "test-dependency"
            version = "0.0.1"
            authors = ["wycats@example.com"]
        "#);

    assert_that(p.cargo_process("build").env("CARGO_HOME", "./cargo_home/"),
                execs()
                .with_status(0));
}
