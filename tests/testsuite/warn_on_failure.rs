//! Tests for whether or not warnings are displayed for build scripts.

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{Project, project, str};

static WARNING1: &str = "Hello! I'm a warning. :)";
static WARNING2: &str = "And one more!";

fn make_lib(lib_src: &str) {
    Package::new("bar", "0.0.1")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                authors = []
                version = "0.0.1"
                edition = "2015"
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
                    fn main() {{
                        use std::io::Write;
                        println!("cargo::warning={{}}", "{}");
                        println!("hidden stdout");
                        write!(&mut ::std::io::stderr(), "hidden stderr");
                        println!("cargo::warning={{}}", "{}");
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
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", &format!("fn main() {{ {} }}", main_src))
        .build()
}

#[cargo_test]
fn no_warning_on_success() {
    make_lib("");
    let upstream = make_upstream("");
    upstream
        .cargo("build")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn no_warning_on_bin_failure() {
    make_lib("");
    let upstream = make_upstream("hi()");
    upstream
        .cargo("build")
        .with_status(101)
        .with_stdout_does_not_contain("hidden stdout")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([ROOT]/foo)
error[E0425]: cannot find function `hi` in this scope
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
        .run();
}

#[cargo_test]
fn warning_on_lib_failure() {
    make_lib("err()");
    let upstream = make_upstream("");
    upstream
        .cargo("build")
        .with_status(101)
        .with_stdout_does_not_contain("hidden stdout")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
error[E0425]: cannot find function `err` in this scope
...
[WARNING] bar@0.0.1: Hello! I'm a warning. :)
[WARNING] bar@0.0.1: And one more!
[ERROR] could not compile `bar` (lib) due to 1 previous error

"#]])
        .run();
}
