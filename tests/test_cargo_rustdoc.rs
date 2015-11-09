use std::path::MAIN_SEPARATOR as SEP;
use support::{execs, project};
use support::{COMPILING, RUNNING, DOCUMENTING};
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
{documenting} foo v0.0.1 ({url})
{running} `rustdoc src{sep}lib.rs --crate-name foo \
        -o {dir}{sep}target{sep}doc \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, documenting = DOCUMENTING, sep = SEP,
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
{documenting} foo v0.0.1 ({url})
{running} `rustdoc src{sep}lib.rs --crate-name foo \
        -o {dir}{sep}target{sep}doc \
        --no-defaults \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, documenting = DOCUMENTING, sep = SEP,
            dir = p.root().display(), url = p.url())));
});



test!(rustdoc_foo_with_bar_dependency {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", r#"
            extern crate bar;
            pub fn foo() {}
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("rustdoc").arg("-v").arg("--").arg("--no-defaults"),
                execs()
                .with_status(0)
                .with_stdout(format!("\
{compiling} bar v0.0.1 ({url})
{running} `rustc {bar_dir}{sep}src{sep}lib.rs [..]`
{documenting} foo v0.0.1 ({url})
{running} `rustdoc src{sep}lib.rs --crate-name foo \
        -o {dir}{sep}target{sep}doc \
        --no-defaults \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps \
        --extern [..]`
",
            running = RUNNING, compiling = COMPILING, sep = SEP,
            documenting = DOCUMENTING,
            dir = foo.root().display(), url = foo.url(),
            bar_dir = bar.root().display())));
});

test!(rustdoc_only_bar_dependency {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {
                bar::baz()
            }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("rustdoc").arg("-v").arg("-p").arg("bar")
                                            .arg("--").arg("--no-defaults"),
                execs()
                .with_status(0)
                .with_stdout(format!("\
{documenting} bar v0.0.1 ({url})
{running} `rustdoc {bar_dir}{sep}src{sep}lib.rs --crate-name bar \
        -o {dir}{sep}target{sep}doc \
        --no-defaults \
        -L dependency={dir}{sep}target{sep}debug{sep}deps \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
            running = RUNNING, documenting = DOCUMENTING, sep = SEP,
            dir = foo.root().display(), url = foo.url(),
            bar_dir = bar.root().display())));
});


test!(rustdoc_same_name_err {
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

    assert_that(p.cargo_process("rustdoc").arg("-v")
                 .arg("--").arg("--no-defaults"),
                execs()
                .with_status(101)
                .with_stderr("Cannot document a package where a library and a \
                              binary have the same name. Consider renaming one \
                              or marking the target as `doc = false`"));
});
