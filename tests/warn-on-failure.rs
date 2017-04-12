extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{project, execs, ProjectBuilder};
use cargotest::support::registry::Package;
use hamcrest::assert_that;

static WARNING1: &'static str = "Hello! I'm a warning. :)";
static WARNING2: &'static str = "And one more!";

fn make_lib(lib_src: &str) {
    Package::new("foo", "0.0.1")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
            build = "build.rs"
        "#)
        .file("build.rs", &format!(r#"
            fn main() {{
                use std::io::Write;
                println!("cargo:warning={{}}", "{}");
                println!("hidden stdout");
                write!(&mut ::std::io::stderr(), "hidden stderr");
                println!("cargo:warning={{}}", "{}");
            }}
        "#, WARNING1, WARNING2))
        .file("src/lib.rs", &format!("fn f() {{ {} }}", lib_src))
        .publish();
}

fn make_upstream(main_src: &str) -> ProjectBuilder {
    project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "*"
        "#)
        .file("src/main.rs", &format!("fn main() {{ {} }}", main_src))
}

#[test]
fn no_warning_on_success() {
    make_lib("");
    let upstream = make_upstream("");
    assert_that(upstream.cargo_process("build"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.1 ([..])
[COMPILING] foo v0.0.1
[COMPILING] bar v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn no_warning_on_bin_failure() {
    make_lib("");
    let upstream = make_upstream("hi()");
    assert_that(upstream.cargo_process("build"),
                execs().with_status(101)
                       .with_stdout_does_not_contain("hidden stdout")
                       .with_stderr_does_not_contain("hidden stderr")
                       .with_stderr_does_not_contain(&format!("[WARNING] {}", WARNING1))
                       .with_stderr_does_not_contain(&format!("[WARNING] {}", WARNING2))
                       .with_stderr_contains("[UPDATING] registry `[..]`")
                       .with_stderr_contains("[DOWNLOADING] foo v0.0.1 ([..])")
                       .with_stderr_contains("[COMPILING] foo v0.0.1")
                       .with_stderr_contains("[COMPILING] bar v0.0.1 ([..])"));
}

#[test]
fn warning_on_lib_failure() {
    make_lib("err()");
    let upstream = make_upstream("");
    assert_that(upstream.cargo_process("build"),
                execs().with_status(101)
                       .with_stdout_does_not_contain("hidden stdout")
                       .with_stderr_does_not_contain("hidden stderr")
                       .with_stderr_does_not_contain("[COMPILING] bar v0.0.1 ([..])")
                       .with_stderr_contains("[UPDATING] registry `[..]`")
                       .with_stderr_contains("[DOWNLOADING] foo v0.0.1 ([..])")
                       .with_stderr_contains("[COMPILING] foo v0.0.1")
                       .with_stderr_contains(&format!("[WARNING] {}", WARNING1))
                       .with_stderr_contains(&format!("[WARNING] {}", WARNING2)));
}
