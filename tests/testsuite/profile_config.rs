//! Tests for profiles defined in config files.

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, paths, project, str};
use cargo_util_schemas::manifest::TomlDebugInfo;

// TODO: this should be remove once -Zprofile-rustflags is stabilized
#[cargo_test]
fn rustflags_works_with_zflag() {
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
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev]
                rustflags = ["-C", "link-dead-code=yes"]
            "#,
        )
        .build();

    p.cargo("check -v")
        .masquerade_as_nightly_cargo(&["profile-rustflags"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] config profile `dev` is not valid (defined in `[ROOT]/foo/.cargo/config.toml`)

Caused by:
  feature `profile-rustflags` is required
...
"#]])
        .run();

    p.cargo("check -v -Zprofile-rustflags")
        .masquerade_as_nightly_cargo(&["profile-rustflags"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] -C link-dead-code=yes [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.change_file(
        ".cargo/config.toml",
        r#"
            [unstable]
            profile-rustflags = true

            [profile.dev]
            rustflags = ["-C", "link-dead-code=yes"]
        "#,
    );

    p.cargo("check -v")
        .masquerade_as_nightly_cargo(&["profile-rustflags"])
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn profile_config_validate_warnings() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [profile.test]
                opt-level = 3

                [profile.asdf]
                opt-level = 3

                [profile.dev]
                bad-key = true

                [profile.dev.build-override]
                bad-key-bo = true

                [profile.dev.package.bar]
                bad-key-bar = true
            "#,
        )
        .build();

    p.cargo("build").with_stderr_data(str![[r#"
[WARNING] unused config key `profile.dev.bad-key` in `[ROOT]/foo/.cargo/config.toml`
[WARNING] unused config key `profile.dev.build-override.bad-key-bo` in `[ROOT]/foo/.cargo/config.toml`
[WARNING] unused config key `profile.dev.package.bar.bad-key-bar` in `[ROOT]/foo/.cargo/config.toml`
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered()).run();
}

#[cargo_test]
fn profile_config_error_paths() {
    // Errors in config show where the error is located.
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev]
                opt-level = 3
            "#,
        )
        .file(
            paths::home().join(".cargo/config.toml"),
            r#"
            [profile.dev]
            rpath = "foo"
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `profile.dev`

Caused by:
  error in [ROOT]/home/.cargo/config.toml: `profile.dev.rpath` expected true/false, but found a string

"#]])
        .run();
}

#[cargo_test]
fn profile_config_validate_errors() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev.package.foo]
                panic = "abort"
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] config profile `dev` is not valid (defined in `[ROOT]/foo/.cargo/config.toml`)

Caused by:
  `panic` may not be specified in a `package` profile

"#]])
        .run();
}

#[cargo_test]
fn profile_config_syntax_errors() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev]
                codegen-units = "foo"
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `profile.dev`

Caused by:
  error in [ROOT]/foo/.cargo/config.toml: `profile.dev.codegen-units` expected an integer, but found a string

"#]])
        .run();
}

#[cargo_test]
fn profile_config_override_spec_multiple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [dependencies]
            bar = { path = "bar" }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev.package.bar]
                opt-level = 3

                [profile.dev.package."bar:0.5.0"]
                opt-level = 3
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    // Unfortunately this doesn't tell you which file, hopefully it's not too
    // much of a problem.
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ERROR] multiple package overrides in profile `dev` match package `bar v0.5.0 ([ROOT]/foo/bar)`
found package specs: bar, bar@0.5.0

"#]])
        .run();
}

