//! Tests for setting custom rustc flags.

use std::fs;

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{
    RawOutput, basic_manifest, paths, project, project_in_home, rustc_host, str,
};
use snapbox::assert_data_eq;

#[cargo_test]
fn env_rustflags_normal_source() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .build();

    // Use RUSTFLAGS to pass an argument that will generate an error
    p.cargo("check --lib")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --bin=a")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --example=b")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("test")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("bench")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn env_rustflags_build_script() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .build();

    p.cargo("check").env("RUSTFLAGS", "--cfg foo").run();
}

#[cargo_test]
fn env_rustflags_build_script_dep() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"

                [build-dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(not(foo))]
                fn bar() { }
            "#,
        )
        .build();

    foo.cargo("check").env("RUSTFLAGS", "--cfg foo").run();
}

#[cargo_test]
fn env_rustflags_normal_source_with_target() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .build();

    let host = &rustc_host();

    // Use RUSTFLAGS to pass an argument that will generate an error
    p.cargo("check --lib --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --bin=a --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --example=b --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("test --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("bench --target")
        .arg(host)
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn env_rustflags_build_script_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(!cfg!(foo)); }
            "#,
        )
        .build();

    let host = rustc_host();
    p.cargo("check --target")
        .arg(host)
        .env("RUSTFLAGS", "--cfg foo")
        .run();
}

#[cargo_test]
fn env_rustflags_build_script_with_target_doesnt_apply_to_host_kind() {
    // RUSTFLAGS should *not* be passed to rustc for build scripts when --target is specified as the
    // host triple even if target-applies-to-host-kind is enabled, to match legacy Cargo behavior.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(!cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                target-applies-to-host = true
            "#,
        )
        .build();

    let host = rustc_host();
    p.cargo("check --target")
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg(host)
        .arg("-Ztarget-applies-to-host")
        .env("RUSTFLAGS", "--cfg foo")
        .run();
}

#[cargo_test]
fn env_rustflags_build_script_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"

                [build-dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(foo)]
                fn bar() { }
            "#,
        )
        .build();

    let host = rustc_host();
    foo.cargo("check --target")
        .arg(host)
        .env("RUSTFLAGS", "--cfg foo")
        .run();
}

#[cargo_test]
fn env_rustflags_recompile() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("check").run();
    // Setting RUSTFLAGS forces a recompile
    p.cargo("check")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn env_rustflags_recompile2() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("check").env("RUSTFLAGS", "--cfg foo").run();
    // Setting RUSTFLAGS forces a recompile
    p.cargo("check")
        .env("RUSTFLAGS", "-Z bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn env_rustflags_no_recompile() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("check").env("RUSTFLAGS", "--cfg foo").run();
    p.cargo("check")
        .env("RUSTFLAGS", "--cfg foo")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_rustflags_normal_source() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = ["-Z", "bogus"]
            "#,
        )
        .build();

    p.cargo("check --lib")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --bin=a")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --example=b")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("test")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("bench")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn build_rustflags_build_script() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn build_rustflags_build_script_dep() {
    // RUSTFLAGS should be passed to rustc for build scripts
    // when --target is not specified.
    // In this test if --cfg foo is not passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"

                [build-dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(not(foo))]
                fn bar() { }
            "#,
        )
        .build();

    foo.cargo("check").run();
}

#[cargo_test]
fn build_rustflags_normal_source_with_target() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = ["-Z", "bogus"]
            "#,
        )
        .build();

    let host = &rustc_host();

    // Use build.rustflags to pass an argument that will generate an error
    p.cargo("check --lib --target")
        .arg(host)
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --bin=a --target")
        .arg(host)
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --example=b --target")
        .arg(host)
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("test --target")
        .arg(host)
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("bench --target")
        .arg(host)
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn build_rustflags_build_script_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(!cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    let host = rustc_host();
    p.cargo("check --target").arg(host).run();
}

