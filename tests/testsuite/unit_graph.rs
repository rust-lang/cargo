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
        .masquerade_as_nightly_cargo(&["unit-graph"])
        .with_json(
            r#"{
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
                      "public": false
                    }
                  ],
                  "features": [
                    "feata"
                  ],
                  "mode": "build",
                  "pkg_id": "a 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
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
                    "strip": "none"
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
                    "src_path": "[..]/a-1.0.0/src/lib.rs",
                    "test": true
                  }
                },
                {
                  "dependencies": [
                    {
                      "extern_crate_name": "c",
                      "index": 2,
                      "noprelude": false,
                      "public": false
                    }
                  ],
                  "features": [
                    "featb"
                  ],
                  "mode": "build",
                  "pkg_id": "b 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
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
                    "strip": "none"
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
                    "src_path": "[..]/b-1.0.0/src/lib.rs",
                    "test": true
                  }
                },
                {
                  "dependencies": [],
                  "features": [
                    "featc"
                  ],
                  "mode": "build",
                  "pkg_id": "c 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
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
                    "strip": "none"
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
                    "src_path": "[..]/c-1.0.0/src/lib.rs",
                    "test": true
                  }
                },
                {
                  "dependencies": [
                    {
                      "extern_crate_name": "a",
                      "index": 0,
                      "noprelude": false,
                      "public": false
                    }
                  ],
                  "features": [],
                  "mode": "build",
                  "pkg_id": "foo 0.1.0 (path+file://[..]/foo)",
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
                    "strip": "none"
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
                    "src_path": "[..]/foo/src/lib.rs",
                    "test": true
                  }
                }
              ],
              "version": 1
            }
            "#,
        )
        .run();
}
