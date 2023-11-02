//! Tests for `-Ztrim-paths`.

use cargo_test_support::basic_manifest;
use cargo_test_support::git;
use cargo_test_support::paths;
use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn gated_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [profile.dev]
                trim-paths = "macro"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  feature `trim-paths` is required",
        )
        .run();
}

#[cargo_test]
fn gated_config_toml() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev]
                trim-paths = "macro"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] config profile `dev` is not valid (defined in `[CWD]/.cargo/config.toml`)

Caused by:
  feature `trim-paths` is required",
        )
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn release_profile_default_to_object() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --release --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] release [..]",
        )
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn one_option() {
    let build = |option| {
        let p = project()
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"

                    [profile.dev]
                    trim-paths = "{option}"
                "#
                ),
            )
            .file("src/lib.rs", "")
            .build();

        p.cargo("build -v -Ztrim-paths")
    };

    for option in ["macro", "diagnostics", "object", "all"] {
        build(option)
            .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope={option} \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] dev [..]",
            ))
            .run();
    }
    build("none")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_does_not_contain("[..]-Zremap-path-scope=[..]")
        .with_stderr_does_not_contain("[..]--remap-path-prefix=[..]")
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn multiple_options() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [profile.dev]
                trim-paths = ["diagnostics", "macro", "object"]
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=diagnostics,macro,object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] dev [..]",
        )
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn profile_merge_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [profile.dev]
                trim-paths = ["macro"]

                [profile.custom]
                inherits = "dev"
                trim-paths = ["diagnostics"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v -Ztrim-paths --profile custom")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=diagnostics \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] custom [..]",
        )
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn registry_dependency() {
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", r#"pub fn f() { println!("{}", file!()); }"#)
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = "0.0.1"

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .build();

    let registry_src = paths::home().join(".cargo/registry/src");
    let pkg_remap = format!("{}/[..]/bar-0.0.1=bar-0.0.1", registry_src.display());

    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stdout("bar-0.0.1/src/lib.rs")
        .with_stderr(&format!(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 ([..])
[COMPILING] bar v0.0.1
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix={pkg_remap} [..]
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] dev [..]
[RUNNING] `target/debug/foo[EXE]`"
        ))
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn git_dependency() {
    let git_project = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
            .file("src/lib.rs", r#"pub fn f() { println!("{}", file!()); }"#)
    });
    let url = git_project.url();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = {{ git = "{url}" }}

                [profile.dev]
                trim-paths = "object"
           "#
            ),
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .build();

    let git_checkouts_src = paths::home().join(".cargo/git/checkouts");
    let pkg_remap = format!("{}/bar-[..]/[..]=bar-0.0.1", git_checkouts_src.display());

    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stdout("bar-0.0.1/src/lib.rs")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{url}`
[COMPILING] bar v0.0.1 ({url}[..])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix={pkg_remap} [..]
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] dev [..]
[RUNNING] `target/debug/foo[EXE]`"
        ))
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn path_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = { path = "cocktail-bar" }

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .file("cocktail-bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "cocktail-bar/src/lib.rs",
            r#"pub fn f() { println!("{}", file!()); }"#,
        )
        .build();

    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stdout("cocktail-bar/src/lib.rs")
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1 ([..]/cocktail-bar)
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] dev [..]
[RUNNING] `target/debug/foo[EXE]`"
        ))
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn path_dependency_outside_workspace() {
    let bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", r#"pub fn f() { println!("{}", file!()); }"#)
        .build();
    let bar_path = bar.url().to_file_path().unwrap();
    let bar_path = bar_path.display();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = { path = "../bar" }

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .build();

    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stdout("bar-0.0.1/src/lib.rs")
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1 ([..]/bar)
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix={bar_path}=bar-0.0.1 [..]
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] dev [..]
[RUNNING] `target/debug/foo[EXE]`"
        ))
        .run();
}

#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn diagnostics_works() {
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", r#"pub fn f() { let unused = 0; }"#)
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = "0.0.1"

                [profile.dev]
                trim-paths = "diagnostics"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    let registry_src = paths::home().join(".cargo/registry/src");
    let registry_src = registry_src.display();
    let pkg_remap = format!("{registry_src}/[..]/bar-0.0.1=bar-0.0.1");

    p.cargo("build -vv -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_line_without(
            &["[..]bar-0.0.1/src/lib.rs:1[..]"],
            &[&format!("{registry_src}")],
        )
        .with_stderr_contains("[..]unused_variables[..]")
        .with_stderr_contains(&format!(
            "\
[RUNNING] [..]rustc [..]\
    -Zremap-path-scope=diagnostics \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix={pkg_remap} [..]",
        ))
        .with_stderr_contains(
            "\
[RUNNING] [..]rustc [..]\
    -Zremap-path-scope=diagnostics \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]",
        )
        .run();
}

#[cfg(target_os = "linux")]
#[cargo_test(requires_readelf, nightly, reason = "-Zremap-path-scope is unstable")]
fn object_works() {
    use std::os::unix::ffi::OsStrExt;

    let run_readelf = |path| {
        std::process::Command::new("readelf")
            .arg("-wi")
            .arg(path)
            .output()
            .expect("readelf works")
    };

    let registry_src = paths::home().join(".cargo/registry/src");
    let pkg_remap = format!("{}/[..]/bar-0.0.1=bar-0.0.1", registry_src.display());
    let rust_src = "/lib/rustc/src/rust".as_bytes();
    let registry_src_bytes = registry_src.as_os_str().as_bytes();

    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", r#"pub fn f() { println!("{}", file!()); }"#)
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = "0.0.1"
           "#,
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .build();

    let pkg_root = p.root();
    let pkg_root = pkg_root.as_os_str().as_bytes();

    p.cargo("build").run();

    let bin_path = p.bin("foo");
    assert!(bin_path.is_file());
    let stdout = run_readelf(bin_path).stdout;
    // TODO: re-enable this check when rustc bootstrap disables remapping
    // <https://github.com/rust-lang/cargo/pull/12625#discussion_r1371714791>
    // assert!(memchr::memmem::find(&stdout, rust_src).is_some());
    assert!(memchr::memmem::find(&stdout, registry_src_bytes).is_some());
    assert!(memchr::memmem::find(&stdout, pkg_root).is_some());

    p.cargo("clean").run();

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = "0.0.1"

            [profile.dev]
            trim-paths = "object"
       "#,
    );

    p.cargo("build --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix={pkg_remap} [..]
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]\
    -Zremap-path-scope=object \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..] \
    --remap-path-prefix=[CWD]= [..]
[FINISHED] dev [..]",
        ))
        .run();

    let bin_path = p.bin("foo");
    assert!(bin_path.is_file());
    let stdout = run_readelf(bin_path).stdout;
    assert!(memchr::memmem::find(&stdout, rust_src).is_none());
    assert!(memchr::memmem::find(&stdout, registry_src_bytes).is_none());
    assert!(memchr::memmem::find(&stdout, pkg_root).is_none());
}

// TODO: might want to move to test/testsuite/build_script.rs once stabilized.
#[cargo_test(nightly, reason = "-Zremap-path-scope is unstable")]
fn custom_build_env_var_trim_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
           "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "")
        .build();

    let test_cases = [
        ("[]", "none"),
        ("\"all\"", "all"),
        ("\"diagnostics\"", "diagnostics"),
        ("\"macro\"", "macro"),
        ("\"none\"", "none"),
        ("\"object\"", "object"),
        ("false", "none"),
        ("true", "all"),
        (
            r#"["diagnostics", "macro", "object"]"#,
            "diagnostics,macro,object",
        ),
    ];

    for (opts, expected) in test_cases {
        p.change_file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [profile.dev]
                trim-paths = {opts}
                "#
            ),
        );

        p.change_file(
            "build.rs",
            &format!(
                r#"
                fn main() {{
                    assert_eq!(
                        std::env::var("CARGO_TRIM_PATHS").unwrap().as_str(),
                        "{expected}",
                    );
                }}
                "#
            ),
        );

        p.cargo("build -Ztrim-paths")
            .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
            .run();
    }
}
