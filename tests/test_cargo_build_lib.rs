use std::path::MAIN_SEPARATOR as SEP;
use support::{basic_bin_manifest, execs, project, ProjectBuilder};
use support::{COMPILING, RUNNING, ERROR};
use hamcrest::{assert_that};

fn setup() {
}

fn verbose_output_for_lib(p: &ProjectBuilder) -> String {
    format!("\
{compiling} {name} v{version} ({url})
{running} `rustc src{sep}lib.rs --crate-name {name} --crate-type lib -g \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, compiling = COMPILING, sep = SEP,
            dir = p.root().display(), url = p.url(),
            name = "foo", version = "0.0.1")
}

test!(build_lib_only {
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
                .with_stdout(verbose_output_for_lib(&p)));
});


test!(build_with_no_lib {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("build").arg("--lib"),
                execs().with_status(101)
                       .with_stderr(&format!("{error} no library targets found", error = ERROR)));
});

test!(build_with_relative_cargo_home_path {
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
});
