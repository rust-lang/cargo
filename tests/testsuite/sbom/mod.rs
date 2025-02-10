//! Tests for cargo-sbom precursor files.

use std::path::PathBuf;

use cargo_test_support::basic_bin_manifest;
use cargo_test_support::cargo_test;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use snapbox::IntoData;

const SBOM_FILE_EXTENSION: &str = ".cargo-sbom.json";

fn with_sbom_suffix(link: &PathBuf) -> PathBuf {
    let mut link_buf = link.clone().into_os_string();
    link_buf.push(SBOM_FILE_EXTENSION);
    PathBuf::from(link_buf)
}

#[cargo_test]
fn warn_without_passing_unstable_flag() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["sbom"])
        .with_stderr_data(
            "\
            [WARNING] ignoring 'sbom' config, pass `-Zsbom` to enable it\n\
            [COMPILING] foo v0.5.0 ([..])\n\
            [FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();

    let file = with_sbom_suffix(&p.bin("foo"));
    assert!(!file.exists());
}

#[cargo_test]
fn simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build -Zsbom")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let file = with_sbom_suffix(&p.bin("foo"));
    let output = std::fs::read_to_string(file).unwrap();
    // The expected test does contain the "rustc" and "profile", sections
    // but other tests omit them for brevity.
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "packages": [
    {
      "cfgs": [],
      "dependencies": [],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.5.0",
      "profile": {
        "debug_assertions": true,
        "debuginfo": 2,
        "lto": "false",
        "name": "dev",
        "opt_level": "0",
        "overflow_checks": true,
        "panic": "unwind",
        "rpath": false
      }
    }
  ],
  "root": 0,
  "rustc": {
    "commit_hash": "{...}",
    "host": "[HOST_TARGET]",
    "verbose_version": "{...}",
    "version": "{...}",
    "workspace_wrapper": null,
    "wrapper": null
  },
  "version": 1
}
"#]]
        .is_json(),
    );
}

#[cargo_test]
fn with_multiple_crate_types() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.2.3"

                [lib]
                crate-type = ["dylib", "rlib"]
            "#,
        )
        .file("src/main.rs", r#"fn main() { let _i = foo::give_five(); }"#)
        .file("src/lib.rs", r#"pub fn give_five() -> i32 { 5 }"#)
        .build();

    p.cargo("build -Zsbom")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    assert_eq!(
        3,
        p.glob(p.target_debug_dir().join("*.cargo-sbom.json"))
            .count()
    );

    let sbom_path = with_sbom_suffix(&p.dylib("foo"));
    assert!(sbom_path.is_file());

    let output = std::fs::read_to_string(sbom_path).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "packages": [
    {
      "cfgs": [],
      "dependencies": [],
      "features": [],
      "id": "path+[ROOTURL]/foo#1.2.3",
      "profile": "{...}"
    }
  ],
  "root": 0,
  "rustc": "{...}",
  "version": 1
}
"#]]
        .is_json(),
    );
}

#[cargo_test]
fn with_simple_build_script() {
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
        .file("src/main.rs", "#[cfg(foo)] fn main() {}")
        .file(
            "build.rs",
            r#"fn main() {
                println!("cargo::rustc-check-cfg=cfg(foo)");
                println!("cargo::rustc-cfg=foo");
            }"#,
        )
        .build();

    p.cargo("build -Zsbom")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let path = with_sbom_suffix(&p.bin("foo"));
    assert!(path.is_file());

    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "packages": [
    {
      "cfgs": [
        "foo"
      ],
      "dependencies": [],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.0.1",
      "profile": "{...}"
    }
  ],
  "root": 0,
  "rustc": "{...}",
  "version": 1
}
"#]]
        .is_json(),
    );
}

#[cargo_test]
fn with_build_dependencies() {
    Package::new("baz", "0.1.0").publish();
    Package::new("bar", "0.1.0")
        .build_dep("baz", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                build = "build.rs"

                [build-dependencies]
                baz = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 2 }")
        .file(
            "build.rs",
            r#"fn main() {
                println!("cargo::rustc-check-cfg=cfg(foo)");
                println!("cargo::rustc-cfg=foo");
            }"#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() { let _i = bar::bar(); }")
        .build();

    p.cargo("build -Zsbom")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let path = with_sbom_suffix(&p.bin("foo"));
    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "packages": [
    {
      "cfgs": [
        "foo"
      ],
      "dependencies": [
        {
          "index": 1,
          "kind": "build"
        }
      ],
      "features": [],
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0",
      "profile": "{...}"
    },
    {
      "cfgs": [],
      "dependencies": [],
      "features": [],
      "id": "registry+https://github.com/rust-lang/crates.io-index#baz@0.1.0",
      "profile": "{...}"
    },
    {
      "cfgs": [],
      "dependencies": [
        {
          "index": 0,
          "kind": "normal"
        }
      ],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.0.1",
      "profile": "{...}"
    }
  ],
  "root": 2,
  "rustc": "{...}",
  "version": 1
}
"#]]
        .is_json(),
    );
}

#[cargo_test]
fn crate_uses_different_features_for_build_and_normal_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                b = { path = "b/", features = ["f1"] }

                [build-dependencies]
                b = { path = "b/", features = ["f2"] }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() { b::f1(); }
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() { b::f2(); }
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.0.1"
                edition = "2021"

                [features]
                f1 = []
                f2 = []
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
                #[cfg(feature = "f1")]
                pub fn f1() {}

                #[cfg(feature = "f2")]
                pub fn f2() {}
            "#,
        )
        .build();

    p.cargo("build -Zsbom")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let path = with_sbom_suffix(&p.bin("a"));
    assert!(path.is_file());
    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "packages": [
    {
      "cfgs": [],
      "dependencies": [
        {
          "index": 1,
          "kind": "build"
        },
        {
          "index": 2,
          "kind": "normal"
        }
      ],
      "features": [],
      "id": "path+[ROOTURL]/foo#a@0.1.0",
      "profile": "{...}"
    },
    {
      "cfgs": [],
      "dependencies": [],
      "features": [
        "f2"
      ],
      "id": "path+[ROOTURL]/foo/b#0.0.1",
      "profile": "{...}"
    },
    {
      "cfgs": [],
      "dependencies": [],
      "features": [
        "f1"
      ],
      "id": "path+[ROOTURL]/foo/b#0.0.1",
      "profile": "{...}"
    }
  ],
  "root": 0,
  "rustc": "{...}",
  "version": 1
}
"#]]
        .is_json(),
    );
}
