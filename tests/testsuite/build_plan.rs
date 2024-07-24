//! Tests for --build-plan feature.

use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_bin_manifest, basic_manifest, main_file, project, str};

#[cargo_test]
fn cargo_build_plan_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --build-plan -Zunstable-options")
        .masquerade_as_nightly_cargo(&["build-plan"])
        .with_stdout_data(
            str![[r#"
{
  "inputs": [
    "[ROOT]/foo/Cargo.toml"
  ],
  "invocations": [
    {
      "args": "{...}",
      "compile_mode": "build",
      "cwd": "[ROOT]/foo",
      "deps": [],
      "env": "{...}",
      "kind": null,
      "links": "{...}",
      "outputs": "{...}",
      "package_name": "foo",
      "package_version": "0.5.0",
      "program": "rustc",
      "target_kind": [
        "bin"
      ]
    }
  ]
}
"#]]
            .is_json(),
        )
        .run();
    assert!(!p.bin("foo").is_file());
}

#[cargo_test]
fn cargo_build_plan_single_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.5.0"

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;
                pub fn foo() { bar::bar(); }

                #[test]
                fn test() { foo(); }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();
    p.cargo("build --build-plan -Zunstable-options")
        .masquerade_as_nightly_cargo(&["build-plan"])
        .with_stdout_data(
            str![[r#"
{
  "inputs": [
    "[ROOT]/foo/Cargo.toml",
    "[ROOT]/foo/bar/Cargo.toml"
  ],
  "invocations": [
    {
      "args": "{...}",
      "compile_mode": "build",
      "cwd": "[ROOT]/foo",
      "deps": [],
      "env": "{...}",
      "kind": null,
      "links": {},
      "outputs": [
        "[ROOT]/foo/target/debug/deps/libbar-[HASH].rlib",
        "[ROOT]/foo/target/debug/deps/libbar-[HASH].rmeta"
      ],
      "package_name": "bar",
      "package_version": "0.0.1",
      "program": "rustc",
      "target_kind": [
        "lib"
      ]
    },
    {
      "args": "{...}",
      "compile_mode": "build",
      "cwd": "[ROOT]/foo",
      "deps": [
        0
      ],
      "env": "{...}",
      "kind": null,
      "links": "{...}",
      "outputs": [
        "[ROOT]/foo/target/debug/deps/libfoo-[HASH].rlib",
        "[ROOT]/foo/target/debug/deps/libfoo-[HASH].rmeta"
      ],
      "package_name": "foo",
      "package_version": "0.5.0",
      "program": "rustc",
      "target_kind": [
        "lib"
      ]
    }
  ]
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn cargo_build_plan_build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", r#"fn main() {}"#)
        .file("build.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build --build-plan -Zunstable-options")
        .masquerade_as_nightly_cargo(&["build-plan"])
        .with_stdout_data(
            str![[r#"
{
  "inputs": [
    "[ROOT]/foo/Cargo.toml"
  ],
  "invocations": [
    {
      "args": "{...}",
      "compile_mode": "build",
      "cwd": "[ROOT]/foo",
      "deps": [],
      "env": "{...}",
      "kind": null,
      "links": "{...}",
      "outputs": "{...}",
      "package_name": "foo",
      "package_version": "0.5.0",
      "program": "rustc",
      "target_kind": [
        "custom-build"
      ]
    },
    {
      "args": "{...}",
      "compile_mode": "run-custom-build",
      "cwd": "[ROOT]/foo",
      "deps": [
        0
      ],
      "env": "{...}",
      "kind": null,
      "links": {},
      "outputs": [],
      "package_name": "foo",
      "package_version": "0.5.0",
      "program": "[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build",
      "target_kind": [
        "custom-build"
      ]
    },
    {
      "args": "{...}",
      "compile_mode": "build",
      "cwd": "[ROOT]/foo",
      "deps": [
        1
      ],
      "env": "{...}",
      "kind": null,
      "links": "{...}",
      "outputs": "{...}",
      "package_name": "foo",
      "package_version": "0.5.0",
      "program": "rustc",
      "target_kind": [
        "bin"
      ]
    }
  ]
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn build_plan_with_dev_dep() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dev-dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --build-plan -Zunstable-options")
        .masquerade_as_nightly_cargo(&["build-plan"])
        .run();
}
