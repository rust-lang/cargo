//! Tests for named profiles.

use cargo_test_support::{basic_lib_manifest, project};

#[cargo_test]
fn inherits_on_release() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.release]
                inherits = "dev"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] `inherits` must not be specified in root profile `release`
",
        )
        .run();
}

#[cargo_test]
fn missing_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.release-lto]
                codegen-units = 7
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] profile `release-lto` is missing an `inherits` directive \
    (`inherits` is required for all profiles except `dev` or `release`)
",
        )
        .run();
}

#[cargo_test]
fn invalid_profile_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.'.release-lto']
                inherits = "release"
                codegen-units = 7
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at [..]

Caused by:
  Invalid character `.` in profile name: `.release-lto`",
        )
        .run();
}

#[cargo_test]
fn invalid_dir_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.'release-lto']
                inherits = "release"
                dir-name = ".subdir"
                codegen-units = 7
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at [..]

Caused by:
  Invalid character `.` in dir-name: `.subdir`",
        )
        .run();
}

#[cargo_test]
fn invalid_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.'release-lto']
                inherits = ".release"
                codegen-units = 7
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at [..]

Caused by:
  Invalid character `.` in inherits: `.release`",
        )
        .run();
}

#[cargo_test]
fn non_existent_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.release-lto]
                codegen-units = 7
                inherits = "non-existent"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] profile `release-lto` inherits from `non-existent`, but that profile is not defined
",
        )
        .run();
}

#[cargo_test]
fn self_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.release-lto]
                codegen-units = 7
                inherits = "release-lto"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] profile inheritance loop detected with profile `release-lto` inheriting `release-lto`
",
        )
        .run();
}

#[cargo_test]
fn inherits_loop() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.release-lto]
                codegen-units = 7
                inherits = "release-lto2"

                [profile.release-lto2]
                codegen-units = 7
                inherits = "release-lto"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] profile inheritance loop detected with profile `release-lto2` inheriting `release-lto`
",
        )
        .run();
}

#[cargo_test]
fn overrides_with_custom() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                xxx = {path = "xxx"}
                yyy = {path = "yyy"}

                [profile.dev]
                codegen-units = 7

                [profile.dev.package.xxx]
                codegen-units = 5
                [profile.dev.package.yyy]
                codegen-units = 3

                [profile.other]
                inherits = "dev"
                codegen-units = 2

                [profile.other.package.yyy]
                codegen-units = 6
            "#,
        )
        .file("src/lib.rs", "")
        .file("xxx/Cargo.toml", &basic_lib_manifest("xxx"))
        .file("xxx/src/lib.rs", "")
        .file("yyy/Cargo.toml", &basic_lib_manifest("yyy"))
        .file("yyy/src/lib.rs", "")
        .build();

    // profile overrides are inherited between profiles using inherits and have a
    // higher priority than profile options provided by custom profiles
    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_unordered(
            "\
[COMPILING] xxx [..]
[COMPILING] yyy [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name xxx [..] -C codegen-units=5 [..]`
[RUNNING] `rustc --crate-name yyy [..] -C codegen-units=3 [..]`
[RUNNING] `rustc --crate-name foo [..] -C codegen-units=7 [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // This also verifies that the custom profile names appears in the finished line.
    p.cargo("build --profile=other -Z unstable-options -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_unordered(
            "\
[COMPILING] xxx [..]
[COMPILING] yyy [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name xxx [..] -C codegen-units=5 [..]`
[RUNNING] `rustc --crate-name yyy [..] -C codegen-units=6 [..]`
[RUNNING] `rustc --crate-name foo [..] -C codegen-units=2 [..]`
[FINISHED] other [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn conflicting_usage() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -Z unstable-options --profile=dev --release")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_unordered("error: Conflicting usage of --profile and --release")
        .run();

    p.cargo("install -Z unstable-options --profile=release --debug")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_unordered("error: Conflicting usage of --profile and --debug")
        .run();
}

#[ignore]
#[cargo_test]
fn clean_custom_dirname() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["named-profiles"]

                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.other]
                inherits = "release"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --release")
        .masquerade_as_nightly_cargo()
        .with_stdout("")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
",
        )
        .run();

    p.cargo("clean -p foo").masquerade_as_nightly_cargo().run();

    p.cargo("build --release")
        .masquerade_as_nightly_cargo()
        .with_stdout("")
        .with_stderr(
            "\
[FINISHED] release [optimized] target(s) in [..]
",
        )
        .run();

    p.cargo("clean -p foo --release")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("build --release")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] release [optimized] target(s) in [..]
",
        )
        .run();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stdout("")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build -Z unstable-options --profile=other")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] other [optimized] target(s) in [..]
",
        )
        .run();

    p.cargo("clean")
        .arg("--release")
        .masquerade_as_nightly_cargo()
        .run();

    // Make sure that 'other' was not cleaned
    assert!(p.build_dir().is_dir());
    assert!(p.build_dir().join("debug").is_dir());
    assert!(p.build_dir().join("other").is_dir());
    assert!(!p.build_dir().join("release").is_dir());

    // This should clean 'other'
    p.cargo("clean -Z unstable-options --profile=other")
        .masquerade_as_nightly_cargo()
        .with_stderr("")
        .run();
    assert!(p.build_dir().join("debug").is_dir());
    assert!(!p.build_dir().join("other").is_dir());
}

#[cargo_test]
fn unknown_profile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["named-profiles"]

            [package]
            name = "foo"
            version = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --profile alpha -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_stderr("[ERROR] profile `alpha` is not defined")
        .with_status(101)
        .run();
    // Clean has a separate code path, need to check it too.
    p.cargo("clean --profile alpha -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_stderr("[ERROR] profile `alpha` is not defined")
        .with_status(101)
        .run();
}
