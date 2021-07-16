//! Tests for build.rs rerun-if-env-changed and rustc-env

use cargo_test_support::basic_manifest;
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

#[ignore]
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
    let build_rs = r#"
        fn main() {
            println!("cargo:rustc-env=RUSTC_BOOTSTRAP=1");
        }
    "#;
    let p = project()
        .file("Cargo.toml", &basic_manifest("has-dashes", "0.0.1"))
        .file("src/lib.rs", "#![feature(rustc_attrs)]")
        .file("build.rs", build_rs)
        .build();
    // RUSTC_BOOTSTRAP unset on stable should error
    p.cargo("build")
        .with_stderr_contains("error: Cannot set `RUSTC_BOOTSTRAP=1` [..]")
        .with_stderr_contains(
            "help: [..] set the environment variable `RUSTC_BOOTSTRAP=has_dashes` [..]",
        )
        .with_status(101)
        .run();
    // nightly should warn whether or not RUSTC_BOOTSTRAP is set
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        // NOTE: uses RUSTC_BOOTSTRAP so it will be propagated to rustc
        // (this matters when tests are being run with a beta or stable cargo)
        .env("RUSTC_BOOTSTRAP", "1")
        .with_stderr_contains("warning: Cannot set `RUSTC_BOOTSTRAP=1` [..]")
        .run();
    // RUSTC_BOOTSTRAP set to the name of the library should warn
    p.cargo("build")
        .env("RUSTC_BOOTSTRAP", "has_dashes")
        .with_stderr_contains("warning: Cannot set `RUSTC_BOOTSTRAP=1` [..]")
        .run();
    // RUSTC_BOOTSTRAP set to some random value should error
    p.cargo("build")
        .env("RUSTC_BOOTSTRAP", "bar")
        .with_stderr_contains("error: Cannot set `RUSTC_BOOTSTRAP=1` [..]")
        .with_stderr_contains(
            "help: [..] set the environment variable `RUSTC_BOOTSTRAP=has_dashes` [..]",
        )
        .with_status(101)
        .run();

    // Tests for binaries instead of libraries
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.1"))
        .file("src/main.rs", "#![feature(rustc_attrs)] fn main() {}")
        .file("build.rs", build_rs)
        .build();
    // nightly should warn when there's no library whether or not RUSTC_BOOTSTRAP is set
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        // NOTE: uses RUSTC_BOOTSTRAP so it will be propagated to rustc
        // (this matters when tests are being run with a beta or stable cargo)
        .env("RUSTC_BOOTSTRAP", "1")
        .with_stderr_contains("warning: Cannot set `RUSTC_BOOTSTRAP=1` [..]")
        .run();
    // RUSTC_BOOTSTRAP conditionally set when there's no library should error (regardless of the value)
    p.cargo("build")
        .env("RUSTC_BOOTSTRAP", "foo")
        .with_stderr_contains("error: Cannot set `RUSTC_BOOTSTRAP=1` [..]")
        .with_stderr_contains("help: [..] set the environment variable `RUSTC_BOOTSTRAP=1` [..]")
        .with_status(101)
        .run();
}
