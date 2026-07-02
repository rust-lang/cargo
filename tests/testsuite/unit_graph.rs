//! Tests for --unit-graph option.

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{basic_bin_manifest, project};

#[cargo_test]
fn gated() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("build --unit-graph")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `--unit-graph` flag is unstable, and only available on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/[..].html for more information about Rust release channels.
See https://github.com/rust-lang/cargo/issues/8002 for more information about the `--unit-graph` flag.

"#]])
        .run();
}

#[cargo_test]
fn simple() {
    Package::new("a", "1.0.0")
        .dep("b", "1.0")
        .feature("feata", &["b/featb"])
        .publish();
    Package::new("b", "1.0.0")
        .dep("c", "1.0")
        .feature("featb", &["c/featc"])
        .publish();
    Package::new("c", "1.0.0").feature("featc", &[]).publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            a = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --features a/feata --unit-graph -Zunstable-options")
        .masquerade_as_nightly_cargo(&["unit-graph"])
        .with_stdout_data(
            str![[r#"
{
  "roots": [
    3
  ],
  "units": [
    {
      "dependencies": [
        {
          "extern_crate_name": "b",
          "index": 1,
          "noprelude": false,
          "nounused": false,
          "public": false
        }
      ],
      "features": [
        "feata"
      ],
      "mode": "build",
      "pkg_id": "registry+https://github.com/rust-lang/crates.io-index#a@1.0.0",
      "platform": null,
      "profile": {
        "codegen_backend": null,
        "codegen_units": null,
        "debug_assertions": true,
        "debuginfo": 2,
        "incremental": false,
        "lto": "false",
        "name": "dev",
        "opt_level": "0",
        "overflow_checks": true,
        "panic": "unwind",
        "rpath": false,
        "split_debuginfo": "{...}",
        "strip": "{...}"
      },
      "target": {
        "crate_types": [
          "lib"
        ],
        "doc": true,
        "doctest": true,
        "edition": "2015",
        "kind": [
          "lib"
        ],
        "name": "a",
        "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/a-1.0.0/src/lib.rs",
        "test": true
      }
    },
    {
      "dependencies": [
        {
          "extern_crate_name": "c",
          "index": 2,
          "noprelude": false,
          "nounused": false,
          "public": false
        }
      ],
      "features": [
        "featb"
      ],
      "mode": "build",
      "pkg_id": "registry+https://github.com/rust-lang/crates.io-index#b@1.0.0",
      "platform": null,
      "profile": {
        "codegen_backend": null,
        "codegen_units": null,
        "debug_assertions": true,
        "debuginfo": 2,
        "incremental": false,
        "lto": "false",
        "name": "dev",
        "opt_level": "0",
        "overflow_checks": true,
        "panic": "unwind",
        "rpath": false,
        "split_debuginfo": "{...}",
        "strip": "{...}"
      },
      "target": {
        "crate_types": [
          "lib"
        ],
        "doc": true,
        "doctest": true,
        "edition": "2015",
        "kind": [
          "lib"
        ],
        "name": "b",
        "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/b-1.0.0/src/lib.rs",
        "test": true
      }
    },
    {
      "dependencies": [],
      "features": [
        "featc"
      ],
      "mode": "build",
      "pkg_id": "registry+https://github.com/rust-lang/crates.io-index#c@1.0.0",
      "platform": null,
      "profile": {
        "codegen_backend": null,
        "codegen_units": null,
        "debug_assertions": true,
        "debuginfo": 2,
        "incremental": false,
        "lto": "false",
        "name": "dev",
        "opt_level": "0",
        "overflow_checks": true,
        "panic": "unwind",
        "rpath": false,
        "split_debuginfo": "{...}",
        "strip": "{...}"
      },
      "target": {
        "crate_types": [
          "lib"
        ],
        "doc": true,
        "doctest": true,
        "edition": "2015",
        "kind": [
          "lib"
        ],
        "name": "c",
        "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/c-1.0.0/src/lib.rs",
        "test": true
      }
    },
    {
      "dependencies": [
        {
          "extern_crate_name": "a",
          "index": 0,
          "noprelude": false,
          "nounused": false,
          "public": false
        }
      ],
      "features": [],
      "mode": "build",
      "pkg_id": "path+[ROOTURL]/foo#0.1.0",
      "platform": null,
      "profile": {
        "codegen_backend": null,
        "codegen_units": null,
        "debug_assertions": true,
        "debuginfo": 2,
        "incremental": false,
        "lto": "false",
        "name": "dev",
        "opt_level": "0",
        "overflow_checks": true,
        "panic": "unwind",
        "rpath": false,
        "split_debuginfo": "{...}",
        "strip": "{...}"
      },
      "target": {
        "crate_types": [
          "lib"
        ],
        "doc": true,
        "doctest": true,
        "edition": "2015",
        "kind": [
          "lib"
        ],
        "name": "foo",
        "src_path": "[ROOT]/foo/src/lib.rs",
        "test": true
      }
    }
  ],
  "version": 1
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn artifact_alias_edges() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            authors = []

            [dependencies]
            bar = { path = "bar/", artifact = "bin" }
            bar-alt = { package = "bar", path = "bar/", artifact = "bin" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    let graph = p
        .cargo("build --unit-graph -Zunstable-options -Z bindeps")
        .masquerade_as_nightly_cargo(&["unit-graph", "bindeps"])
        .run_json();

    let units = graph["units"].as_array().unwrap();
    let artifact_index = units
        .iter()
        .position(|unit| unit["target"]["name"] == "bar")
        .unwrap();
    let root_unit = units
        .iter()
        .find(|unit| unit["target"]["name"] == "foo")
        .unwrap();
    let mut aliases = root_unit["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|dep| {
            (
                dep["extern_crate_name"].as_str().unwrap().to_owned(),
                dep["index"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    aliases.sort();

    assert_eq!(
        aliases,
        vec![
            ("bar".to_owned(), artifact_index as u64),
            ("bar_alt".to_owned(), artifact_index as u64),
        ]
    );
}
