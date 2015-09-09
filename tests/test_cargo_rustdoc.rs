use std::path::MAIN_SEPARATOR as SEP;
use support::{execs, project};
use support::{COMPILING, RUNNING};
use hamcrest::{assert_that};

fn setup() {
}


test!(rustdoc_simple {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustdoc").arg("-v"),
                execs()
                .with_status(0)
                .with_stdout(format!("\
{compiling} foo v0.0.1 ({url})
{running} `rustdoc src{sep}lib.rs --crate-name foo \
        -o {dir}{sep}target{sep}doc \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, compiling = COMPILING, sep = SEP,
            dir = p.root().display(), url = p.url())));
});

test!(rustdoc_args {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustdoc").arg("-v").arg("--").arg("--no-defaults"),
                execs()
                .with_status(0)
                .with_stdout(format!("\
{compiling} foo v0.0.1 ({url})
{running} `rustdoc src{sep}lib.rs --crate-name foo \
        -o {dir}{sep}target{sep}doc \
        --no-defaults \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, compiling = COMPILING, sep = SEP,
            dir = p.root().display(), url = p.url())));
});

