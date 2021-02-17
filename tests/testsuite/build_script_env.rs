//! Tests for build.rs rerun-if-env-changed.

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
    p.change_file("foo", "");
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

#[cargo_test]
fn rustc_bootstrap() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rustc-env=RUSTC_BOOTSTRAP=1");
            }
        "#,
        )
        .build();
    p.cargo("build")
        .with_stderr_contains("error: Cannot set `RUSTC_BOOTSTRAP=1` [..]")
        .with_stderr_contains("help: [..] use `RUSTC_BOOTSTRAP=1 cargo build` [..]")
        .with_status(101)
        .run();
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("warning: Cannot set `RUSTC_BOOTSTRAP=1` [..]")
        .run();
}