#[cargo_test]
fn profile_config_all_options() {
    // Ensure all profile options are supported.
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
            [profile.release]
            opt-level = 1
            debug = true
            debug-assertions = true
            overflow-checks = false
            rpath = true
            lto = true
            codegen-units = 2
            panic = "abort"
            incremental = true
            "#,
        )
        .build();

    p.cargo("build --release -v")
        .env_remove("CARGO_INCREMENTAL")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] -C opt-level=1 -C panic=abort -C lto[..]-C codegen-units=2 -C debuginfo=2 [..]-C debug-assertions=on -C overflow-checks=off [..]-C rpath --out-dir [ROOT]/foo/target/release/deps -C incremental=[ROOT]/foo/target/release/incremental[..]`
[FINISHED] `release` profile [optimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn profile_config_override_precedence() {
    // Config values take precedence over manifest values.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = {path = "bar"}

                [profile.dev]
                codegen-units = 2

                [profile.dev.package.bar]
                opt-level = 3
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev.package.bar]
                opt-level = 2
            "#,
        )
        .build();

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar [..] -C opt-level=2[..]-C codegen-units=2 [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]-C codegen-units=2 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn profile_config_no_warn_unknown_override() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev.package.bar]
                codegen-units = 4
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr_does_not_contain("[..]warning[..]")
        .run();
}

#[cargo_test]
fn profile_config_mixed_types() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev]
                opt-level = 3
            "#,
        )
        .file(
            paths::home().join(".cargo/config.toml"),
            r#"
            [profile.dev]
            opt-level = 's'
            "#,
        )
        .build();

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-C opt-level=3 [..]`
[FINISHED] `dev` profile [optimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn named_config_profile() {
    // Exercises config named profiles.
    // foo -> middle -> bar -> dev
    // middle exists in Cargo.toml, the others in .cargo/config.toml
    use super::config::GlobalContextBuilder;
    use cargo::core::compiler::CompileKind;
    use cargo::core::profiles::{Profiles, UnitFor};
    use cargo::core::{PackageId, Workspace};
    use std::fs;
    paths::root().join(".cargo").mkdir_p();
    fs::write(
        paths::root().join(".cargo/config.toml"),
        r#"
            [profile.foo]
            inherits = "middle"
            codegen-units = 2
            [profile.foo.build-override]
            codegen-units = 6
            [profile.foo.package.dep]
            codegen-units = 7

            [profile.middle]
            inherits = "bar"
            codegen-units = 3

            [profile.bar]
            inherits = "dev"
            codegen-units = 4
            debug = 1
        "#,
    )
    .unwrap();
    fs::write(
        paths::root().join("Cargo.toml"),
        r#"
            [workspace]

            [profile.middle]
            inherits = "bar"
            codegen-units = 1
            opt-level = 1
            [profile.middle.package.dep]
            overflow-checks = false

            [profile.foo.build-override]
            codegen-units = 5
            debug-assertions = false
            [profile.foo.package.dep]
            codegen-units = 8
        "#,
    )
    .unwrap();
    let gctx = GlobalContextBuilder::new().build();
    let profile_name = "foo".into();
    let ws = Workspace::new(&paths::root().join("Cargo.toml"), &gctx).unwrap();
    let profiles = Profiles::new(&ws, profile_name).unwrap();

    let crates_io = cargo::core::SourceId::crates_io(&gctx).unwrap();
    let a_pkg = PackageId::try_new("a", "0.1.0", crates_io).unwrap();
    let dep_pkg = PackageId::try_new("dep", "0.1.0", crates_io).unwrap();

    // normal package
    let kind = CompileKind::Host;
    let p = profiles.get_profile(a_pkg, true, true, UnitFor::new_normal(kind), kind);
    assert_eq!(p.name, "foo");
    assert_eq!(p.codegen_units, Some(2)); // "foo" from config
    assert_eq!(p.opt_level, "1"); // "middle" from manifest
    assert_eq!(p.debuginfo.into_inner(), TomlDebugInfo::Limited); // "bar" from config
    assert_eq!(p.debug_assertions, true); // "dev" built-in (ignore build-override)
    assert_eq!(p.overflow_checks, true); // "dev" built-in (ignore package override)

    // build-override
    let bo = profiles.get_profile(a_pkg, true, true, UnitFor::new_host(false, kind), kind);
    assert_eq!(bo.name, "foo");
    assert_eq!(bo.codegen_units, Some(6)); // "foo" build override from config
    assert_eq!(bo.opt_level, "0"); // default to zero
    assert_eq!(bo.debuginfo.into_inner(), TomlDebugInfo::Limited); // SAME as normal
    assert_eq!(bo.debug_assertions, false); // "foo" build override from manifest
    assert_eq!(bo.overflow_checks, true); // SAME as normal

    // package overrides
    let po = profiles.get_profile(dep_pkg, false, true, UnitFor::new_normal(kind), kind);
    assert_eq!(po.name, "foo");
    assert_eq!(po.codegen_units, Some(7)); // "foo" package override from config
    assert_eq!(po.opt_level, "1"); // SAME as normal
    assert_eq!(po.debuginfo.into_inner(), TomlDebugInfo::Limited); // SAME as normal
    assert_eq!(po.debug_assertions, true); // SAME as normal
    assert_eq!(po.overflow_checks, false); // "middle" package override from manifest
}

#[cargo_test]
fn named_env_profile() {
    // Environment variables used to define a named profile.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v --profile=other")
        .env("CARGO_PROFILE_OTHER_CODEGEN_UNITS", "1")
        .env("CARGO_PROFILE_OTHER_INHERITS", "dev")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-C codegen-units=1 [..]`
[FINISHED] `other` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn test_with_dev_profile() {
    // The `test` profile inherits from `dev` for both local crates and
    // dependencies.
    Package::new("somedep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            somedep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("test --lib --no-run -v")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] somedep v1.0.0 (registry `dummy-registry`)
[COMPILING] somedep v1.0.0
[RUNNING] `rustc --crate-name somedep [..]`
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `test` profile [unoptimized] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`

"#]])
        .with_stdout_does_not_contain("[..] -C debuginfo=0[..]")
        .run();
}
