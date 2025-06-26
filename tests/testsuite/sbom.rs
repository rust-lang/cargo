//! Tests for cargo-sbom precursor files.

use std::path::PathBuf;

use crate::prelude::*;
use cargo_test_support::basic_bin_manifest;
use cargo_test_support::cargo_test;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use snapbox::IntoData;

const SBOM_FILE_EXTENSION: &str = ".cargo-sbom.json";

fn append_sbom_suffix(link: &PathBuf) -> PathBuf {
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

    let file = append_sbom_suffix(&p.bin("foo"));
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

    let file = append_sbom_suffix(&p.bin("foo"));
    let output = std::fs::read_to_string(file).unwrap();
    // The expected test does contain the "rustc" section
    // but other tests omit them for brevity.
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "crates": [
    {
      "dependencies": [],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.5.0",
      "kind": [
        "bin"
      ]
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
  "target": "[HOST_TARGET]",
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

    let sbom_path = append_sbom_suffix(&p.dylib("foo"));
    assert!(sbom_path.is_file());

    let output = std::fs::read_to_string(sbom_path).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "crates": [
    {
      "dependencies": [],
      "features": [],
      "id": "path+[ROOTURL]/foo#1.2.3",
      "kind": [
        "dylib",
        "rlib"
      ]
    }
  ],
  "root": 0,
  "rustc": "{...}",
  "target": "[HOST_TARGET]",
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

    let path = append_sbom_suffix(&p.bin("foo"));
    assert!(path.is_file());

    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "crates": [
    {
      "dependencies": [
        {
          "index": 1,
          "kind": "build"
        }
      ],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.0.1",
      "kind": [
        "bin"
      ]
    },
    {
      "dependencies": [],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.0.1",
      "kind": [
        "custom-build"
      ]
    }
  ],
  "root": 0,
  "rustc": "{...}",
  "target": "[HOST_TARGET]",
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

    let path = append_sbom_suffix(&p.bin("foo"));
    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "crates": [
    {
      "dependencies": [
        {
          "index": 1,
          "kind": "build"
        }
      ],
      "features": [],
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0",
      "kind": [
        "lib"
      ]
    },
    {
      "dependencies": [
        {
          "index": 2,
          "kind": "normal"
        }
      ],
      "features": [],
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0",
      "kind": [
        "custom-build"
      ]
    },
    {
      "dependencies": [],
      "features": [],
      "id": "registry+https://github.com/rust-lang/crates.io-index#baz@0.1.0",
      "kind": [
        "lib"
      ]
    },
    {
      "dependencies": [
        {
          "index": 0,
          "kind": "normal"
        }
      ],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.0.1",
      "kind": [
        "bin"
      ]
    }
  ],
  "root": 3,
  "rustc": "{...}",
  "target": "[HOST_TARGET]",
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

    let path = append_sbom_suffix(&p.bin("a"));
    assert!(path.is_file());
    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "crates": [
    {
      "dependencies": [
        {
          "index": 1,
          "kind": "build"
        },
        {
          "index": 3,
          "kind": "normal"
        }
      ],
      "features": [],
      "id": "path+[ROOTURL]/foo#a@0.1.0",
      "kind": [
        "bin"
      ]
    },
    {
      "dependencies": [
        {
          "index": 2,
          "kind": "normal"
        }
      ],
      "features": [],
      "id": "path+[ROOTURL]/foo#a@0.1.0",
      "kind": [
        "custom-build"
      ]
    },
    {
      "dependencies": [],
      "features": [
        "f2"
      ],
      "id": "path+[ROOTURL]/foo/b#0.0.1",
      "kind": [
        "lib"
      ]
    },
    {
      "dependencies": [],
      "features": [
        "f1"
      ],
      "id": "path+[ROOTURL]/foo/b#0.0.1",
      "kind": [
        "lib"
      ]
    }
  ],
  "root": 0,
  "rustc": "{...}",
  "target": "[HOST_TARGET]",
  "version": 1
}
"#]]
        .is_json(),
    );
}

