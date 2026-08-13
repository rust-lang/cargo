//! Tests for `-Ztrim-paths`.

use crate::prelude::*;
use cargo_test_support::basic_manifest;
use cargo_test_support::compare::assert_e2e;
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

#[cargo_test]
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

#[cargo_test]
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

#[cargo_test]
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

#[cargo_test]
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

#[cargo_test]
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
/cargo/registry/[..]/bar-0.0.1/src/lib.rs

"#]]) // Omit the hash of Source URL
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/home/.cargo/registry/src/[..]=/cargo/registry/[..] --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    // Unremap files for both original exe and uplifted exe.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert_e2e().eq(
        &std::fs::read_to_string(&unremap_file).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": ".",
    "to": "[ROOT]/foo"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/cargo/registry/[..]",
    "to": "[ROOT]/home/.cargo/registry/src/-[HASH]"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
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
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
[RUNNING] `rustc --crate-name build_script_build [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/home/.cargo/registry/src/[..]=/cargo/registry/[..] --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[RUNNING] `[ROOT]/foo/target/debug/build/bar-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name bar [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/home/.cargo/registry/src/[..]=/cargo/registry/[..] --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    // Unremap files for both original exe and uplifted exe.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert!(unremap_file.exists());
}

#[cargo_test]
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
/cargo/git/[..]/src/lib.rs

"#]]) // Omit the hash of Source URL and commit
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to highest compatible version
[COMPILING] bar v0.0.1 ([ROOTURL]/bar#[..])
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/home/.cargo/git/checkouts/bar-[..]=/cargo/git/[..] --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    // Unremap files for both original exe and uplifted exe.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert_e2e().eq(
        &std::fs::read_to_string(&unremap_file).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": ".",
    "to": "[ROOT]/foo"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/cargo/git/[..]",
    "to": "[ROOT]/home/.cargo/git/checkouts/bar-[..]"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
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
[LOCKING] 1 package to highest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/cocktail-bar)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    // Unremap files for both original exe and uplifted exe.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert_e2e().eq(
        &std::fs::read_to_string(&unremap_file).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": ".",
    "to": "[ROOT]/foo"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
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
/cargo/deps/bar-0.0.1/src/lib.rs

