use std::path::MAIN_SEPARATOR as SEP;
use support::{execs, project, ProjectBuilder};
use support::{COMPILING, RUNNING};
use hamcrest::{assert_that};

fn setup() {
}

fn verbose_output_for_target(lib: bool, p: &ProjectBuilder) -> String {
    let (target, kind) = match lib {
        true => ("lib", "lib"),
        false => ("main", "bin"),
    };
    format!("\
{compiling} {name} v{version} ({url})
{running} `rustc src{sep}{target}.rs --crate-name {name} --crate-type {kind} -g \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, compiling = COMPILING, sep = SEP,
            dir = p.root().display(), url = p.url(),
            target = target, kind = kind,
            name = "foo", version = "0.0.1")
}

fn verbose_output_for_target_with_args(lib: bool, p: &ProjectBuilder, args: &str) -> String {
    verbose_output_for_target(lib, p).replace(" -g ", &format!(" -g {} ", args))
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

    assert_that(p.cargo_process("rustc").arg("--lib").arg("-v"),
                execs()
                .with_status(0)
                .with_stdout(verbose_output_for_target(true, &p)));
});

test!(build_lib_and_allow_unstable_options {
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

    assert_that(p.cargo_process("rustc").arg("--lib").arg("-v")
                .arg("--").arg("-Z").arg("unstable-options"),
                execs()
                .with_status(0)
                .with_stdout(verbose_output_for_target_with_args(true, &p,
                                                                 "-Z unstable-options")));
});

test!(build_main_and_allow_unstable_options {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]

            name = "foo"
            version = "0.0.1"
            authors = ["wycats@example.com"]
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("rustc").arg("-v")
                .arg("--").arg("-Z").arg("unstable-options"),
                execs()
                .with_status(0)
                .with_stdout(verbose_output_for_target_with_args(false, &p,
                                                                 "-Z unstable-options")));
});