#[cargo_test]
fn artifact_dep() {
    Package::new("bar", "0.5.0")
        .file("src/main.rs", "fn main() {}")
        .file("Cargo.toml", &basic_bin_manifest("bar"))
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2021"

                [lib]
                crate-type = ["dylib"] 

                [dependencies]
                bar = { version = "0.5.0", artifact = "bin" }

                [build-dependencies]
                bar = { version = "0.5.0", artifact = "bin" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"
                fn main() {
                    let bar: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR").expect("CARGO_BIN_FILE_BAR").into();
                    assert!(&bar.is_file());
                }"#)
        .build();
    p.cargo("build -Z bindeps -Z sbom")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["bindeps", "sbom"])
        .run();

    let output = std::fs::read_to_string(append_sbom_suffix(&p.dylib("foo"))).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "crates": [
    {
      "dependencies": [],
      "features": [],
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.5.0",
      "kind": [
        "bin"
      ]
    },
    {
      "dependencies": [
        {
          "index": 0,
          "kind": "normal"
        },
        {
          "index": 0,
          "kind": "build"
        },
        {
          "index": 2,
          "kind": "build"
        }
      ],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.0.0",
      "kind": [
        "dylib"
      ]
    },
    {
      "dependencies": [],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.0.0",
      "kind": [
        "custom-build"
      ]
    }
  ],
  "root": 1,
  "rustc": "{...}",
  "target": "[HOST_TARGET]",
  "version": 1
}
"#]]
        .is_json(),
    );
}

#[cargo_test]
fn proc_macro() {
    Package::new("noop", "0.0.1")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "noop"
            version = "0.0.1"
            edition = "2021"

            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro_derive(Noop)]
            pub fn noop(_input: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
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
                edition = "2021"

                [dependencies]
                noop = "0.0.1"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[macro_use]
                extern crate noop;

                #[derive(Noop)]
                struct X;

                fn main() {}
            "#,
        )
        .build();

    p.cargo("build -Z sbom")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let output = std::fs::read_to_string(append_sbom_suffix(&p.bin("foo"))).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "crates": [
    {
      "dependencies": [
        {
          "index": 1,
          "kind": "build"
        }
      ],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.0.1",
      "kind": [
        "bin"
      ]
    },
    {
      "dependencies": [],
      "features": [],
      "id": "registry+https://github.com/rust-lang/crates.io-index#noop@0.0.1",
      "kind": [
        "proc-macro"
      ]
    }
  ],
  "root": 0,
  "rustc": "{...}",
  "target": "[HOST_TARGET]",
  "version": 1
}
"#]]
        .is_json(),
    );
}

#[cargo_test]
fn workspace_wrapper() {
    let wrapper = project()
        .at("wrapper")
        .file("Cargo.toml", &basic_bin_manifest("wrapper"))
        .file(
            "src/main.rs",
            r#"
            fn main() {
                let mut args = std::env::args().skip(1);
                if let Some(sbom) = std::env::var_os("CARGO_SBOM_PATH") {
                    for sbom in std::env::split_paths(&sbom) {
                        eprintln!("found sbom");
                        assert!(sbom.exists());
                    }
                }
                let status = std::process::Command::new(&args.next().unwrap())
                    .args(args).status().unwrap();
                std::process::exit(status.code().unwrap_or(1));
          }
          "#,
        )
        .build();
    wrapper.cargo("build").run();

    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {}"#)
        .build();
    p.cargo("build -Zsbom")
        .env("CARGO_BUILD_SBOM", "true")
        .env("RUSTC_WRAPPER", wrapper.bin("wrapper"))
        .masquerade_as_nightly_cargo(&["sbom"])
        .with_stderr_data(snapbox::str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
found sbom
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let file = append_sbom_suffix(&p.bin("foo"));
    let output = std::fs::read_to_string(file).unwrap();
    assert_e2e().eq(
        output,
        snapbox::str![[r#"
{
  "crates": [
    {
      "dependencies": [],
      "features": [],
      "id": "path+[ROOTURL]/foo#0.5.0",
      "kind": [
        "bin"
      ]
    }
  ],
  "root": 0,
  "rustc": "{...}",
  "target": "[HOST_TARGET]",
  "version": 1
}
"#]]
        .is_json(),
    );
}