"#]])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/bar)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/bar=/cargo/deps/bar-0.0.1 --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    // Unremap files for both original exe and uplifted exe.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert_e2e().eq(
        &std::fs::read_to_string(&unremap_file).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": ".",
    "to": "[ROOT]/foo"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/cargo/deps/bar-0.0.1",
    "to": "[ROOT]/bar"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn vendored_dependencies() {
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", r#"pub fn f() { println!("{}", file!()); }"#)
        .publish();
    let git_project = git::new("baz", |project| {
        project
            .file("Cargo.toml", &basic_manifest("baz", "0.0.1"))
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
                bar = "0.0.1"
                baz = {{ git = "{url}" }}

                [profile.dev]
                trim-paths = "object"
           "#
            ),
        )
        .file("src/main.rs", "fn main() { bar::f(); baz::f(); }")
        .build();

    p.cargo("vendor --respect-source-config -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();
    p.change_file(
        ".cargo/config.toml",
        &format!(
            r#"
            [source."git+{url}"]
            git = "{url}"
            replace-with = "vendored-sources"

            [source.crates-io]
            replace-with = "vendored-sources"

            [source.vendored-sources]
            directory = "vendor"
            "#
        ),
    );

    // Vendored deps within the workspace are remapped as local packages
    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(
            str![[r#"
[COMPILING] bar v0.0.1
[COMPILING] baz v0.0.1 ([ROOTURL]/baz#[..])
[RUNNING] `rustc --crate-name bar [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[RUNNING] `rustc --crate-name baz [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]]
            .unordered(),
        )
        .with_stdout_data(str![[r#"
./vendor/bar/src/lib.rs
./vendor/baz/src/lib.rs

"#]])
        .run();

    // Unremap files for both original exe and uplifted exe.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert_e2e().eq(
        &std::fs::read_to_string(&unremap_file).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": ".",
    "to": "[ROOT]/foo"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn vendored_dependencies_outside_workspace() {
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", r#"pub fn f() { println!("{}", file!()); }"#)
        .publish();
    let git_project = git::new("baz", |project| {
        project
            .file("Cargo.toml", &basic_manifest("baz", "0.0.1"))
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
                bar = "0.0.1"
                baz = {{ git = "{url}" }}

                [profile.dev]
                trim-paths = "object"
           "#
            ),
        )
        .file("src/main.rs", "fn main() { bar::f(); baz::f(); }")
        .build();

    p.cargo("vendor --respect-source-config -Ztrim-paths ../shared-vendor")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();
    p.change_file(
        ".cargo/config.toml",
        &format!(
            r#"
            [source."git+{url}"]
            git = "{url}"
            replace-with = "vendored-sources"

            [source.crates-io]
            replace-with = "vendored-sources"

            [source.vendored-sources]
            directory = '{}'
            "#,
            paths::root().join("shared-vendor").display()
        ),
    );

    // Vendored deps outside the workspace are remapped as path dependencies
    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(
            str![[r#"
[COMPILING] bar v0.0.1
[COMPILING] baz v0.0.1 ([ROOTURL]/baz#[..])
[RUNNING] `rustc --crate-name bar [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/shared-vendor/bar=/cargo/deps/bar-0.0.1 --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[RUNNING] `rustc --crate-name baz [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/shared-vendor/baz=/cargo/deps/baz-0.0.1 --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]]
            .unordered(),
        )
        .with_stdout_data(str![[r#"
/cargo/deps/bar-0.0.1/src/lib.rs
/cargo/deps/baz-0.0.1/src/lib.rs

"#]])
        .run();

    // Unremap files for both original exe and uplifted exe.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert_e2e().eq(
        &std::fs::read_to_string(&unremap_file).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": ".",
    "to": "[ROOT]/foo"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/cargo/deps/bar-0.0.1",
    "to": "[ROOT]/shared-vendor/bar"
  },
  {
    "from": "/cargo/deps/baz-0.0.1",
    "to": "[ROOT]/shared-vendor/baz"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn local_package_with_build_script_codegen() {
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
            "src/main.rs",
            r#"
            include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
            fn main() { println!("{}", my_file()); }
            "#,
        )
        .build();

    // The build-dir rule is passed last
    // so paths should be remapped to `/cargo/build-dir`
    p.cargo("run --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stdout_data(str![[r#"
/cargo/build-dir/debug/build/foo-[HASH]/out/bindings.rs

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name build_script_build [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name foo [..]--remap-path-scope=object --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    // Unremap files for both original exe and uplifted exe.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert!(unremap_file.exists());
}

#[cargo_test]
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
[RUNNING] `[..] rustc [..]--remap-path-scope=diagnostics --remap-path-prefix=[ROOT]/home/.cargo/registry/src/[..]=/cargo/registry/[..] --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[WARNING] unused variable: `unused`
...
[RUNNING] `[..] rustc [..]--remap-path-scope=diagnostics --remap-path-prefix=[ROOT]/foo=. --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
...
"#]])
        .run();

    // Non `object` scope never emits unremap files.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 0);
}

#[cfg(target_os = "macos")]
mod object_works {
    use super::*;

    fn inspect_debuginfo(path: &std::path::Path) -> Vec<u8> {
        let mut command = std::process::Command::new("nm");
        command.arg("-pa").arg(path);
        command_output(&mut command, "nm").stdout
    }

    #[cargo_test(requires = "nm")]
    fn with_split_debuginfo_off() {
        object_works_helper("off", inspect_debuginfo);
    }

    #[cargo_test(requires = "nm")]
    fn with_split_debuginfo_packed() {
        object_works_helper("packed", inspect_debuginfo);
    }

    #[cargo_test(requires = "nm")]
    fn with_split_debuginfo_unpacked() {
        object_works_helper("unpacked", inspect_debuginfo);
    }
}

#[cfg(target_os = "linux")]
mod object_works {
    use super::*;

    fn inspect_debuginfo(path: &std::path::Path) -> Vec<u8> {
        let mut command = std::process::Command::new("readelf");
        command
            .arg("--debug-dump=info")
            .arg("--debug-dump=no-follow-links") // older version can't recognized but just a warning
            .arg(path);
        command_output(&mut command, "readelf").stdout
    }

    #[cargo_test(requires = "readelf")]
    fn with_split_debuginfo_off() {
        object_works_helper("off", inspect_debuginfo);
    }

    // Some Linux targets, such as RISC-V, only support `off`.
    // See https://github.com/rust-lang/cargo/issues/17255.
    #[cargo_test(requires = "readelf", requires_host_split_debuginfo = "packed")]
    fn with_split_debuginfo_packed() {
        object_works_helper("packed", inspect_debuginfo);
    }

    #[cargo_test(requires = "readelf", requires_host_split_debuginfo = "unpacked")]
    fn with_split_debuginfo_unpacked() {
        object_works_helper("unpacked", inspect_debuginfo);
    }
}

#[cfg(target_env = "msvc")]
mod object_works {
    use super::*;

    fn inspect_debuginfo(path: &std::path::Path) -> Vec<u8> {
        let mut command = std::process::Command::new("strings");
        command.arg(path);
        command_output(&mut command, "strings").stdout
    }

    // windows-msvc supports split-debuginfo=packed only
    #[cargo_test(requires = "strings")]
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
        let mut command = std::process::Command::new("objdump");
        // Avoid echoing the absolute input path in objdump's file header.
        command
            .current_dir(parent)
            .arg("--dwarf=info")
            .arg(file_name);
        command_output(&mut command, "objdump").stdout
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
    --remap-path-prefix=[ROOT]/home/.cargo/registry/src/[..]=/cargo/registry/[..] \
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
#[cargo_test]
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

#[cfg(unix)]
#[cargo_test(requires = "lldb")]
fn lldb_works_after_trimmed() {
    use cargo_test_support::compare::assert_e2e;

    #[cfg(target_os = "macos")]
    if !cargo_util::is_ci() {
        // On macOS lldb requires elevated privileges to run developer tools.
        // See rust-lang/cargo#13413
        return;
    }

    let run_lldb = |path| {
        let mut command = std::process::Command::new("lldb");
        command
            .args(["--batch", "--no-lldbinit"])
            .args([
                "-o",
                "breakpoint set --one-shot true --file src/main.rs --line 4",
            ])
            .args(["-o", "run"])
            .args(["-o", "continue"])
            .arg("--no-use-colors")
            .arg(path);
        command_output(&mut command, "lldb")
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
(lldb) breakpoint set --one-shot true --file src/main.rs --line 4
Breakpoint 1: [..]locations.
(lldb) run
...
[..]stopped[..]
[..]stop reason = one-shot breakpoint 1[..]
...
(lldb) continue
...
Hello, Ferris!
...

"#]],
    );
}

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", target_env = "gnu", not(target_abi = "llvm"))
))]
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
#[cargo_test(requires = "cdb")]
fn cdb_works_after_trimmed() {
    use cargo_test_support::compare::assert_e2e;

    let run_debugger = |path| {
        let mut command = std::process::Command::new("cdb");
        command
            .args(["-lines", "-c", r"bp `src\main.rs:3`;g;g;q"])
            .arg(path);
        command_output(&mut command, "cdb")
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

#[cargo_test]
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

#[cargo_test]
fn rustdoc_diagnostics_works() {
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
[RUNNING] `[..]rustc [..]--remap-path-scope=diagnostics --remap-path-prefix=[ROOT]/home/.cargo/registry/src/[..]=/cargo/registry/[..] --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
...
[WARNING] unopened HTML tag `script`
 --> /cargo/registry/[HASH]/bar-0.0.1/src/lib.rs:2:17
...
"#]])
        .run();
}

fn command_output(command: &mut std::process::Command, name: &str) -> std::process::Output {
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("{name} failed to start: {err}"));
    assert!(
        output.status.success(),
        "{name} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

#[cargo_test]
fn workspace_remap_with_root_dir() {
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

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file("src/main.rs", "fn main() { bar::f(); }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn f() {}")
        .build();

    p.cargo("build --verbose -Ztrim-paths -Zroot-dir=..")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths", "-Zroot-dir"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]=. --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-scope=object --remap-path-prefix=[ROOT]=. --remap-path-prefix=[ROOT]/foo/target=/cargo/build-dir --remap-path-prefix=[..]/lib/rustlib/src/rust=/rustc/[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert_e2e().eq(
        &std::fs::read_to_string(&unremap_file).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": ".",
    "to": "[ROOT]"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn workspace_prefix_override_from_env() {
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
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --verbose -Ztrim-paths")
        .env("__CARGO_RUSTC_BOOTSTRAP_WS_REMAP", "/rustc-dev/1111111")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-prefix=[ROOT]/foo=/rustc-dev/1111111 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert_e2e().eq(
        &std::fs::read_to_string(&unremap_file).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/rustc-dev/1111111",
    "to": "[ROOT]/foo"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
}

#[cargo_test]
fn workspace_prefix_override_fingerprint() {
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
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Ztrim-paths")
        .env("__CARGO_RUSTC_BOOTSTRAP_WS_REMAP", "/rustc-dev/1111111")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    p.cargo("build --verbose -Ztrim-paths")
        .env("__CARGO_RUSTC_BOOTSTRAP_WS_REMAP", "/rustc-dev/2222222")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the profile configuration changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--remap-path-prefix=[ROOT]/foo=/rustc-dev/2222222 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unremap_file_rebuild() {
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
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();
    assert!(p.bin("foo").is_file());
    let unremap_file = unremap_file_path(&p.bin("foo"));
    assert!(unremap_file.exists());

    // Deleting the uplifted copy won't cause rebuild.
    std::fs::remove_file(&unremap_file).unwrap();
    p.cargo("build --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(unremap_file.exists());

    // Deleting the original one will cause rebuild.
    // The non-uplifted copy is the one that is not `unremap_file`,
    // as its file name layout varies across platforms.
    let deps_file = p
        .glob("target/**/*.trim-paths.jsonl")
        .map(|f| f.unwrap())
        .find(|f| *f != unremap_file)
        .unwrap();
    std::fs::remove_file(&deps_file).unwrap();
    p.cargo("build --verbose -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): couldn't read metadata for file `target/debug/[..]/foo[..].trim-paths.jsonl`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(deps_file.exists());
    assert!(unremap_file.exists());
}

#[cargo_test]
fn unremap_file_without_debuginfo() {
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
                debug = 0
           "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    // No debuginfo. No unremap file.
    assert!(p.bin("foo").is_file());
    assert!(!unremap_file_path(&p.bin("foo")).exists());
}

#[cargo_test]
fn unremap_file_with_cargo_clean() {
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
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    assert!(unremap_file_path(&p.bin("foo")).exists());
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);

    p.cargo("clean -p foo -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    assert!(!unremap_file_path(&p.bin("foo")).exists());
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 0);

    // Test the new layout

    p.cargo("clean -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    p.cargo("build -Ztrim-paths -Zbuild-dir-new-layout")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    assert!(unremap_file_path(&p.bin("foo")).exists());
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 2);

    p.cargo("clean -p foo -Ztrim-paths -Zbuild-dir-new-layout")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    assert!(!unremap_file_path(&p.bin("foo")).exists());
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 0);
}

// MSVC always emits a PDB when debuginfo is on (which the unremap file requires),
// It adds a third `filenames` entry in JSON message.
// Skip to make snapshot's life easier.
#[cfg(not(target_env = "msvc"))]
#[cargo_test]
fn unremap_file_in_json_messages() {
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
                # Suppress the platform-default dSYM on macOS so that `filenames`
                # in JSON message is identical on all non-MSVC platforms.
                split-debuginfo = "off"
           "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Ztrim-paths --message-format=json")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_stdout_data(
            str![[r#"
[
  {
    "executable": "[ROOT]/foo/target/debug/foo[EXE]",
    "features": [],
    "filenames": [
      "[ROOT]/foo/target/debug/foo[EXE]",
      "[ROOT]/foo/target/debug/foo[EXE].trim-paths.jsonl"
    ],
    "fresh": false,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.0.1",
    "profile": "{...}",
    "reason": "compiler-artifact",
    "target": "{...}"
  },
  {
    "reason": "build-finished",
    "success": true
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}

#[cargo_test]
fn unremap_file_with_artifact_dir() {
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
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Ztrim-paths -Zunstable-options --artifact-dir out")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths", "unstable-options"])
        .run();

    let exported = p
        .root()
        .join("out")
        .join(format!("foo{}", std::env::consts::EXE_SUFFIX));
    assert!(exported.is_file());
    assert!(unremap_file_path(&exported).exists());
}

#[cargo_test]
fn unremap_file_for_all_bin_types() {
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
        .file("src/lib.rs", "#[test] fn t() {}")
        .file("tests/it.rs", "#[test] fn t() {}")
        .file("examples/ex.rs", "fn main() {}")
        .build();

    p.cargo("test --no-run -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    // Unit test, integration test, and example binaries are all root units
    // and receive unremap files.
    assert_eq!(p.glob("target/**/foo-*.trim-paths.jsonl").count(), 1);
    assert_eq!(p.glob("target/**/it-*.trim-paths.jsonl").count(), 1);
    // MSVC executables don't get a hashed filename
    // The PDB path is embedded in the executable.
    let expected = if cfg!(target_env = "msvc") { 1 } else { 2 };
    assert_eq!(
        p.glob("target/debug/examples/*.trim-paths.jsonl").count(),
        expected
    );
}

#[cargo_test]
fn unremap_file_with_multiple_crate_types() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lib]
                crate-type = ["cdylib", "staticlib"]

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    // Unremap files for both original cdylib/staticlib and uplifted ones.
    assert_eq!(p.glob("target/**/*.trim-paths.jsonl").count(), 4);
    let uplifted: Vec<_> = p
        .glob("target/debug/*.trim-paths.jsonl")
        .map(|f| f.unwrap())
        .collect();
    assert_eq!(uplifted.len(), 2);
    let contents: Vec<_> = uplifted
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap())
        .collect();
    assert_eq!(contents[0], contents[1]);
}

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", target_env = "gnu", not(target_abi = "llvm"))
))]
#[cargo_test(requires = "gdb")]
fn unremap_file_works_in_gdb() {
    let p = unremap_debugger_project();
    let subs = unremap_substitutions(&p.bin("foo"));

    let breakpoints = [
        "break -source src/main.rs -line 5",
        "break bar::hello",
        "break baz::hi",
        "break foo::generated",
        "run",
        "continue",
        "continue",
        "continue",
        "continue",
        "",
    ]
    .join("\n");

    let run_gdb = |commands_file: &str| {
        let stdout = p
            .process("gdb")
            .args(&["--batch", "--nx", "--quiet", "--command"])
            .arg(&p.root().join(commands_file))
            .arg(&p.bin("foo"))
            // We set to test root rather than package root
            // so that we can exercise remap of ws-root -> `.`
            .cwd(paths::root())
            .run()
            .stdout;
        String::from_utf8(stdout).unwrap()
    };

    // No remap: breakpoints hit, but no source marker is shown.
    p.change_file("gdb.commands", &breakpoints);
    let stdout = run_gdb("gdb.commands");
    for marker in UNREMAP_MARKERS {
        assert!(
            !stdout.contains(marker),
            "unexpected `{marker}` in:\n{stdout}"
        );
    }

    let subs: String = subs
        .iter()
        .map(|(from, to)| format!("set substitute-path \"{from}\" \"{to}\"\n"))
        .collect();
    p.change_file("gdb-unremap.commands", &format!("{subs}{breakpoints}"));
    let stdout = run_gdb("gdb-unremap.commands");
    for marker in UNREMAP_MARKERS {
        assert!(stdout.contains(marker), "missing `{marker}` in:\n{stdout}");
    }
}

#[cfg(unix)]
#[cargo_test(requires = "lldb")]
fn unremap_file_works_in_lldb() {
    #[cfg(target_os = "macos")]
    if !cargo_util::is_ci() {
        // On macOS lldb requires elevated privileges to run developer tools.
        // See rust-lang/cargo#13413
        return;
    }

    let p = unremap_debugger_project();
    let subs = unremap_substitutions(&p.bin("foo"));

    let breakpoints = [
        "breakpoint set --file src/main.rs --line 5",
        "breakpoint set --name bar::hello",
        "breakpoint set --name baz::hi",
        "breakpoint set --name foo::generated",
        "run",
        "continue",
        "continue",
        "continue",
        "",
    ]
    .join("\n");

    let run_lldb = |commands_file: &str| {
        let stdout = p
            .process("lldb")
            .args(&["--batch", "--no-lldbinit", "--no-use-colors"])
            // If without rust-src component,
            // sysroot remap will not be found and lldb errors on that.
            // Set this to makes lldb continue.
            .args(&[
                "-O",
                "settings set interpreter.stop-command-source-on-error false",
            ])
            .arg("--source")
            .arg(&p.root().join(commands_file))
            .arg(&p.bin("foo"))
            // We set to test root rather than package root
            // so that we can exercise remap of ws-root -> `.`
            .cwd(paths::root())
            .run()
            .stdout;
        String::from_utf8(stdout).unwrap()
    };

    // No remap: breakpoints hit, but no source marker is shown.
    p.change_file("lldb.commands", &breakpoints);
    let stdout = run_lldb("lldb.commands");
    for marker in UNREMAP_MARKERS {
        assert!(
            !stdout.contains(marker),
            "unexpected `{marker}` in:\n{stdout}"
        );
    }

    let source_map = format!(
        "settings append target.source-map{}",
        subs.iter()
            .map(|(from, to)| format!(r#" "{from}" "{to}""#))
            .collect::<String>()
    );
    p.change_file(
        "lldb-unremap.commands",
        &format!("{source_map}\n{breakpoints}"),
    );
    let stdout = run_lldb("lldb-unremap.commands");
    // Expect to find all markers.
    for marker in UNREMAP_MARKERS {
        assert!(stdout.contains(marker), "missing `{marker}` in:\n{stdout}");
    }
}

fn unremap_file_path(artifact: &std::path::Path) -> std::path::PathBuf {
    let mut path = artifact.as_os_str().to_owned();
    path.push(".trim-paths.jsonl");
    path.into()
}

/// Source markers on breakpoints.
#[cfg(any(
    unix,
    all(target_os = "windows", target_env = "gnu", not(target_abi = "llvm"))
))]
const UNREMAP_MARKERS: &[&str] = &[
    "TRIM_PATHS_ROOT_MARKER",
    "TRIM_PATHS_REGISTRY_MARKER",
    "TRIM_PATHS_PATH_DEP_MARKER",
    "TRIM_PATHS_BUILD_DIR_MARKER",
];

/// Builds a `-Ztrim-paths` project covering several remap kinds via [`UNREMAP_MARKERS`].
#[cfg(any(
    unix,
    all(target_os = "windows", target_env = "gnu", not(target_abi = "llvm"))
))]
fn unremap_debugger_project() -> cargo_test_support::Project {
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                pub fn hello() {
                    println!("in registry dep"); // TRIM_PATHS_REGISTRY_MARKER
                }
            "#,
        )
        .publish();

    let _baz = project()
        .at("baz")
        .file("Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file(
            "src/lib.rs",
            r#"
                pub fn hi() {
                    println!("in path dep"); // TRIM_PATHS_PATH_DEP_MARKER
                }
            "#,
        )
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
                bar = "0.0.1"
                baz = { path = "../baz" }

                [profile.dev]
                trim-paths = "object"
           "#,
        )
        .file(
            "build.rs",
            r##"
                fn main() {
                    let out_dir = std::env::var("OUT_DIR").unwrap();
                    let gen = r#"
                pub fn generated() {
                    println!("in generated code"); // TRIM_PATHS_BUILD_DIR_MARKER
                }
            "#;
                    std::fs::write(std::path::Path::new(&out_dir).join("gen.rs"), gen).unwrap();
                }
            "##,
        )
        // Line numbers matter: breakpoints are set at line 5.
        .file(
            "src/main.rs",
            r#"
                include!(concat!(env!("OUT_DIR"), "/gen.rs"));

                fn main() {
                    println!("in root package"); // TRIM_PATHS_ROOT_MARKER
                    bar::hello();
                    baz::hi();
                    generated();
                }
            "#,
        )
        .build();

    p.cargo("build -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .run();

    assert_e2e().eq(
        &std::fs::read_to_string(unremap_file_path(&p.bin("foo"))).unwrap(),
        str![[r#"
[
  {
    "v": 1
  },
  {
    "rust_version": "[..]",
    "workspace_root": "[ROOT]/foo"
  },
  {
    "from": ".",
    "to": "[ROOT]/foo"
  },
  {
    "from": "/cargo/build-dir",
    "to": "[ROOT]/foo/target"
  },
  {
    "from": "/cargo/deps/baz-0.0.1",
    "to": "[ROOT]/baz"
  },
  {
    "from": "/cargo/registry/[..]",
    "to": "[ROOT]/home/.cargo/registry/src/-[HASH]"
  },
  {
    "from": "/rustc/[..]",
    "to": "[..]/lib/rustlib/src/rust"
  }
]
"#]]
        .is_json()
        .against_jsonlines(),
    );
    p
}

/// Parses an unremap file into the substitution pairs for a debugger to consume.
#[cfg(any(
    unix,
    all(target_os = "windows", target_env = "gnu", not(target_abi = "llvm"))
))]
fn unremap_substitutions(artifact: &std::path::Path) -> Vec<(String, String)> {
    let content = std::fs::read_to_string(unremap_file_path(artifact)).unwrap();
    let mut values = serde_json::Deserializer::from_str(&content).into_iter::<serde_json::Value>();

    let version = values.next().unwrap().unwrap();
    assert_eq!(version["v"], 1);

    let _metadata = values.next().unwrap().unwrap();

    let mut pairs = Vec::new();

    for record in values {
        let record = record.unwrap();
        pairs.push((
            record["from"].as_str().unwrap().to_owned(),
            record["to"].as_str().unwrap().to_owned(),
        ));
    }

    pairs
}
