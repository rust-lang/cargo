//! Tests for --unit-graph option.

use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn gated() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("build --unit-graph")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the `--unit-graph` flag is unstable[..]
See [..]
See [..]
",
        )
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
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"{
              "version": 1,
              "units": [
                {
                  "pkg_id": "a 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
                  "target": {
                    "kind": [
                      "lib"
                    ],
                    "crate_types": [
                      "lib"
                    ],
                    "name": "a",
                    "src_path": "[..]/a-1.0.0/src/lib.rs",
                    "edition": "2015",
                    "doc": true,
                    "doctest": true,
                    "test": true
                  },
                  "profile": {
                    "name": "dev",
                    "opt_level": "0",
                    "lto": "false",
                    "codegen_units": null,
                    "debuginfo": 2,
                    "debug_assertions": true,
                    "overflow_checks": true,
                    "rpath": false,
                    "incremental": false,
                    "panic": "unwind",
                    "strip": "none",
                    "split_debuginfo": "{...}",
                    "trim_path": null
                  },
                  "platform": null,
                  "mode": "build",
                  "features": [
                    "feata"
                  ],
                  "dependencies": [
                    {
                      "index": 1,
                      "extern_crate_name": "b",
                      "public": false,
                      "noprelude": false
                    }
                  ]
                },
                {
                  "pkg_id": "b 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
                  "target": {
                    "kind": [
                      "lib"
                    ],
                    "crate_types": [
                      "lib"
                    ],
                    "name": "b",
                    "src_path": "[..]/b-1.0.0/src/lib.rs",
                    "edition": "2015",
                    "doc": true,
                    "doctest": true,
                    "test": true
                  },
                  "profile": {
                    "name": "dev",
                    "opt_level": "0",
                    "lto": "false",
                    "codegen_units": null,
                    "debuginfo": 2,
                    "debug_assertions": true,
                    "overflow_checks": true,
                    "rpath": false,
                    "incremental": false,
                    "panic": "unwind",
                    "strip": "none",
                    "split_debuginfo": "{...}",
                    "trim_path": null
                  },
                  "platform": null,
                  "mode": "build",
                  "features": [
                    "featb"
                  ],
                  "dependencies": [
                    {
                      "index": 2,
                      "extern_crate_name": "c",
                      "public": false,
                      "noprelude": false
                    }
                  ]
                },
                {
                  "pkg_id": "c 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
                  "target": {
                    "kind": [
                      "lib"
                    ],
                    "crate_types": [
                      "lib"
                    ],
                    "name": "c",
                    "src_path": "[..]/c-1.0.0/src/lib.rs",
                    "edition": "2015",
                    "test": true,
                    "doc": true,
                    "doctest": true
                  },
                  "profile": {
                    "name": "dev",
                    "opt_level": "0",
                    "lto": "false",
                    "codegen_units": null,
                    "debuginfo": 2,
                    "debug_assertions": true,
                    "overflow_checks": true,
                    "rpath": false,
                    "incremental": false,
                    "panic": "unwind",
                    "strip": "none",
                    "split_debuginfo": "{...}",
                    "trim_path": null
                  },
                  "platform": null,
                  "mode": "build",
                  "features": [
                    "featc"
                  ],
                  "dependencies": []
                },
                {
                  "pkg_id": "foo 0.1.0 (path+file://[..]/foo)",
                  "target": {
                    "kind": [
                      "lib"
                    ],
                    "crate_types": [
                      "lib"
                    ],
                    "name": "foo",
                    "src_path": "[..]/foo/src/lib.rs",
                    "edition": "2015",
                    "test": true,
                    "doc": true,
                    "doctest": true
                  },
                  "profile": {
                    "name": "dev",
                    "opt_level": "0",
                    "lto": "false",
                    "codegen_units": null,
                    "debuginfo": 2,
                    "debug_assertions": true,
                    "overflow_checks": true,
                    "rpath": false,
                    "incremental": false,
                    "panic": "unwind",
                    "strip": "none",
                    "split_debuginfo": "{...}",
                    "trim_path": null
                  },
                  "platform": null,
                  "mode": "build",
                  "features": [],
                  "dependencies": [
                    {
                      "index": 0,
                      "extern_crate_name": "a",
                      "public": false,
                      "noprelude": false
                    }
                  ]
                }
              ],
              "roots": [3]
            }
            "#,
        )
        .run();
}