#[cargo_test]
fn build_rustflags_build_script_dep_with_target() {
    // RUSTFLAGS should not be passed to rustc for build scripts
    // when --target is specified.
    // In this test if --cfg foo is passed the build will fail.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                build = "build.rs"

                [build-dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                fn bar() { }
                #[cfg(foo)]
                fn bar() { }
            "#,
        )
        .build();

    let host = rustc_host();
    foo.cargo("check --target").arg(host).run();
}

#[cargo_test]
fn build_rustflags_recompile() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("check").run();

    // Setting RUSTFLAGS forces a recompile
    let config = r#"
        [build]
        rustflags = ["-Z", "bogus"]
        "#;
    let config_file = paths::root().join("foo/.cargo/config.toml");
    fs::create_dir_all(config_file.parent().unwrap()).unwrap();
    fs::write(config_file, config).unwrap();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn build_rustflags_recompile2() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("check").env("RUSTFLAGS", "--cfg foo").run();

    // Setting RUSTFLAGS forces a recompile
    let config = r#"
        [build]
        rustflags = ["-Z", "bogus"]
        "#;
    let config_file = paths::root().join("foo/.cargo/config.toml");
    fs::create_dir_all(config_file.parent().unwrap()).unwrap();
    fs::write(config_file, config).unwrap();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn build_rustflags_no_recompile() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p.cargo("check").env("RUSTFLAGS", "--cfg foo").run();
    p.cargo("check")
        .env("RUSTFLAGS", "--cfg foo")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_rustflags_with_home_config() {
    // We need a config file inside the home directory
    let home = paths::home();
    let home_config = home.join(".cargo");
    fs::create_dir(&home_config).unwrap();
    fs::write(
        &home_config.join("config"),
        r#"
            [build]
            rustflags = ["-Cllvm-args=-x86-asm-syntax=intel"]
        "#,
    )
    .unwrap();

    // And we need the project to be inside the home directory
    // so the walking process finds the home project twice.
    let p = project_in_home("foo").file("src/lib.rs", "").build();

    p.cargo("check -v").run();
}

