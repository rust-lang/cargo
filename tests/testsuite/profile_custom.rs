//! Tests for named profiles.

use crate::prelude::*;
use cargo_test_support::{basic_lib_manifest, project, str};

#[cargo_test]
fn inherits_on_release() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [profile.release]
                inherits = "dev"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `inherits` must not be specified in root profile `release`

"#]])
        .run();
}

#[cargo_test]
fn missing_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [profile.release-lto]
                codegen-units = 7
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] profile `release-lto` is missing an `inherits` directive (`inherits` is required for all profiles except `dev` or `release`)

"#]])
        .run();
}

#[cargo_test]
fn invalid_profile_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [profile.'.release-lto']
                inherits = "release"
                codegen-units = 7
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character `.` in profile name: `.release-lto`, allowed characters are letters, numbers, underscore, and hyphen
 --> Cargo.toml:8:26
  |
8 |                 [profile.'.release-lto']
  |                          ^^^^^^^^^^^^^^

"#]])
        .run();
}

#[cargo_test]
// We are currently uncertain if dir-name will ever be exposed to the user.
// The code for it still roughly exists, but only for the internal profiles.
// This test was kept in case we ever want to enable support for it again.
#[ignore = "dir-name is disabled"]
fn invalid_dir_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  Invalid character `.` in dir-name: `.subdir`",

"#]])
        .run();
}

#[cargo_test]
fn dir_name_disabled() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [profile.release-lto]
                inherits = "release"
                dir-name = "lto"
                lto = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dir-name="lto" in profile `release-lto` is not currently allowed, directory names are tied to the profile name for custom profiles

"#]])
        .run();
}

#[cargo_test]
fn invalid_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [profile.'release-lto']
                inherits = ".release"
                codegen-units = 7
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] profile `release-lto` inherits from `.release`, but that profile is not defined

"#]])
        .run();
}

#[cargo_test]
fn non_existent_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [profile.release-lto]
                codegen-units = 7
                inherits = "non-existent"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] profile `release-lto` inherits from `non-existent`, but that profile is not defined

"#]])
        .run();
}

#[cargo_test]
fn self_inherits() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [profile.release-lto]
                codegen-units = 7
                inherits = "release-lto"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] profile inheritance loop detected with profile `release-lto` inheriting `release-lto`

"#]])
        .run();
}

#[cargo_test]
fn inherits_loop() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] profile inheritance loop detected with profile `release-lto2` inheriting `release-lto`

"#]])
        .run();
}

#[cargo_test]
fn overrides_with_custom() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
        .with_stderr_data(
            str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] xxx v0.5.0 ([ROOT]/foo/xxx)
