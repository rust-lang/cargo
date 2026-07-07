//! Tests for the metabuild feature (declarative build scripts).

use std::str;

use crate::prelude::*;
use cargo_test_support::{
    Project, basic_lib_manifest, basic_manifest, is_coarse_mtime, project, registry::Package,
    rustc_host, str,
};

#[cargo_test]
fn metabuild_gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                metabuild = ["mb"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `metabuild` is required

  The package requires the Cargo feature called `metabuild`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["metabuild"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#metabuild for more information about the status of this feature.

"#]])
        .run();
}

fn basic_project() -> Project {
    project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                metabuild = ["mb", "mb-other"]

                [build-dependencies]
                mb = {path="mb"}
                mb-other = {path="mb-other"}
            "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .file(
            "mb-other/Cargo.toml",
            r#"
                [package]
                name = "mb-other"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file(
            "mb-other/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb-other"); }"#,
        )
        .build()
}

#[cargo_test]
fn metabuild_basic() {
    let p = basic_project();
    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_data(str![[r#"
[foo 0.0.1] Hello mb
[foo 0.0.1] Hello mb-other

"#]])
        .run();
}

#[cargo_test]
fn metabuild_error_both() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                metabuild = "mb"

                [build-dependencies]
                mb = {path="mb"}
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"fn main() {}"#)
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .build();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  cannot specify both `metabuild` and `build`

"#]])
        .run();
}

#[cargo_test]
fn metabuild_missing_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                metabuild = "mb"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  metabuild package `mb` must be specified in `build-dependencies`

"#]])
        .run();
}

#[cargo_test]
fn metabuild_optional_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                metabuild = "mb"

                [build-dependencies]
                mb = {path="mb", optional=true}
            "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .build();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_does_not_contain("[foo 0.0.1] Hello mb")
        .run();

    p.cargo("check -vv --features mb")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_contains("[foo 0.0.1] Hello mb")
        .run();
}

#[cargo_test]
fn metabuild_lib_name() {
    // Test when setting `name` on [lib].
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                metabuild = "mb"

                [build-dependencies]
                mb = {path="mb"}
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "mb/Cargo.toml",
            r#"
                [package]
                name = "mb"
                version = "0.0.1"
                edition = "2015"
                [lib]
                name = "other"
            "#,
        )
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .build();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_data(str![[r#"
[foo 0.0.1] Hello mb

"#]])
        .run();
}

#[cargo_test]
fn metabuild_fresh() {
    if is_coarse_mtime() {
        // This test doesn't work on coarse mtimes very well. Because the
        // metabuild script is created at build time, its mtime is almost
        // always equal to the mtime of the output. The second call to `build`
        // will then think it needs to be rebuilt when it should be fresh.
        return;
    }

    // Check that rebuild is fresh.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                metabuild = "mb"

                [build-dependencies]
                mb = {path="mb"}
            "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .build();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_contains("[foo 0.0.1] Hello mb")
        .run();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_does_not_contain("[foo 0.0.1] Hello mb")
        .with_stderr_data(str![[r#"
[FRESH] mb v0.5.0 ([ROOT]/foo/mb)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn metabuild_links() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                links = "cat"
                metabuild = "mb"

                [build-dependencies]
                mb = {path="mb"}
            "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"
                pub fn metabuild() {
                    assert_eq!(std::env::var("CARGO_MANIFEST_LINKS"),
                        Ok("cat".to_string()));
                    println!("Hello mb");
                }
            "#,
        )
        .build();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_data(str![[r#"
[foo 0.0.1] Hello mb

"#]])
        .run();
}

#[cargo_test]
fn metabuild_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                links = "cat"
                metabuild = "mb"

                [build-dependencies]
                mb = {path="mb"}
            "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { panic!("should not run"); }"#,
        )
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.{}.cat]
                    rustc-link-lib = ["a"]
                "#,
                rustc_host()
            ),
        )
        .build();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .run();
}

#[cargo_test]
fn metabuild_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["member1", "member2"]
            "#,
        )
        .file(
            "member1/Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "member1"
                version = "0.0.1"
                edition = "2015"
                metabuild = ["mb1", "mb2"]

                [build-dependencies]
                mb1 = {path="../../mb1"}
                mb2 = {path="../../mb2"}
            "#,
        )
        .file("member1/src/lib.rs", "")
        .file(
            "member2/Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "member2"
                version = "0.0.1"
                edition = "2015"
                metabuild = ["mb1"]

                [build-dependencies]
                mb1 = {path="../../mb1"}
            "#,
        )
        .file("member2/src/lib.rs", "")
        .build();

    project()
        .at("mb1")
        .file("Cargo.toml", &basic_lib_manifest("mb1"))
        .file(
            "src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb1 {}", std::env::var("CARGO_MANIFEST_DIR").unwrap()); }"#,
        )
        .build();

    project()
        .at("mb2")
        .file("Cargo.toml", &basic_lib_manifest("mb2"))
        .file(
            "src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb2 {}", std::env::var("CARGO_MANIFEST_DIR").unwrap()); }"#,
        )
        .build();

    p.cargo("check -vv --workspace")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_contains("[member1 0.0.1] Hello mb1 [..]member1")
        .with_stdout_contains("[member1 0.0.1] Hello mb2 [..]member1")
        .with_stdout_contains("[member2 0.0.1] Hello mb1 [..]member2")
        .with_stdout_does_not_contain("[member2 0.0.1] Hello mb2 [..]member2")
        .run();
}

