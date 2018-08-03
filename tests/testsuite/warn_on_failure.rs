use support::{execs, project, Project};
use support::registry::Package;
use support::hamcrest::assert_that;

static WARNING1: &'static str = "Hello! I'm a warning. :)";
static WARNING2: &'static str = "And one more!";

fn make_lib(lib_src: &str) {
    Package::new("bar", "0.0.1")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            authors = []
            version = "0.0.1"
            build = "build.rs"
        "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
            fn main() {{
                use std::io::Write;
                println!("cargo:warning={{}}", "{}");
                println!("hidden stdout");
                write!(&mut ::std::io::stderr(), "hidden stderr");
                println!("cargo:warning={{}}", "{}");
            }}
        "#,
                WARNING1, WARNING2
            ),
        )
        .file("src/lib.rs", &format!("fn f() {{ {} }}", lib_src))
        .publish();
}

fn make_upstream(main_src: &str) -> Project {
    project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#,
        )
        .file("src/main.rs", &format!("fn main() {{ {} }}", main_src))
        .build()
}

#[test]
fn no_warning_on_success() {
    make_lib("");
    let upstream = make_upstream("");
    assert_that(
        upstream.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] bar v0.0.1 ([..])
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn no_warning_on_bin_failure() {
    make_lib("");
    let upstream = make_upstream("hi()");
    assert_that(
        upstream.cargo("build"),
        execs()
            .with_status(101)
            .with_stdout_does_not_contain("hidden stdout")
            .with_stderr_does_not_contain("hidden stderr")
            .with_stderr_does_not_contain(&format!("[WARNING] {}", WARNING1))
            .with_stderr_does_not_contain(&format!("[WARNING] {}", WARNING2))
            .with_stderr_contains("[UPDATING] registry `[..]`")
            .with_stderr_contains("[DOWNLOADING] bar v0.0.1 ([..])")
            .with_stderr_contains("[COMPILING] bar v0.0.1")
            .with_stderr_contains("[COMPILING] foo v0.0.1 ([..])"),
    );
}

#[test]
fn warning_on_lib_failure() {
    make_lib("err()");
    let upstream = make_upstream("");
    assert_that(
        upstream.cargo("build"),
        execs()
            .with_status(101)
            .with_stdout_does_not_contain("hidden stdout")
            .with_stderr_does_not_contain("hidden stderr")
            .with_stderr_does_not_contain("[COMPILING] foo v0.0.1 ([..])")
            .with_stderr_contains("[UPDATING] registry `[..]`")
            .with_stderr_contains("[DOWNLOADING] bar v0.0.1 ([..])")
            .with_stderr_contains("[COMPILING] bar v0.0.1")
            .with_stderr_contains(&format!("[WARNING] {}", WARNING1))
            .with_stderr_contains(&format!("[WARNING] {}", WARNING2)),
    );
}
