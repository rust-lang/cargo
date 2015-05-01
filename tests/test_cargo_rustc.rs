use std::path::MAIN_SEPARATOR as SEP;
use support::{execs, project, ProjectBuilder};
use support::{COMPILING, RUNNING};
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

test!(build_lib_for_foo {
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

    assert_that(p.cargo_process("rustc").arg("--lib").arg("-v").arg("foo"),
                execs()
                .with_status(0)
                .with_stdout(verbose_output_for_lib(&p)));
});