#[cargo_test]
fn metabuild_metadata() {
    // The metabuild Target is filtered out of the `metadata` results.
    let p = basic_project();

    let meta = p
        .cargo("metadata --format-version=1")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .run_json();
    let mb_info: Vec<&str> = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"].as_str().unwrap() == "foo")
        .unwrap()["metabuild"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(mb_info, ["mb", "mb-other"]);
}

#[cargo_test]
fn metabuild_two_versions() {
    // Two versions of a metabuild dep with the same name.
    let p = project()
        .at("ws")
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["member1", "member2"]
            "#,
        )
        .file(
            "member1/Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "member1"
                version = "0.0.1"
                edition = "2015"
                metabuild = ["mb"]

                [build-dependencies]
                mb = {path="../../mb1"}
            "#,
        )
        .file("member1/src/lib.rs", "")
        .file(
            "member2/Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "member2"
                version = "0.0.1"
                edition = "2015"
                metabuild = ["mb"]

                [build-dependencies]
                mb = {path="../../mb2"}
            "#,
        )
        .file("member2/src/lib.rs", "")
        .build();

    project().at("mb1")
        .file("Cargo.toml", r#"
            [package]
            name = "mb"
            version = "0.0.1"
            edition = "2015"
        "#)
        .file(
            "src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb1 {}", std::env::var("CARGO_MANIFEST_DIR").unwrap()); }"#,
        )
        .build();

    project().at("mb2")
        .file("Cargo.toml", r#"
            [package]
            name = "mb"
            version = "0.0.2"
            edition = "2015"
        "#)
        .file(
            "src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb2 {}", std::env::var("CARGO_MANIFEST_DIR").unwrap()); }"#,
        )
        .build();

    p.cargo("check -vv --workspace")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_data(
            str![[r#"
[member2 0.0.1] Hello mb2 [ROOT]/ws/member2
[member1 0.0.1] Hello mb1 [ROOT]/ws/member1

"#]]
            .unordered(),
        )
        .run();

    assert_eq!(
        p.glob("target/.metabuild/metabuild-member?-*.rs").count(),
        2
    );
}

#[cargo_test]
fn metabuild_external_dependency() {
    Package::new("mb", "1.0.0")
        .file("Cargo.toml", &basic_manifest("mb", "1.0.0"))
        .file(
            "src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .publish();
    Package::new("dep", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["metabuild"]
                [package]
                name = "dep"
                version = "1.0.0"
                edition = "2015"
                metabuild = ["mb"]

                [build-dependencies]
                mb = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build_dep("mb", "1.0.0")
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
            dep = "1.0"
            "#,
        )
        .file("src/lib.rs", "extern crate dep;")
        .build();

    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_data(str![[r#"
[dep 1.0.0] Hello mb

"#]])
        .run();

    assert_eq!(p.glob("target/.metabuild/metabuild-dep-*.rs").count(), 1);
}

#[cargo_test]
fn metabuild_json_artifact() {
    let p = basic_project();
    p.cargo("check --message-format=json")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_stdout_data(
            str![[r#"
[
  "{...}",
  {
    "executable": null,
    "features": [],
    "filenames": "{...}",
    "fresh": false,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.0.1",
    "profile": "{...}",
    "reason": "compiler-artifact",
    "target": {
      "crate_types": [
        "bin"
      ],
      "doc": false,
      "doctest": false,
      "edition": "2018",
      "kind": [
        "custom-build"
      ],
      "name": "metabuild-foo",
      "src_path": "[ROOT]/foo/target/.metabuild/metabuild-foo-[HASH].rs",
      "test": false
    }
  },
  {
    "cfgs": [],
    "env": [],
    "linked_libs": [],
    "linked_paths": [],
    "out_dir": "[ROOT]/foo/target/debug/build/foo-[HASH]/out",
    "package_id": "path+[ROOTURL]/foo#0.0.1",
    "reason": "build-script-executed"
  },
  "{...}"
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}

#[cargo_test]
fn metabuild_failed_build_json() {
    let p = basic_project();
    // Modify the metabuild dep so that it fails to compile.
    p.change_file("mb/src/lib.rs", "");
    p.cargo("check --message-format=json")
        .masquerade_as_nightly_cargo(&["metabuild"])
        .with_status(101)
        .with_stdout_data(
            str![[r#"
[
  "{...}",
  {
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "message": {
      "level": "error",
      "message": "cannot find function `metabuild` in crate `mb`",
      "...": "{...}"
    },
    "package_id": "path+[ROOTURL]/foo#0.0.1",
    "reason": "compiler-message",
    "target": {
      "crate_types": [
        "bin"
      ],
      "doc": false,
      "doctest": false,
      "edition": "2018",
      "kind": [
        "custom-build"
      ],
      "name": "metabuild-foo",
      "src_path": null,
      "test": false
    }
  },
  "{...}"
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}
