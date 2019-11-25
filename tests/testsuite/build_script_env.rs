//! Tests for build.rs rerun-if-env-changed.

use std::fs::File;

use cargo_test_support::project;
use cargo_test_support::sleep_ms;

#[cargo_test]
fn rerun_if_env_changes() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-env-changed=FOO");
            }
        "#,
        )
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build")
        .env("FOO", "bar")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build")
        .env("FOO", "baz")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build")
        .env("FOO", "baz")
        .with_stderr("[FINISHED] [..]")
        .run();
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn rerun_if_env_or_file_changes() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-env-changed=FOO");
                println!("cargo:rerun-if-changed=foo");
            }
        "#,
        )
        .file("foo", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build")
        .env("FOO", "bar")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build")
        .env("FOO", "bar")
        .with_stderr("[FINISHED] [..]")
        .run();
    sleep_ms(1000);
    File::create(p.root().join("foo")).unwrap();
    p.cargo("build")
        .env("FOO", "bar")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
}
