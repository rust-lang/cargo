//! Tests for `-Ztrim-paths`.

use crate::prelude::*;
use cargo_test_support::basic_manifest;
use cargo_test_support::git;
use cargo_test_support::paths;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

#[cargo_test]
fn gated_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [profile.dev]
                trim-paths = "macro"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `trim-paths` is required
...
"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] config profile `dev` is not valid (defined in `[ROOT]/foo/.cargo/config.toml`)

Caused by:
  feature `trim-paths` is required
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
fn release_profile_default_to_object() {
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

    p.cargo("build --release --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
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
                    edition = "2015"

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
            .with_stderr_data(&format!(
                "\
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]\
    --remap-path-scope={option} \
    --remap-path-prefix=[ROOT]/foo=. \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
",
            ))
            .run();
    }
    build("none")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_does_not_contain("[..]--remap-path-scope=[..]")
        .with_stderr_does_not_contain("[..]--remap-path-prefix=[..]")
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
fn multiple_options() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [profile.dev]
                trim-paths = ["diagnostics", "macro", "object"]
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=diagnostics,macro,object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
fn profile_merge_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=diagnostics --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `custom` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
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
                edition = "2015"

                [dependencies]
                bar = "0.0.1"

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .build();

    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stdout_data(str![[r#"
-[..]/bar-0.0.1/src/lib.rs

"#]]) // Omit the hash of Source URL
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/home/.cargo/registry/src= --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
fn registry_dependency_with_build_script_codegen() {
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "build.rs",
            r#"
            fn main() {
                let out_dir = std::env::var("OUT_DIR").unwrap();
                let dest = std::path::PathBuf::from(out_dir);
                std::fs::write(
                    dest.join("bindings.rs"),
                    "pub fn my_file() -> &'static str { file!() }",
                )
                .unwrap();
            }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
        "#,
        )
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = "0.0.1"

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file(
            "src/main.rs",
            r#"fn main() { println!("{}", bar::my_file()); }"#,
        )
        .build();

    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        // Macros should be sanitized
        .with_stdout_data(str![[r#"
/cargo/build-dir/debug/build/bar-[HASH]/out/bindings.rs

"#]]) // Omit the hash of Source URL
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
[RUNNING] `rustc --crate-name build_script_build [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/home/.cargo/registry/src= --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[RUNNING] `[ROOT]/foo/target/debug/build/bar-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name bar [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/home/.cargo/registry/src= --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
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
                edition = "2015"

                [dependencies]
                bar = {{ git = "{url}" }}

                [profile.dev]
                trim-paths = "object"
           "#
            ),
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .build();

    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stdout_data(str![[r#"
bar-[..]/[..]/src/lib.rs

"#]]) // Omit the hash of Source URL and commit
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOTURL]/bar#[..])
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/home/.cargo/git/checkouts= --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
fn path_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

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
        .with_stdout_data(str![[r#"
cocktail-bar/src/lib.rs

"#]])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/cocktail-bar)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
fn path_dependency_outside_workspace() {
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", r#"pub fn f() { println!("{}", file!()); }"#)
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

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
        .with_stdout_data(str![[r#"
bar-0.0.1/src/lib.rs

"#]])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/bar)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/bar=bar-0.0.1 --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
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
                edition = "2015"

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

    p.cargo("build -vv -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_line_without(
            &["[..]bar-0.0.1/src/lib.rs:1[..]"],
            &[&format!("{registry_src}")],
        )
        .with_stderr_data(str![[r#"
...
[RUNNING] `[..] rustc [..]--remap-path-scope=diagnostics --remap-path-prefix=[ROOT]/home/.cargo/registry/src= --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[WARNING] unused variable: `unused`
...
[RUNNING] `[..] rustc [..]--remap-path-scope=diagnostics --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
...
"#]])
        .run();
}

#[cfg(target_os = "macos")]
mod object_works {
    use super::*;

    fn inspect_debuginfo(path: &std::path::Path) -> Vec<u8> {
        std::process::Command::new("nm")
            .arg("-pa")
            .arg(path)
            .output()
            .expect("nm works")
            .stdout
    }

    #[cargo_test(
        requires = "nm",
        nightly,
        reason = "--remap-path-scope will be stabilized in 1.95"
    )]
    fn with_split_debuginfo_off() {
        object_works_helper("off", inspect_debuginfo);
    }

    #[cargo_test(
        requires = "nm",
        nightly,
        reason = "--remap-path-scope will be stabilized in 1.95"
    )]
    fn with_split_debuginfo_packed() {
        object_works_helper("packed", inspect_debuginfo);
    }

    #[cargo_test(
        requires = "nm",
        nightly,
        reason = "--remap-path-scope will be stabilized in 1.95"
    )]
    fn with_split_debuginfo_unpacked() {
        object_works_helper("unpacked", inspect_debuginfo);
    }
}

#[cfg(target_os = "linux")]
mod object_works {
    use super::*;

    fn inspect_debuginfo(path: &std::path::Path) -> Vec<u8> {
        std::process::Command::new("readelf")
            .arg("--debug-dump=info")
            .arg("--debug-dump=no-follow-links") // older version can't recognized but just a warning
            .arg(path)
            .output()
            .expect("readelf works")
            .stdout
    }

    #[cargo_test(
        requires = "readelf",
        nightly,
        reason = "--remap-path-scope will be stabilized in 1.95"
    )]
    fn with_split_debuginfo_off() {
        object_works_helper("off", inspect_debuginfo);
    }

    #[cargo_test(
        requires = "readelf",
        nightly,
        reason = "--remap-path-scope will be stabilized in 1.95"
    )]
    fn with_split_debuginfo_packed() {
        object_works_helper("packed", inspect_debuginfo);
    }

    #[cargo_test(
        requires = "readelf",
        nightly,
        reason = "--remap-path-scope will be stabilized in 1.95"
    )]
    fn with_split_debuginfo_unpacked() {
        object_works_helper("unpacked", inspect_debuginfo);
    }
}

#[cfg(target_env = "msvc")]
mod object_works {
    use super::*;

    fn inspect_debuginfo(path: &std::path::Path) -> Vec<u8> {
        std::process::Command::new("strings")
            .arg(path)
            .output()
            .expect("strings works")
            .stdout
    }

    // windows-msvc supports split-debuginfo=packed only
    #[cargo_test(
        requires = "strings",
        nightly,
        reason = "--remap-path-scope will be stabilized in 1.95"
    )]
    fn with_split_debuginfo_packed() {
        object_works_helper("packed", inspect_debuginfo);
    }
}

#[cfg(all(target_os = "windows", target_env = "gnu", not(target_abi = "llvm")))]
mod object_works {
    use super::*;

    fn inspect_debuginfo(path: &std::path::Path) -> Vec<u8> {
        let parent = path.parent().expect("binary has a parent directory");
        let file_name = path.file_name().expect("binary has a file name");
        let output = std::process::Command::new("objdump")
            // Avoid echoing the absolute input path in objdump's file header.
            .current_dir(parent)
            .arg("--dwarf=info")
            .arg(file_name)
            .output()
            .expect("objdump works");
        assert!(
            output.status.success(),
            "objdump failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        output.stdout
    }

    // rustc currently supports only split-debuginfo=off on windows-gnu.
    // <https://github.com/rust-lang/rust/blob/47101adcea71daee3c2879218f5b883bcdf180aa/compiler/rustc_target/src/spec/base/windows_gnu.rs#L105-L107>
    #[cargo_test(requires = "objdump")]
    fn with_split_debuginfo_off() {
        object_works_helper("off", inspect_debuginfo);
    }
}

fn object_works_helper(split_debuginfo: &str, run: impl Fn(&std::path::Path) -> Vec<u8>) {
    let registry_src = paths::home().join(".cargo").join("registry").join("src");
    let registry_src_bytes = registry_src.as_os_str().as_encoded_bytes();
    let rust_src = "/lib/rustc/src/rust".as_bytes();

    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", r#"pub fn f() { println!("{}", file!()); }"#)
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = "0.0.1"

                [profile.dev]
                split-debuginfo = "{split_debuginfo}"
           "#
            ),
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .build();

    let pkg_root = p.root();
    let pkg_root = pkg_root.as_os_str().as_encoded_bytes();

    // Our baseline of which source roots are discoverable without object trimming.
    p.cargo("build").run();

    let bin_path = p.bin("foo");
    assert!(bin_path.is_file());
    let stdout = run(&bin_path);

    // TODO: re-enable this check when rustc bootstrap disables remapping
    // <https://github.com/rust-lang/cargo/pull/12625#discussion_r1371714791>
    // assert!(memchr::memmem::find(&stdout, rust_src).is_some());

    // `file!()` in `bar()` keeps untrimmed registry source in the executable
    // even when debuginfo is separate.
    assert!(memchr::memmem::find(&stdout, registry_src_bytes).is_some());

    // The local package root occurs only in debuginfo in this fixture.
    // MSVC puts that debuginfo in the PDB,
    // while the other inspectors read embedded debuginfo.
    if cfg!(target_env = "msvc") {
        assert!(memchr::memmem::find(&stdout, pkg_root).is_none());
    } else {
        assert!(memchr::memmem::find(&stdout, pkg_root).is_some());
    }
    p.cargo("clean").run();

    p.cargo("build --verbose -Ztrim-paths")
        .arg("--config")
        .arg(r#"profile.dev.trim-paths="object""#)
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(&format!(
            "\
[COMPILING] bar v0.0.1
[RUNNING] `rustc [..]-C split-debuginfo={split_debuginfo} [..]\
    --remap-path-scope=object \
    --remap-path-prefix=[ROOT]/home/.cargo/registry/src= \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]-C split-debuginfo={split_debuginfo} [..]\
    --remap-path-scope=object \
    --remap-path-prefix=[ROOT]/foo=. \
    --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
",
        ))
        .run();

    let bin_path = p.bin("foo");
    assert!(bin_path.is_file());
    let stdout = run(&bin_path);

    // Original sysroot source root should be trimmed.
    assert!(memchr::memmem::find(&stdout, rust_src).is_none());

    // Check line by line so macOS can exempt untrimmable `OSO` symbols.
    for line in stdout.split(|c| c == &b'\n') {
        // original registry source root was trimmed.
        let registry_is_trimmed = memchr::memmem::find(line, registry_src_bytes).is_none();
        // original project root was trimmed.
        let local_is_trimmed = memchr::memmem::find(line, pkg_root).is_none();
        if registry_is_trimmed && local_is_trimmed {
            continue;
        }

        #[cfg(target_os = "macos")]
        {
            // `OSO` symbols can't be trimmed at this moment.
            // See <https://github.com/rust-lang/rust/issues/116948#issuecomment-1793617018>
            if memchr::memmem::find(line, b" OSO ").is_some() {
                continue;
            }
        }

        panic!(
            "unexpected untrimmed symbol: {}",
            String::from_utf8(line.into()).unwrap()
        );
    }
}

// TODO: might want to move to test/testsuite/build_script.rs once stabilized.
#[cargo_test(nightly, reason = "--remap-path-scope will be stabilized in 1.95")]
fn custom_build_env_var_trim_paths() {
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
                edition = "2015"

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
                    let scope = std::env::var("CARGO_TRIM_PATHS_SCOPE").unwrap();
                    assert_eq!(scope.as_str(), "{expected}");

                    let remap = std::env::var_os("CARGO_TRIM_PATHS_REMAP");
                    if scope == "none" {{
                        assert_eq!(remap, None);
                    }} else {{
                        let remap = remap.unwrap();
                        let pairs: Vec<String> = std::env::split_paths(&remap)
                            .map(|p| p.into_os_string().into_string().unwrap())
                            .collect();
                        // package, build-dir, sysroot
                        assert_eq!(pairs.len(), 3, "remap = {{remap:?}}");
                        // The package lives at the workspace root, remapped to `.`.
                        assert!(pairs[0].ends_with("=."), "remap = {{remap:?}}");
                        assert!(pairs[1].ends_with("=/cargo/build-dir"), "remap = {{remap:?}}");
                        assert!(pairs[2].contains("=/rustc/"), "remap = {{remap:?}}");
                    }}
                }}
                "#
            ),
        );

        p.cargo("build -Ztrim-paths")
            .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
            .run();
    }
}

// This test is disabled, as it currently doesn't work due to issues with lldb.
#[cfg(any())]
#[cfg(unix)]
#[cargo_test(
    requires = "lldb",
    nightly,
    reason = "--remap-path-scope will be stabilized in 1.95"
)]
fn lldb_works_after_trimmed() {
    use cargo_test_support::compare::assert_e2e;
    use cargo_util::is_ci;

    if !is_ci() {
        // On macOS lldb requires elevated privileges to run developer tools.
        // See rust-lang/cargo#13413
        return;
    }

    let run_lldb = |path| {
        std::process::Command::new("lldb")
            .args(["-o", "breakpoint set --file src/main.rs --line 4"])
            .args(["-o", "run"])
            .args(["-o", "continue"])
            .args(["-o", "exit"])
            .arg("--no-use-colors")
            .arg(path)
            .output()
            .expect("lldb works")
    };

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let msg = "Hello, Ferris!";
                    println!("{msg}");
                }
            "#,
        )
        .build();

    p.cargo("build --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let bin_path = p.bin("foo");
    assert!(bin_path.is_file());
    let stdout = String::from_utf8(run_lldb(bin_path).stdout).unwrap();
    assert_e2e().eq(
        &stdout,
        str![[r#"
...
[..]stopped[..]
[..]stop reason = breakpoint 1.1[..]
...
(lldb) continue
Hello, Ferris!
...

"#]],
    );
}

#[cfg(all(target_os = "windows", target_env = "gnu", not(target_abi = "llvm")))]
#[cargo_test(requires = "gdb")]
fn gdb_works_after_trimmed() {
    use cargo_test_support::compare::assert_e2e;

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let msg = "Hello, Ferris!";
                    println!("{msg}");
                }
            "#,
        )
        .build();

    p.cargo("build --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let bin_path = p.bin("foo");
    assert!(bin_path.is_file());

    // GitHub's Windows runner uses MinGW-builds GDB wrapper,
    // which loses the boundary of spaced `-ex` args when forwarding them to `gdborig.exe`.
    // Therefore we use a command file instead here.
    //
    // See <https://github.com/niXman/mingw-builds/blob/1d6a1c28/sources/gdb-wrapper/gdb-wrapper.c#L141-L145>
    p.change_file(
        "gdb.commands",
        "break -source src/main.rs -line 4\nrun\nlist\ncontinue\n",
    );
    let stdout = String::from_utf8(
        p.process("gdb")
            .args(&["--batch", "--nx", "--quiet", "--command=gdb.commands"])
            .arg(bin_path.strip_prefix(p.root()).unwrap())
            .run()
            .stdout,
    )
    .unwrap();
    assert_e2e().eq(
        &stdout,
        str![[r#"
...
[..]Breakpoint 1,[..]
...
Hello, Ferris!
...

"#]],
    );
}

#[cfg(target_env = "msvc")]
#[cargo_test(
    requires = "cdb",
    nightly,
    reason = "--remap-path-scope will be stabilized in 1.95"
)]
fn cdb_works_after_trimmed() {
    use cargo_test_support::compare::assert_e2e;

    let run_debugger = |path| {
        std::process::Command::new("cdb")
            .args(["-lines", "-c", r"bp `src\main.rs:3`;g;g;q"])
            .arg(path)
            .output()
            .expect("debugger works")
    };

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let msg = "Hello, Ferris!";
                    println!("{msg}");
                }
            "#,
        )
        .build();

    p.cargo("build --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let bin_path = p.bin("foo");
    assert!(bin_path.is_file());
    let stdout = String::from_utf8(run_debugger(bin_path).stdout).unwrap();
    assert_e2e().eq(
        &stdout,
        str![[r#"
...
Breakpoint 0 hit
Hello, Ferris!
...

"#]],
    );
}

#[cargo_test(nightly, reason = "rustdoc --remap-path-prefix is unstable")]
fn rustdoc_without_diagnostics_scope() {
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
            /// </script>
            pub struct Bar;
            "#,
        )
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = "0.0.1"

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("doc -vv -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
...
[WARNING] unopened HTML tag `script`
 --> [ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.0.1/src/lib.rs:2:17
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "rustdoc --remap-path-prefix is unstable")]
fn rustdoc_diagnostics_works() {
    // This is expected to work after rust-lang/rust#128736
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
            /// </script>
            pub struct Bar;
            "#,
        )
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = "0.0.1"

                [profile.dev]
                trim-paths = "diagnostics"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("doc -vv -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
...
[RUNNING] `[..]rustc [..]--remap-path-scope=diagnostics --remap-path-prefix=[ROOT]/home/.cargo/registry/src= --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
...
[WARNING] unopened HTML tag `script`
 --> -[..]/bar-0.0.1/src/lib.rs:2:17
...
"#]])
        .run();
}
