use std::path::MAIN_SEPARATOR as SEP;
use support::{execs, project};
use support::{COMPILING, RUNNING};
use hamcrest::{assert_that};

fn setup() {
}

test!(build_lib_for_foo {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustc").arg("--lib").arg("-v"),
                execs()
                .with_status(0)
                .with_stdout(format!("\
{compiling} foo v0.0.1 ({url})
{running} `rustc src{sep}lib.rs --crate-name foo --crate-type lib -g \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, compiling = COMPILING, sep = SEP,
            dir = p.root().display(), url = p.url())));
});

test!(build_lib_and_allow_unstable_options {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustc").arg("--lib").arg("-v")
                .arg("--").arg("-Z").arg("unstable-options"),
                execs()
                .with_status(0)
                .with_stdout(format!("\
{compiling} foo v0.0.1 ({url})
{running} `rustc src{sep}lib.rs --crate-name foo --crate-type lib -g \
        -Z unstable-options \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, compiling = COMPILING, sep = SEP,
            dir = p.root().display(), url = p.url())))
});

test!(build_main_and_allow_unstable_options {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustc").arg("-v").arg("--bin").arg("foo")
                .arg("--").arg("-Z").arg("unstable-options"),
                execs()
                .with_status(0)
                .with_stdout(&format!("\
{compiling} {name} v{version} ({url})
{running} `rustc src{sep}lib.rs --crate-name {name} --crate-type lib -g \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
{running} `rustc src{sep}main.rs --crate-name {name} --crate-type bin -g \
        -Z unstable-options \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps \
        --extern {name}={dir}{sep}target{sep}debug{sep}lib{name}.rlib`
",
            running = RUNNING, compiling = COMPILING, sep = SEP,
            dir = p.root().display(), url = p.url(),
            name = "foo", version = "0.0.1")));
});

test!(fails_when_trying_to_build_main_and_lib_with_args {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustc").arg("-v")
                .arg("--").arg("-Z").arg("unstable-options"),
                execs()
                .with_status(101)
                .with_stderr("extra arguments to `rustc` can only be invoked for one target"));
});