#[cargo_test]
fn target_rustflags_normal_source() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            "benches/d.rs",
            r#"
            #![feature(test)]
            extern crate test;
            #[bench] fn run1(_ben: &mut test::Bencher) { }
            "#,
        )
        .file(
            ".cargo/config.toml",
            &format!(
                "
            [target.{}]
            rustflags = [\"-Z\", \"bogus\"]
            ",
                rustc_host()
            ),
        )
        .build();

    p.cargo("check --lib")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --bin=a")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --example=b")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("test")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("bench")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn target_rustflags_also_for_build_scripts() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config.toml",
            &format!(
                "
            [target.{}]
            rustflags = [\"--cfg=foo\"]
            ",
                rustc_host()
            ),
        )
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn target_rustflags_not_for_build_scripts_with_target() {
    let host = rustc_host();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(!cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config.toml",
            &format!(
                "
            [target.{}]
            rustflags = [\"--cfg=foo\"]
            ",
                host
            ),
        )
        .build();

    p.cargo("check --target").arg(host).run();

    // Enabling -Ztarget-applies-to-host should not make a difference without the config setting
    p.cargo("check --target")
        .arg(host)
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .run();

    // Even with the setting, the rustflags from `target.` should not apply, to match the legacy
    // Cargo behavior.
    p.change_file(
        ".cargo/config.toml",
        &format!(
            "
        target-applies-to-host = true

        [target.{}]
        rustflags = [\"--cfg=foo\"]
        ",
            host
        ),
    );
    p.cargo("check --target")
        .arg(host)
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .run();
}

#[cargo_test]
fn build_rustflags_for_build_scripts() {
    let host = rustc_host();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() { assert!(cfg!(foo), "CFG FOO!"); }
            "#,
        )
        .file(
            ".cargo/config.toml",
            "
            [build]
            rustflags = [\"--cfg=foo\"]
            ",
        )
        .build();

    // With "legacy" behavior, build.rustflags should apply to build scripts without --target
    p.cargo("check").run();

    // But should _not_ apply _with_ --target
    p.cargo("check --target")
        .arg(host)
        .with_status(101)
        .with_stderr_data("...\n[..]CFG FOO![..]\n...")
        .run();

    // Enabling -Ztarget-applies-to-host should not make a difference without the config setting
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .run();
    p.cargo("check --target")
        .arg(host)
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .with_status(101)
        .with_stderr_data("...\n[..]CFG FOO![..]\n...")
        .run();

    // When set to false though, the "proper" behavior where host artifacts _only_ pick up on
    // [host] should be applied.
    p.change_file(
        ".cargo/config.toml",
        "
        target-applies-to-host = false

        [build]
        rustflags = [\"--cfg=foo\"]
        ",
    );
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .with_status(101)
        .with_stderr_data("...\n[..]CFG FOO![..]\n...")
        .run();
    p.cargo("check --target")
        .arg(host)
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .with_status(101)
        .with_stderr_data("...\n[..]CFG FOO![..]\n...")
        .run();
}

#[cargo_test]
fn host_rustflags_for_build_scripts() {
    let host = rustc_host();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                // Ensure that --cfg=foo is passed.
                fn main() { assert!(cfg!(foo)); }
            "#,
        )
        .file(
            ".cargo/config.toml",
            &format!(
                "
                target-applies-to-host = false

                [host.{}]
                rustflags = [\"--cfg=foo\"]
                ",
                host
            ),
        )
        .build();

    p.cargo("check --target")
        .arg(host)
        .masquerade_as_nightly_cargo(&["target-applies-to-host", "host-config"])
        .arg("-Ztarget-applies-to-host")
        .arg("-Zhost-config")
        .run();
}

// target.{}.rustflags takes precedence over build.rustflags
#[cargo_test]
fn target_rustflags_precedence() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            ".cargo/config.toml",
            &format!(
                "
            [build]
            rustflags = [\"--cfg\", \"foo\"]

            [target.{}]
            rustflags = [\"-Z\", \"bogus\"]
            ",
                rustc_host()
            ),
        )
        .build();

    p.cargo("check --lib")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --bin=a")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("check --example=b")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("test")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
    p.cargo("bench")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  [..]bogus[..]
...
"#]])
        .run();
}

#[cargo_test]
fn cfg_rustflags_normal_source() {
    let p = project()
        .file("src/lib.rs", "pub fn t() {}")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                [target.'cfg({})']
                rustflags = ["--cfg", "bar"]
                "#,
                if rustc_host().contains("-windows-") {
                    "windows"
                } else {
                    "not(windows)"
                }
            ),
        )
        .build();

    p.cargo("build --lib -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] --cfg bar`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build --bin=a -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name a [..] --cfg bar`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build --example=b -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name b [..] --cfg bar`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("test --no-run -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] --cfg bar`
[RUNNING] `rustc [..] --cfg bar`
[RUNNING] `rustc [..] --cfg bar`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/a-[HASH][EXE]`
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/c-[HASH][EXE]`

"#]])
        .run();

    p.cargo("bench --no-run -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] --cfg bar`
[RUNNING] `rustc [..] --cfg bar`
[RUNNING] `rustc [..] --cfg bar`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/release/deps/foo-[HASH][EXE]`
[EXECUTABLE] `[ROOT]/foo/target/release/deps/a-[HASH][EXE]`

"#]])
        .run();
}

// target.'cfg(...)'.rustflags takes precedence over build.rustflags
#[cargo_test]
fn cfg_rustflags_precedence() {
    let p = project()
        .file("src/lib.rs", "pub fn t() {}")
        .file("src/bin/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("tests/c.rs", "#[test] fn f() { }")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                [build]
                rustflags = ["--cfg", "foo"]

                [target.'cfg({})']
                rustflags = ["--cfg", "bar"]
                "#,
                if rustc_host().contains("-windows-") {
                    "windows"
                } else {
                    "not(windows)"
                }
            ),
        )
        .build();

    p.cargo("build --lib -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] --cfg bar`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build --bin=a -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name a [..] --cfg bar`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build --example=b -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name b [..] --cfg bar`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("test --no-run -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] --cfg bar`
[RUNNING] `rustc [..] --cfg bar`
[RUNNING] `rustc [..] --cfg bar`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/foo-[HASH][EXE]`
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/a-[HASH][EXE]`
[EXECUTABLE] `[ROOT]/foo/target/debug/deps/c-[HASH][EXE]`

"#]])
        .run();

    p.cargo("bench --no-run -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] --cfg bar`
[RUNNING] `rustc [..] --cfg bar`
[RUNNING] `rustc [..] --cfg bar`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/release/deps/foo-[HASH][EXE]`
[EXECUTABLE] `[ROOT]/foo/target/release/deps/a-[HASH][EXE]`

"#]])
        .run();
}

#[cargo_test]
fn target_rustflags_string_and_array_form1() {
    let p1 = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p1.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] --cfg foo`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let p2 = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            rustflags = "--cfg foo"
            "#,
        )
        .build();

    p2.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] --cfg foo`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn target_rustflags_string_and_array_form2() {
    let p1 = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.{}]
                    rustflags = ["--cfg", "foo"]
                "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p1.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] --cfg foo`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let p2 = project()
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.{}]
                    rustflags = "--cfg foo"
                "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p2.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] --cfg foo`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn two_matching_in_config() {
    let p1 = project()
        .file(
            ".cargo/config.toml",
            r#"
                [target.'cfg(unix)']
                rustflags = ["--cfg", 'foo="a"']
                [target.'cfg(windows)']
                rustflags = ["--cfg", 'foo="a"']
                [target.'cfg(target_pointer_width = "32")']
                rustflags = ["--cfg", 'foo="b"']
                [target.'cfg(target_pointer_width = "64")']
                rustflags = ["--cfg", 'foo="b"']
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #![allow(unexpected_cfgs)]
                fn main() {
                    if cfg!(foo = "a") {
                        println!("a");
                    } else if cfg!(foo = "b") {
                        println!("b");
                    } else {
                        panic!()
                    }
                }
            "#,
        )
        .build();

    p1.cargo("run").run();
    p1.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn env_rustflags_misspelled() {
    let p = project().file("src/main.rs", "fn main() { }").build();

    for cmd in &["check", "build", "run", "test", "bench"] {
        p.cargo(cmd)
            .env("RUST_FLAGS", "foo")
            .with_stderr_data(str![[r#"
[WARNING] ignoring environment variable `RUST_FLAGS`
  |
  = [HELP] rust flags are passed via `RUSTFLAGS`
...
"#]])
            .run();
    }
}

#[cargo_test]
fn env_rustflags_misspelled_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"
                build = "build.rs"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() { }")
        .build();

    p.cargo("check")
        .env("RUST_FLAGS", "foo")
        .with_stderr_data(str![[r#"
[WARNING] ignoring environment variable `RUST_FLAGS`
  |
  = [HELP] rust flags are passed via `RUSTFLAGS`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn remap_path_prefix_works() {
    // Check that remap-path-prefix works.
    Package::new("bar", "0.1.0")
        .file("src/lib.rs", "pub fn f() -> &'static str { file!() }")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = "0.1"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                println!("{}", bar::f());
            }
            "#,
        )
        .build();

    p.cargo("run")
        .env(
            "RUSTFLAGS",
            format!("--remap-path-prefix={}=/foo", paths::root().display()),
        )
        .with_stdout_data(str![[r#"
/foo/home/.cargo/registry/src/-[HASH]/bar-0.1.0/src/lib.rs

"#]])
        .run();
}

#[cargo_test]
fn rustflags_remap_path_prefix_ignored_for_c_metadata() {
    let p = project().file("src/lib.rs", "").build();

    let build_output = p
        .cargo("build -v")
        .env(
            "RUSTFLAGS",
            "--remap-path-prefix=/abc=/zoo --remap-path-prefix /spaced=/zoo",
        )
        .run();
    let first_c_metadata = dbg!(get_c_metadata(build_output));

    p.cargo("clean").run();

    let build_output = p
        .cargo("build -v")
        .env(
            "RUSTFLAGS",
            "--remap-path-prefix=/def=/zoo --remap-path-prefix /earth=/zoo",
        )
        .run();
    let second_c_metadata = dbg!(get_c_metadata(build_output));

    assert_data_eq!(first_c_metadata, second_c_metadata);
}

#[cargo_test]
fn rustc_remap_path_prefix_ignored_for_c_metadata() {
    let p = project().file("src/lib.rs", "").build();

    let build_output = p
        .cargo("rustc -v -- --remap-path-prefix=/abc=/zoo --remap-path-prefix /spaced=/zoo")
        .run();
    let first_c_metadata = dbg!(get_c_metadata(build_output));

    p.cargo("clean").run();

    let build_output = p
        .cargo("rustc -v -- --remap-path-prefix=/def=/zoo --remap-path-prefix /earth=/zoo")
        .run();
    let second_c_metadata = dbg!(get_c_metadata(build_output));

    assert_data_eq!(first_c_metadata, second_c_metadata);
}

// `--remap-path-prefix` is meant to take two different binaries and make them the same but the
// rlib name, including `-Cextra-filename`, can still end up in the binary so it can't change
#[cargo_test]
fn rustflags_remap_path_prefix_ignored_for_c_extra_filename() {
    let p = project().file("src/lib.rs", "").build();

    let build_output = p
        .cargo("build -v")
        .env(
            "RUSTFLAGS",
            "--remap-path-prefix=/abc=/zoo --remap-path-prefix /spaced=/zoo",
        )
        .run();
    let first_c_extra_filename = dbg!(get_c_extra_filename(build_output));

    p.cargo("clean").run();

    let build_output = p
        .cargo("build -v")
        .env(
            "RUSTFLAGS",
            "--remap-path-prefix=/def=/zoo --remap-path-prefix /earth=/zoo",
        )
        .run();
    let second_c_extra_filename = dbg!(get_c_extra_filename(build_output));

    assert_data_eq!(first_c_extra_filename, second_c_extra_filename);
}

// `--remap-path-prefix` is meant to take two different binaries and make them the same but the
// rlib name, including `-Cextra-filename`, can still end up in the binary so it can't change
#[cargo_test]
fn rustc_remap_path_prefix_ignored_for_c_extra_filename() {
    let p = project().file("src/lib.rs", "").build();

    let build_output = p
        .cargo("rustc -v -- --remap-path-prefix=/abc=/zoo --remap-path-prefix /spaced=/zoo")
        .run();
    let first_c_extra_filename = dbg!(get_c_extra_filename(build_output));

    p.cargo("clean").run();

    let build_output = p
        .cargo("rustc -v -- --remap-path-prefix=/def=/zoo --remap-path-prefix /earth=/zoo")
        .run();
    let second_c_extra_filename = dbg!(get_c_extra_filename(build_output));

    assert_data_eq!(first_c_extra_filename, second_c_extra_filename);
}

fn get_c_metadata(output: RawOutput) -> String {
    let get_c_metadata_re =
        regex::Regex::new(r".* (--crate-name [^ ]+).* (-C ?metadata=[^ ]+).*").unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    let mut c_metadata = get_c_metadata_re
        .captures_iter(&stderr)
        .map(|c| {
            let (_, [name, c_metadata]) = c.extract();
            format!("{name} {c_metadata}")
        })
        .collect::<Vec<_>>();
    assert!(
        !c_metadata.is_empty(),
        "`{get_c_metadata_re:?}` did not match:\n```\n{stderr}\n```"
    );
    c_metadata.sort();
    c_metadata.join("\n")
}

fn get_c_extra_filename(output: RawOutput) -> String {
    let get_c_extra_filename_re =
        regex::Regex::new(r".* (--crate-name [^ ]+).* (-C ?extra-filename=[^ ]+).*").unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    let mut c_extra_filename = get_c_extra_filename_re
        .captures_iter(&stderr)
        .map(|c| {
            let (_, [name, c_extra_filename]) = c.extract();
            format!("{name} {c_extra_filename}")
        })
        .collect::<Vec<_>>();
    assert!(
        !c_extra_filename.is_empty(),
        "`{get_c_extra_filename_re:?}` did not match:\n```\n{stderr}\n```"
    );
    c_extra_filename.sort();
    c_extra_filename.join("\n")
}

#[cargo_test]
fn host_config_rustflags_with_target() {
    // regression test for https://github.com/rust-lang/cargo/issues/10206
    let p = project()
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() { assert!(cfg!(foo)); }")
        .file(".cargo/config.toml", "target-applies-to-host = false")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["target-applies-to-host", "host-config"])
        .arg("-Zhost-config")
        .arg("-Ztarget-applies-to-host")
        .arg("-Zunstable-options")
        .arg("--config")
        .arg("host.rustflags=[\"--cfg=foo\"]")
        .run();
}

#[cargo_test]
fn target_applies_to_host_rustflags_works() {
    // Ensures that rustflags are passed to the target when
    // target_applies_to_host=false
    let p = project()
        .file(
            "src/lib.rs",
            r#"#[cfg(feature = "flag")] compile_error!("flag passed");"#,
        )
        .build();

    // Use RUSTFLAGS to pass an argument that will generate an error.
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .env("CARGO_TARGET_APPLIES_TO_HOST", "false")
        .env("RUSTFLAGS", r#"--cfg feature="flag""#)
        .with_status(101)
        .with_stderr_data(
            "[CHECKING] foo v0.0.1 ([ROOT]/foo)
[ERROR] flag passed
...",
        )
        .run();
}

#[cargo_test]
fn target_applies_to_host_rustdocflags_works() {
    // Ensures that rustflags are passed to the target when
    // target_applies_to_host=false
    let p = project()
        .file(
            "src/lib.rs",
            r#"#[cfg(feature = "flag")] compile_error!("flag passed");"#,
        )
        .build();

    // Use RUSTFLAGS to pass an argument that would generate an error
    // but it is ignored.
    p.cargo("doc")
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .env("CARGO_TARGET_APPLIES_TO_HOST", "false")
        .env("RUSTDOCFLAGS", r#"--cfg feature="flag""#)
        .with_status(101)
        .with_stderr_data(
            "[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[ERROR] flag passed
...",
        )
        .run();
}

#[cargo_test]
fn host_config_shared_build_dep() {
    // rust-lang/cargo#14253
    Package::new("cc", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bootstrap"
            edition = "2021"

            [dependencies]
            cc = "1.0.0"

            [build-dependencies]
            cc = "1.0.0"

            [profile.dev]
            debug = 0
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            "
            target-applies-to-host=false

            [host]
            rustflags = ['--cfg', 'from_host']

            [build]
            rustflags = ['--cfg', 'from_target']
            ",
        )
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo(&["target-applies-to-host"])
        .arg("-Ztarget-applies-to-host")
        .arg("-Zhost-config")
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] cc v1.0.0 (registry `dummy-registry`)
[COMPILING] cc v1.0.0
[RUNNING] `rustc --crate-name cc [..]--cfg from_host[..]`
[RUNNING] `rustc --crate-name cc [..]--cfg from_target[..]`
[COMPILING] bootstrap v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name build_script_build [..]--cfg from_host[..]`
[RUNNING] `[ROOT]/foo/target/debug/build/bootstrap-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name bootstrap[..]--cfg from_target[..]`
[FINISHED] `dev` profile [unoptimized] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}
