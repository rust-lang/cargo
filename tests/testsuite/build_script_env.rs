//! Tests for build.rs rerun-if-env-changed and rustc-env

use cargo_test_support::basic_manifest;
use cargo_test_support::prelude::*;
use cargo_test_support::project;
use cargo_test_support::sleep_ms;
use cargo_test_support::str;

#[cargo_test]
fn rerun_if_env_changes() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rerun-if-env-changed=FOO");
                }
            "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .env("FOO", "bar")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .env("FOO", "baz")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .env("FOO", "baz")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    println!("cargo::rerun-if-env-changed=FOO");
                    println!("cargo::rerun-if-changed=foo");
                }
            "#,
        )
        .file("foo", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .env("FOO", "bar")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .env("FOO", "bar")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    sleep_ms(1000);
    p.change_file("foo", "// modified");
    p.cargo("check")
        .env("FOO", "bar")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rustc_bootstrap() {
    let build_rs = r#"
        fn main() {
            println!("cargo::rustc-env=RUSTC_BOOTSTRAP=1");
        }
    "#;
    let p = project()
        .file("Cargo.toml", &basic_manifest("has-dashes", "0.0.1"))
        .file(
            "src/lib.rs",
            "#![allow(internal_features)] #![feature(rustc_attrs)]",
        )
        .file("build.rs", build_rs)
        .build();
    // RUSTC_BOOTSTRAP unset on stable should error
    p.cargo("check")
        .with_stderr_data(str![[r#"
[COMPILING] has-dashes v0.0.1 ([ROOT]/foo)
[ERROR] Cannot set `RUSTC_BOOTSTRAP=1` from build script of `has-dashes v0.0.1 ([ROOT]/foo)`.
[NOTE] Crates cannot set `RUSTC_BOOTSTRAP` themselves, as doing so would subvert the stability guarantees of Rust for your project.
[HELP] If you're sure you want to do this in your project, set the environment variable `RUSTC_BOOTSTRAP=has_dashes` before running cargo instead.

"#]])
        .with_status(101)
        .run();
    // nightly should warn whether or not RUSTC_BOOTSTRAP is set
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["RUSTC_BOOTSTRAP"])
        // NOTE: uses RUSTC_BOOTSTRAP so it will be propagated to rustc
        // (this matters when tests are being run with a beta or stable cargo)
        .env("RUSTC_BOOTSTRAP", "1")
        .with_stderr_data(str![[r#"
[COMPILING] has-dashes v0.0.1 ([ROOT]/foo)
[WARNING] has-dashes@0.0.1: Cannot set `RUSTC_BOOTSTRAP=1` from build script of `has-dashes v0.0.1 ([ROOT]/foo)`.
[NOTE] Crates cannot set `RUSTC_BOOTSTRAP` themselves, as doing so would subvert the stability guarantees of Rust for your project.
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // RUSTC_BOOTSTRAP set to the name of the library should warn
    p.cargo("check")
        .env("RUSTC_BOOTSTRAP", "has_dashes")
        .with_stderr_data(str![[r#"
[WARNING] has-dashes@0.0.1: Cannot set `RUSTC_BOOTSTRAP=1` from build script of `has-dashes v0.0.1 ([ROOT]/foo)`.
[NOTE] Crates cannot set `RUSTC_BOOTSTRAP` themselves, as doing so would subvert the stability guarantees of Rust for your project.
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // RUSTC_BOOTSTRAP set to some random value should error
    p.cargo("check")
        .env("RUSTC_BOOTSTRAP", "bar")
        .with_stderr_data(str![[r#"
[ERROR] Cannot set `RUSTC_BOOTSTRAP=1` from build script of `has-dashes v0.0.1 ([ROOT]/foo)`.
[NOTE] Crates cannot set `RUSTC_BOOTSTRAP` themselves, as doing so would subvert the stability guarantees of Rust for your project.
[HELP] If you're sure you want to do this in your project, set the environment variable `RUSTC_BOOTSTRAP=has_dashes` before running cargo instead.

"#]])
        .with_status(101)
        .run();

    // Tests for binaries instead of libraries
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.1"))
        .file(
            "src/main.rs",
            "#![allow(internal_features)] #![feature(rustc_attrs)] fn main() {}",
        )
        .file("build.rs", build_rs)
        .build();
    // nightly should warn when there's no library whether or not RUSTC_BOOTSTRAP is set
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["RUSTC_BOOTSTRAP"])
        // NOTE: uses RUSTC_BOOTSTRAP so it will be propagated to rustc
        // (this matters when tests are being run with a beta or stable cargo)
        .env("RUSTC_BOOTSTRAP", "1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[WARNING] foo@0.0.1: Cannot set `RUSTC_BOOTSTRAP=1` from build script of `foo v0.0.1 ([ROOT]/foo)`.
[NOTE] Crates cannot set `RUSTC_BOOTSTRAP` themselves, as doing so would subvert the stability guarantees of Rust for your project.
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // RUSTC_BOOTSTRAP conditionally set when there's no library should error (regardless of the value)
    p.cargo("check")
        .env("RUSTC_BOOTSTRAP", "foo")
        .with_stderr_data(str![[r#"
[ERROR] Cannot set `RUSTC_BOOTSTRAP=1` from build script of `foo v0.0.1 ([ROOT]/foo)`.
[NOTE] Crates cannot set `RUSTC_BOOTSTRAP` themselves, as doing so would subvert the stability guarantees of Rust for your project.
[HELP] If you're sure you want to do this in your project, set the environment variable `RUSTC_BOOTSTRAP=1` before running cargo instead.

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn build_script_env_verbose() {
    let build_rs = r#"
        fn main() {}
    "#;
    let p = project()
        .file("Cargo.toml", &basic_manifest("verbose-build", "0.0.1"))
        .file("src/lib.rs", "")
        .file("build.rs", build_rs)
        .build();

    p.cargo("check -vv")
        .with_stderr_data(
            "\
...
[RUNNING] `[..]CARGO=[..]build-script-build`
...",
        )
        .run();
}

#[cargo_test]
#[cfg(target_arch = "x86_64")]
fn build_script_sees_cfg_target_feature() {
    let build_rs = r#"
        fn main() {
            let cfg = std::env::var("CARGO_CFG_TARGET_FEATURE").unwrap();
            eprintln!("CARGO_CFG_TARGET_FEATURE={cfg}");
        }
    "#;

    let configs = [
        r#"
            [build]
            rustflags = ["-Ctarget-feature=+sse4.1,+sse4.2"]
        "#,
        r#"
            [target.'cfg(target_arch = "x86_64")']
            rustflags = ["-Ctarget-feature=+sse4.1,+sse4.2"]
        "#,
    ];

    for config in configs {
        let p = project()
            .file(".cargo/config.toml", config)
            .file("src/lib.rs", r#""#)
            .file("build.rs", build_rs)
            .build();

        p.cargo("check -vv")
            .with_stderr_data(
                "\
...
[foo 0.0.1] CARGO_CFG_TARGET_FEATURE=[..]sse4.2[..]
...
[..]-Ctarget-feature=[..]+sse4.2[..]
...",
            )
            .run();
    }
}

/// In this test, the cfg is self-contradictory. There's no *right* answer as to
/// what the value of `RUSTFLAGS` should be in this case. We chose to give a
/// warning. However, no matter what we do, it's important that build scripts
/// and rustc see a consistent picture
#[cargo_test]
fn cfg_paradox() {
    let build_rs = r#"
        fn main() {
            let cfg = std::env::var("CARGO_CFG_BERTRAND").is_ok();
            eprintln!("cfg!(bertrand)={cfg}");
        }
    "#;

    let config = r#"
        [target.'cfg(not(bertrand))']
        rustflags = ["--cfg=bertrand"]
    "#;

    let p = project()
        .file(".cargo/config.toml", config)
        .file("src/lib.rs", r#""#)
        .file("build.rs", build_rs)
        .build();

    p.cargo("check -vv")
        .with_stderr_data(
            "\
[WARNING] non-trivial mutual dependency between target-specific configuration and RUSTFLAGS
...
[foo 0.0.1] cfg!(bertrand)=true
...
[..]--cfg=bertrand[..]
...",
        )
        .run();
}

/// This test checks how Cargo handles rustc cfgs which are defined both with
/// and without a value. The expected behavior is that the environment variable
/// is going to contain all the values.
///
/// For example, this configuration:
/// ```
/// target_has_atomic
/// target_has_atomic="16"
/// target_has_atomic="32"
/// target_has_atomic="64"
/// target_has_atomic="8"
/// target_has_atomic="ptr"
/// ```
///
/// Should result in the following environment variable:
///
/// ```
/// CARGO_CFG_TARGET_HAS_ATOMIC=16,32,64,8,ptr
/// ```
///
/// On the other hand, configuration symbols without any value should result in
/// an empty string.
///
/// For example, this configuration:
///
/// ```
/// target_thread_local
/// ```
///
/// Should result in the following environment variable:
///
/// ```
/// CARGO_CFG_TARGET_THREAD_LOCAL=
/// ```
#[cargo_test(nightly, reason = "affected rustc cfg is unstable")]
#[cfg(target_arch = "x86_64")]
fn rustc_cfg_with_and_without_value() {
    let build_rs = r#"
        fn main() {
            let cfg = std::env::var("CARGO_CFG_TARGET_HAS_ATOMIC");
            eprintln!("CARGO_CFG_TARGET_HAS_ATOMIC={cfg:?}");
            let cfg = std::env::var("CARGO_CFG_WINDOWS");
            eprintln!("CARGO_CFG_WINDOWS={cfg:?}");
            let cfg = std::env::var("CARGO_CFG_UNIX");
            eprintln!("CARGO_CFG_UNIX={cfg:?}");
        }
    "#;
    let p = project()
        .file("src/lib.rs", r#""#)
        .file("build.rs", build_rs)
        .build();

    let mut check = p.cargo("check -vv");
    #[cfg(target_has_atomic = "64")]
    check.with_stderr_data(
        "\
...
[foo 0.0.1] CARGO_CFG_TARGET_HAS_ATOMIC=Ok(\"[..]64[..]\")
...",
    );
    #[cfg(windows)]
    check.with_stderr_data(
        "\
...
[foo 0.0.1] CARGO_CFG_WINDOWS=Ok(\"\")
...",
    );
    #[cfg(unix)]
    check.with_stderr_data(
        "\
...
[foo 0.0.1] CARGO_CFG_UNIX=Ok(\"\")
...",
    );
    check.run();
}

#[cargo_test]
fn rerun_if_env_is_exsited_config() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo::rerun-if-env-changed=FOO");
            }
        "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
            [env]
            FOO = "foo"
            "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo(r#"check --config 'env.FOO="bar"'"#)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rerun_if_env_newly_added_in_config() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo::rerun-if-env-changed=FOO");
            }
        "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo(r#"check --config 'env.FOO="foo"'"#)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