[COMPILING] yyy v0.5.0 ([ROOT]/foo/yyy)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name xxx [..] -C codegen-units=5 [..]`
[RUNNING] `rustc --crate-name yyy [..] -C codegen-units=3 [..]`
[RUNNING] `rustc --crate-name foo [..] -C codegen-units=7 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    // This also verifies that the custom profile names appears in the finished line.
    p.cargo("build --profile=other -v")
        .with_stderr_data(
            str![[r#"
[COMPILING] xxx v0.5.0 ([ROOT]/foo/xxx)
[COMPILING] yyy v0.5.0 ([ROOT]/foo/yyy)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name xxx [..] -C codegen-units=5 [..]`
[RUNNING] `rustc --crate-name yyy [..] -C codegen-units=6 [..]`
[RUNNING] `rustc --crate-name foo [..] -C codegen-units=2 [..]`
[FINISHED] `other` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
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
                edition = "2015"
                authors = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --profile=dev --release")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--profile <PROFILE-NAME>' cannot be used with '--release'

Usage: cargo[EXE] build --profile <PROFILE-NAME>

For more information, try '--help'.

"#]])
        .run();

    p.cargo("install --profile=release --debug")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--profile <PROFILE-NAME>' cannot be used with '--debug'

Usage: cargo[EXE] install --profile <PROFILE-NAME> [CRATE[@<VER>]]...

For more information, try '--help'.

"#]])
        .run();

    p.cargo("check --profile=dev --release")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--profile <PROFILE-NAME>' cannot be used with '--release'

Usage: cargo[EXE] check --profile <PROFILE-NAME>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn clean_custom_dirname() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [profile.other]
                inherits = "release"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --release")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean -p foo").run();

    p.cargo("build --release")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean -p foo --release").run();

    p.cargo("build --release")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build --profile=other")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `other` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean").arg("--release").run();

    // Make sure that 'other' was not cleaned
    assert!(p.build_dir().is_dir());
    assert!(p.build_dir().join("debug").is_dir());
    assert!(p.build_dir().join("other").is_dir());
    assert!(!p.build_dir().join("release").is_dir());

    // This should clean 'other'
    p.cargo("clean --profile=other")
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
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
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --profile alpha")
        .with_stderr_data(str![[r#"
[ERROR] profile `alpha` is not defined

"#]])
        .with_status(101)
        .run();
    // Clean has a separate code path, need to check it too.
    p.cargo("clean --profile alpha")
        .with_stderr_data(str![[r#"
[ERROR] profile `alpha` is not defined

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn reserved_profile_names() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [profile.doc]
                opt-level = 1
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --profile=doc")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] profile `doc` is reserved and not allowed to be explicitly specified

"#]])
        .run();
    // Not an exhaustive list, just a sample.
    for name in ["build", "cargo", "check", "rustc", "CaRgO_startswith"] {
        p.cargo(&format!("build --profile={}", name))
            .with_status(101)
            .with_stderr_data(&format!(
                "\
[ERROR] profile name `{}` is reserved
Please choose a different name.
See https://doc.rust-lang.org/cargo/reference/profiles.html for more on configuring profiles.
",
                name
            ))
            .run();
    }
    for name in ["build", "check", "cargo", "rustc", "CaRgO_startswith"] {
        p.change_file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"
                    edition = "2015"

                    [profile.{}]
                    opt-level = 1
                "#,
                name
            ),
        );

        let highlight = "^".repeat(name.len());
        p.cargo("build")
            .with_status(101)
            .with_stderr_data(&format!(
                "\
[ERROR] profile name `{name}` is reserved
       Please choose a different name.
       See https://doc.rust-lang.org/cargo/reference/profiles.html for more on configuring profiles.
 --> Cargo.toml:7:30
  |
7 |                     [profile.{name}]
  |                              {highlight}
"
            ))
            .run();
    }

    p.change_file(
        "Cargo.toml",
        r#"
               [package]
               name = "foo"
               version = "0.1.0"
               edition = "2015"
               authors = []

               [profile.debug]
               debug = 1
               inherits = "dev"
            "#,
    );

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] profile name `debug` is reserved
       To configure the default development profile, use the name `dev` as in [profile.dev]
       See https://doc.rust-lang.org/cargo/reference/profiles.html for more on configuring profiles.
 --> Cargo.toml:8:25
  |
8 |                [profile.debug]
  |                         ^^^^^

"#]])
        .run();
}

#[cargo_test]
fn legacy_commands_support_custom() {
    // These commands have had `--profile` before custom named profiles.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
               [package]
               name = "foo"
               version = "0.1.0"
               edition = "2015"

               [profile.super-dev]
               codegen-units = 3
               inherits = "dev"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    for command in ["rustc", "fix", "check"] {
        let mut pb = p.cargo(command);
        if command == "fix" {
            pb.arg("--allow-no-vcs");
        }
        pb.arg("--profile=super-dev")
            .arg("-v")
            .with_stderr_data(str![
                r#"
...
[RUNNING] [..]codegen-units=3[..]
...
"#
            ])
            .run();
        p.build_dir().rm_rf();
    }
}

#[cargo_test]
fn legacy_rustc() {
    // `cargo rustc` historically has supported dev/test/bench/check
    // other profiles are covered in check::rustc_check
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [profile.dev]
                codegen-units = 3
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("rustc --profile dev -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]-C codegen-units=3[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
