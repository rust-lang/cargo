use crate::support::{basic_lib_manifest, project};

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
[ERROR] failed to parse manifest at [..]

Caused by:
  An 'inherits' directive is needed for all profiles that are not 'dev' or 'release'. Here it is missing from 'release-lto'",
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
[ERROR] failed to parse manifest at [..]

Caused by:
  Profile 'non-existent' not found in Cargo.toml",
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
[ERROR] failed to parse manifest at [..]

Caused by:
  Inheritance loop of profiles cycles with profile 'release-lto'",
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
[ERROR] failed to parse manifest at [..]

Caused by:
  Inheritance loop of profiles cycles with profile 'release-lto'",
        )
        .run();
}

#[cargo_test]
fn overrides_with_custom() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["profile-overrides", "named-profiles"]

            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            xxx = {path = "xxx"}
            yyy = {path = "yyy"}

            [profile.dev]
            codegen-units = 7

            [profile.dev.overrides.xxx]
            codegen-units = 5
            [profile.dev.overrides.yyy]
            codegen-units = 3

            [profile.other]
            inherits = "dev"
            codegen-units = 2

            [profile.other.overrides.yyy]
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
