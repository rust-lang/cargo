//! Tests for the `cargo metadata` command.

use cargo_test_support::cross_compile::alternate;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_bin_manifest, basic_lib_manifest, main_file, project, rustc_host};

#[cargo_test]
fn cargo_metadata_simple() {
    let p = project()
        .file("src/foo.rs", "")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [
                    "wycats@example.com"
                ],
                "categories": [],
                "name": "foo",
                "version": "0.5.0",
                "id": "foo[..]",
                "keywords": [],
                "source": null,
                "dependencies": [],
                "edition": "2015",
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "readme": null,
                "repository": null,
                "targets": [
                    {
                        "kind": [
                            "bin"
                        ],
                        "crate_types": [
                            "bin"
                        ],
                        "doctest": false,
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/foo.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null
            }
        ],
        "workspace_members": ["foo 0.5.0 (path+file:[..]foo)"],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [],
                    "id": "foo 0.5.0 (path+file:[..]foo)"
                }
            ],
            "root": "foo 0.5.0 (path+file:[..]foo)"
        },
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_warns_on_implicit_version() {
    let p = project()
        .file("src/foo.rs", "")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .build();

    p.cargo("metadata").with_stderr("[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems").run();

    p.cargo("metadata --format-version 1").with_stderr("").run();
}

#[cargo_test]
fn library_with_several_crate_types() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.5.0"

[lib]
crate-type = ["lib", "staticlib"]
            "#,
        )
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [],
                "categories": [],
                "name": "foo",
                "readme": null,
                "repository": null,
                "version": "0.5.0",
                "id": "foo[..]",
                "keywords": [],
                "source": null,
                "dependencies": [],
                "edition": "2015",
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "targets": [
                    {
                        "kind": [
                            "lib",
                            "staticlib"
                        ],
                        "crate_types": [
                            "lib",
                            "staticlib"
                        ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null
            }
        ],
        "workspace_members": ["foo 0.5.0 (path+file:[..]foo)"],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [],
                    "id": "foo 0.5.0 (path+file:[..]foo)"
                }
            ],
            "root": "foo 0.5.0 (path+file:[..]foo)"
        },
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn library_with_features() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.5.0"

[features]
default = ["default_feat"]
default_feat = []
optional_feat = []
            "#,
        )
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [],
                "categories": [],
                "name": "foo",
                "readme": null,
                "repository": null,
                "version": "0.5.0",
                "id": "foo[..]",
                "keywords": [],
                "source": null,
                "dependencies": [],
                "edition": "2015",
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "targets": [
                    {
                        "kind": [
                            "lib"
                        ],
                        "crate_types": [
                            "lib"
                        ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/lib.rs"
                    }
                ],
                "features": {
                  "default": [
                      "default_feat"
                  ],
                  "default_feat": [],
                  "optional_feat": []
                },
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null
            }
        ],
        "workspace_members": ["foo 0.5.0 (path+file:[..]foo)"],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [
                      "default",
                      "default_feat"
                    ],
                    "id": "foo 0.5.0 (path+file:[..]foo)"
                }
            ],
            "root": "foo 0.5.0 (path+file:[..]foo)"
        },
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_with_deps_and_version() {
    let p = project()
        .file("src/foo.rs", "")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            license = "MIT"
            description = "foo"

            [[bin]]
            name = "foo"

            [dependencies]
            bar = "*"
            [dev-dependencies]
            foobar = "*"
        "#,
        )
        .build();
    Package::new("baz", "0.0.1").publish();
    Package::new("foobar", "0.0.1").publish();
    Package::new("bar", "0.0.1").dep("baz", "0.0.1").publish();

    p.cargo("metadata -q --format-version 1")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [],
                "categories": [],
                "dependencies": [],
                "description": null,
                "edition": "2015",
                "features": {},
                "id": "baz 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                "keywords": [],
                "license": null,
                "license_file": null,
                "links": null,
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null,
                "name": "baz",
                "readme": null,
                "repository": null,
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "targets": [
                    {
                        "crate_types": [
                            "lib"
                        ],
                        "doctest": true,
                        "edition": "2015",
                        "kind": [
                            "lib"
                        ],
                        "name": "baz",
                        "src_path": "[..]src/lib.rs"
                    }
                ],
                "version": "0.0.1"
            },
            {
                "authors": [],
                "categories": [],
                "dependencies": [
                    {
                        "features": [],
                        "kind": null,
                        "name": "bar",
                        "optional": false,
                        "registry": null,
                        "rename": null,
                        "req": "*",
                        "source": "registry+https://github.com/rust-lang/crates.io-index",
                        "target": null,
                        "uses_default_features": true
                    },
                    {
                        "features": [],
                        "kind": "dev",
                        "name": "foobar",
                        "optional": false,
                        "registry": null,
                        "rename": null,
                        "req": "*",
                        "source": "registry+https://github.com/rust-lang/crates.io-index",
                        "target": null,
                        "uses_default_features": true
                    }
                ],
                "description": "foo",
                "edition": "2015",
                "features": {},
                "id": "foo 0.5.0 (path+file:[..]foo)",
                "keywords": [],
                "license": "MIT",
                "license_file": null,
                "links": null,
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null,
                "name": "foo",
                "readme": null,
                "repository": null,
                "source": null,
                "targets": [
                    {
                        "crate_types": [
                            "bin"
                        ],
                        "doctest": false,
                        "edition": "2015",
                        "kind": [
                            "bin"
                        ],
                        "name": "foo",
                        "src_path": "[..]src/foo.rs"
                    }
                ],
                "version": "0.5.0"
            },
            {
                "authors": [],
                "categories": [],
                "dependencies": [],
                "description": null,
                "edition": "2015",
                "features": {},
                "id": "foobar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                "keywords": [],
                "license": null,
                "license_file": null,
                "links": null,
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null,
                "name": "foobar",
                "readme": null,
                "repository": null,
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "targets": [
                    {
                        "crate_types": [
                            "lib"
                        ],
                        "doctest": true,
                        "edition": "2015",
                        "kind": [
                            "lib"
                        ],
                        "name": "foobar",
                        "src_path": "[..]src/lib.rs"
                    }
                ],
                "version": "0.0.1"
            },
            {
                "authors": [],
                "categories": [],
                "dependencies": [
                    {
                        "features": [],
                        "kind": null,
                        "name": "baz",
                        "optional": false,
                        "registry": null,
                        "rename": null,
                        "req": "^0.0.1",
                        "source": "registry+https://github.com/rust-lang/crates.io-index",
                        "target": null,
                        "uses_default_features": true
                    }
                ],
                "description": null,
                "edition": "2015",
                "features": {},
                "id": "bar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                "keywords": [],
                "license": null,
                "license_file": null,
                "links": null,
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null,
                "name": "bar",
                "readme": null,
                "repository": null,
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "targets": [
                    {
                        "crate_types": [
                            "lib"
                        ],
                        "doctest": true,
                        "edition": "2015",
                        "kind": [
                            "lib"
                        ],
                        "name": "bar",
                        "src_path": "[..]src/lib.rs"
                    }
                ],
                "version": "0.0.1"
            }
        ],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [],
                    "id": "baz 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                },
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [],
                    "id": "foobar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                },
                {
                    "dependencies": [
                        "bar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                        "foobar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                    ],
                    "deps": [
                        {
                            "dep_kinds": [
                              {
                                "kind": null,
                                "target": null
                              }
                            ],
                            "name": "bar",
                            "pkg": "bar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                        },
                        {
                            "dep_kinds": [
                              {
                                "kind": "dev",
                                "target": null
                              }
                            ],
                            "name": "foobar",
                            "pkg": "foobar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                        }
                    ],
                    "features": [],
                    "id": "foo 0.5.0 (path+file:[..]foo)"
                },
                {
                    "dependencies": [
                        "baz 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                    ],
                    "deps": [
                        {
                            "dep_kinds": [
                              {
                                "kind": null,
                                "target": null
                              }
                            ],
                            "name": "baz",
                            "pkg": "baz 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                        }
                    ],
                    "features": [],
                    "id": "bar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                }
            ],
            "root": "foo 0.5.0 (path+file:[..]foo)"
        },
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_members": [
            "foo 0.5.0 (path+file:[..]foo)"
        ],
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn example() {
    let p = project()
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"

[[example]]
name = "ex"
            "#,
        )
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [],
                "categories": [],
                "name": "foo",
                "readme": null,
                "repository": null,
                "version": "0.1.0",
                "id": "foo[..]",
                "keywords": [],
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "edition": "2015",
                "source": null,
                "dependencies": [],
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "crate_types": [ "lib" ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/lib.rs"
                    },
                    {
                        "kind": [ "example" ],
                        "crate_types": [ "bin" ],
                        "doctest": false,
                        "edition": "2015",
                        "name": "ex",
                        "src_path": "[..]/foo/examples/ex.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null
            }
        ],
        "workspace_members": [
            "foo 0.1.0 (path+file:[..]foo)"
        ],
        "resolve": {
            "root": "foo 0.1.0 (path+file://[..]foo)",
            "nodes": [
                {
                    "id": "foo 0.1.0 (path+file:[..]foo)",
                    "features": [],
                    "dependencies": [],
                    "deps": []
                }
            ]
        },
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn example_lib() {
    let p = project()
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"

[[example]]
name = "ex"
crate-type = ["rlib", "dylib"]
            "#,
        )
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [],
                "categories": [],
                "name": "foo",
                "readme": null,
                "repository": null,
                "version": "0.1.0",
                "id": "foo[..]",
                "keywords": [],
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "edition": "2015",
                "source": null,
                "dependencies": [],
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "crate_types": [ "lib" ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/lib.rs"
                    },
                    {
                        "kind": [ "example" ],
                        "crate_types": [ "rlib", "dylib" ],
                        "doctest": false,
                        "edition": "2015",
                        "name": "ex",
                        "src_path": "[..]/foo/examples/ex.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null
            }
        ],
        "workspace_members": [
            "foo 0.1.0 (path+file:[..]foo)"
        ],
        "resolve": {
            "root": "foo 0.1.0 (path+file://[..]foo)",
            "nodes": [
                {
                    "id": "foo 0.1.0 (path+file:[..]foo)",
                    "features": [],
                    "dependencies": [],
                    "deps": []
                }
            ]
        },
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn workspace_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar", "baz"]
        "#,
        )
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [
                    "wycats@example.com"
                ],
                "categories": [],
                "name": "bar",
                "version": "0.5.0",
                "id": "bar[..]",
                "readme": null,
                "repository": null,
                "keywords": [],
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "edition": "2015",
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "crate_types": [ "lib" ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "bar",
                        "src_path": "[..]bar/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]bar/Cargo.toml",
                "metadata": null,
                "publish": null
            },
            {
                "authors": [
                    "wycats@example.com"
                ],
                "categories": [],
                "name": "baz",
                "readme": null,
                "repository": null,
                "version": "0.5.0",
                "id": "baz[..]",
                "keywords": [],
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "edition": "2015",
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "crate_types": [ "lib" ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "baz",
                        "src_path": "[..]baz/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]baz/Cargo.toml",
                "metadata": null,
                "publish": null
            }
        ],
        "workspace_members": ["baz 0.5.0 (path+file:[..]baz)", "bar 0.5.0 (path+file:[..]bar)"],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [],
                    "id": "baz 0.5.0 (path+file:[..]baz)"
                },
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [],
                    "id": "bar 0.5.0 (path+file:[..]bar)"
                }
            ],
            "root": null
        },
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn workspace_metadata_no_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar", "baz"]
        "#,
        )
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("metadata --no-deps")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [
                    "wycats@example.com"
                ],
                "categories": [],
                "name": "bar",
                "readme": null,
                "repository": null,
                "version": "0.5.0",
                "id": "bar[..]",
                "keywords": [],
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "edition": "2015",
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "crate_types": [ "lib" ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "bar",
                        "src_path": "[..]bar/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]bar/Cargo.toml",
                "metadata": null,
                "publish": null
            },
            {
                "authors": [
                    "wycats@example.com"
                ],
                "categories": [],
                "name": "baz",
                "readme": null,
                "repository": null,
                "version": "0.5.0",
                "id": "baz[..]",
                "keywords": [],
                "source": null,
                "dependencies": [],
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "edition": "2015",
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "crate_types": ["lib"],
                        "doctest": true,
                        "edition": "2015",
                        "name": "baz",
                        "src_path": "[..]baz/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]baz/Cargo.toml",
                "metadata": null,
                "publish": null
            }
        ],
        "workspace_members": ["baz 0.5.0 (path+file:[..]baz)", "bar 0.5.0 (path+file:[..]bar)"],
        "resolve": null,
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_with_invalid_manifest() {
    let p = project().file("Cargo.toml", "").build();

    p.cargo("metadata --format-version 1")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  virtual manifests must be configured with [workspace]",
        )
        .run();
}

const MANIFEST_OUTPUT: &str = r#"
{
    "packages": [{
        "authors": [
            "wycats@example.com"
        ],
        "categories": [],
        "name":"foo",
        "version":"0.5.0",
        "id":"foo[..]0.5.0[..](path+file://[..]/foo)",
        "source":null,
        "dependencies":[],
        "keywords": [],
        "license": null,
        "license_file": null,
        "links": null,
        "description": null,
        "edition": "2015",
        "targets":[{
            "kind":["bin"],
            "crate_types":["bin"],
            "doctest": false,
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]/foo/src/foo.rs"
        }],
        "features":{},
        "manifest_path":"[..]Cargo.toml",
        "metadata": null,
        "publish": null,
        "readme": null,
        "repository": null
    }],
    "workspace_members": [ "foo 0.5.0 (path+file:[..]foo)" ],
    "resolve": null,
    "target_directory": "[..]foo/target",
    "version": 1,
    "workspace_root": "[..]/foo"
}"#;

#[cargo_test]
fn cargo_metadata_no_deps_path_to_cargo_toml_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps --manifest-path foo/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .with_json(MANIFEST_OUTPUT)
        .run();
}

#[cargo_test]
fn cargo_metadata_no_deps_path_to_cargo_toml_absolute() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps --manifest-path")
        .arg(p.root().join("Cargo.toml"))
        .cwd(p.root().parent().unwrap())
        .with_json(MANIFEST_OUTPUT)
        .run();
}

#[cargo_test]
fn cargo_metadata_no_deps_path_to_cargo_toml_parent_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps --manifest-path foo")
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr(
            "[ERROR] the manifest-path must be \
             a path to a Cargo.toml file",
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_no_deps_path_to_cargo_toml_parent_absolute() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps --manifest-path")
        .arg(p.root())
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr(
            "[ERROR] the manifest-path must be \
             a path to a Cargo.toml file",
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_no_deps_cwd() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps")
        .with_json(MANIFEST_OUTPUT)
        .run();
}

#[cargo_test]
fn cargo_metadata_bad_version() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps --format-version 2")
        .with_status(1)
        .with_stderr_contains(
            "\
error: '2' isn't a valid value for '--format-version <VERSION>'
<tab>[possible values: 1]
",
        )
        .run();
}

#[cargo_test]
fn multiple_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [features]
            a = []
            b = []
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("metadata --features").arg("a b").run();
}

#[cargo_test]
fn package_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = ["wycats@example.com"]
            categories = ["database"]
            keywords = ["database"]
            readme = "README.md"
            repository = "https://github.com/rust-lang/cargo"

            [package.metadata.bar]
            baz = "quux"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("metadata --no-deps")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": ["wycats@example.com"],
                "categories": ["database"],
                "name": "foo",
                "readme": "README.md",
                "repository": "https://github.com/rust-lang/cargo",
                "version": "0.1.0",
                "id": "foo[..]",
                "keywords": ["database"],
                "source": null,
                "dependencies": [],
                "edition": "2015",
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "crate_types": [ "lib" ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]foo/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]foo/Cargo.toml",
                "metadata": {
                    "bar": {
                        "baz": "quux"
                    }
                },
                "publish": null
            }
        ],
        "workspace_members": ["foo[..]"],
        "resolve": null,
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn package_publish() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = ["wycats@example.com"]
            categories = ["database"]
            keywords = ["database"]
            readme = "README.md"
            repository = "https://github.com/rust-lang/cargo"
            publish = ["my-registry"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("metadata --no-deps")
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": ["wycats@example.com"],
                "categories": ["database"],
                "name": "foo",
                "readme": "README.md",
                "repository": "https://github.com/rust-lang/cargo",
                "version": "0.1.0",
                "id": "foo[..]",
                "keywords": ["database"],
                "source": null,
                "dependencies": [],
                "edition": "2015",
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "targets": [
                    {
                        "kind": [ "lib" ],
                        "crate_types": [ "lib" ],
                        "doctest": true,
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]foo/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]foo/Cargo.toml",
                "metadata": null,
                "publish": ["my-registry"]
            }
        ],
        "workspace_members": ["foo[..]"],
        "resolve": null,
        "target_directory": "[..]foo/target",
        "version": 1,
        "workspace_root": "[..]/foo"
    }"#,
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_path_to_cargo_toml_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
        "#,
        )
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("package --manifest-path")
        .arg(p.root().join("bar/Cargo.toml"))
        .cwd(p.root().parent().unwrap())
        .run();

    p.cargo("metadata --manifest-path")
        .arg(p.root().join("target/package/bar-0.5.0/Cargo.toml"))
        .with_json(
            r#"
        {
            "packages": [
            {
                "authors": [
                    "wycats@example.com"
                ],
                "categories": [],
                "dependencies": [],
                "description": null,
                "edition": "2015",
                "features": {},
                "id": "bar 0.5.0 ([..])",
                "keywords": [],
                "license": null,
                "license_file": null,
                "links": null,
                "manifest_path": "[..]Cargo.toml",
                "metadata": null,
                "publish": null,
                "name": "bar",
                "readme": null,
                "repository": null,
                "source": null,
                "targets": [
                {
                    "crate_types": [
                        "lib"
                    ],
                    "doctest": true,
                    "edition": "2015",
                    "kind": [
                        "lib"
                    ],
                    "name": "bar",
                    "src_path": "[..]src/lib.rs"
                }
                ],
                "version": "0.5.0"
            }
            ],
            "resolve": {
                "nodes": [
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [],
                    "id": "bar 0.5.0 ([..])"
                }
                ],
                "root": "bar 0.5.0 (path+file:[..])"
            },
            "target_directory": "[..]",
            "version": 1,
            "workspace_members": [
                "bar 0.5.0 (path+file:[..])"
            ],
            "workspace_root": "[..]"
        }
"#,
        )
        .run();
}

#[cargo_test]
fn package_edition_2018() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = ["wycats@example.com"]
            edition = "2018"
        "#,
        )
        .build();
    p.cargo("metadata")
        .with_json(
            r#"
        {
            "packages": [
                {
                    "authors": [
                        "wycats@example.com"
                    ],
                    "categories": [],
                    "dependencies": [],
                    "description": null,
                    "edition": "2018",
                    "features": {},
                    "id": "foo 0.1.0 (path+file:[..])",
                    "keywords": [],
                    "license": null,
                    "license_file": null,
                    "links": null,
                    "manifest_path": "[..]Cargo.toml",
                    "metadata": null,
                    "publish": null,
                    "name": "foo",
                    "readme": null,
                    "repository": null,
                    "source": null,
                    "targets": [
                        {
                            "crate_types": [
                                "lib"
                            ],
                            "doctest": true,
                            "edition": "2018",
                            "kind": [
                                "lib"
                            ],
                            "name": "foo",
                            "src_path": "[..]src/lib.rs"
                        }
                    ],
                    "version": "0.1.0"
                }
            ],
            "resolve": {
                "nodes": [
                    {
                        "dependencies": [],
                        "deps": [],
                        "features": [],
                        "id": "foo 0.1.0 (path+file:[..])"
                    }
                ],
                "root": "foo 0.1.0 (path+file:[..])"
            },
            "target_directory": "[..]",
            "version": 1,
            "workspace_members": [
                "foo 0.1.0 (path+file:[..])"
            ],
            "workspace_root": "[..]"
        }
        "#,
        )
        .run();
}

#[cargo_test]
fn target_edition_2018() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = ["wycats@example.com"]
            edition = "2015"

            [lib]
            edition = "2018"
        "#,
        )
        .build();
    p.cargo("metadata")
        .with_json(
            r#"
        {
            "packages": [
                {
                    "authors": [
                        "wycats@example.com"
                    ],
                    "categories": [],
                    "dependencies": [],
                    "description": null,
                    "edition": "2015",
                    "features": {},
                    "id": "foo 0.1.0 (path+file:[..])",
                    "keywords": [],
                    "license": null,
                    "license_file": null,
                    "links": null,
                    "manifest_path": "[..]Cargo.toml",
                    "metadata": null,
                    "publish": null,
                    "name": "foo",
                    "readme": null,
                    "repository": null,
                    "source": null,
                    "targets": [
                        {
                            "crate_types": [
                                "lib"
                            ],
                            "doctest": true,
                            "edition": "2018",
                            "kind": [
                                "lib"
                            ],
                            "name": "foo",
                            "src_path": "[..]src/lib.rs"
                        },
                        {
                            "crate_types": [
                                "bin"
                            ],
                            "doctest": false,
                            "edition": "2015",
                            "kind": [
                                "bin"
                            ],
                            "name": "foo",
                            "src_path": "[..]src/main.rs"
                        }
                    ],
                    "version": "0.1.0"
                }
            ],
            "resolve": {
                "nodes": [
                    {
                        "dependencies": [],
                        "deps": [],
                        "features": [],
                        "id": "foo 0.1.0 (path+file:[..])"
                    }
                ],
                "root": "foo 0.1.0 (path+file:[..])"
            },
            "target_directory": "[..]",
            "version": 1,
            "workspace_members": [
                "foo 0.1.0 (path+file:[..])"
            ],
            "workspace_root": "[..]"
        }
        "#,
        )
        .run();
}

#[cargo_test]
fn rename_dependency() {
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { version = "0.1.0" }
            baz = { version = "0.2.0", package = "bar" }
        "#,
        )
        .file("src/lib.rs", "extern crate bar; extern crate baz;")
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
{
    "packages": [
        {
            "authors": [],
            "categories": [],
            "dependencies": [
                {
                    "features": [],
                    "kind": null,
                    "name": "bar",
                    "optional": false,
                    "rename": null,
                    "registry": null,
                    "req": "^0.1.0",
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    "target": null,
                    "uses_default_features": true
                },
                {
                    "features": [],
                    "kind": null,
                    "name": "bar",
                    "optional": false,
                    "rename": "baz",
                    "registry": null,
                    "req": "^0.2.0",
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                    "target": null,
                    "uses_default_features": true
                }
            ],
            "description": null,
            "edition": "2015",
            "features": {},
            "id": "foo 0.0.1[..]",
            "keywords": [],
            "license": null,
            "license_file": null,
            "links": null,
            "manifest_path": "[..]",
            "metadata": null,
            "publish": null,
            "name": "foo",
            "readme": null,
            "repository": null,
            "source": null,
            "targets": [
                {
                    "crate_types": [
                        "lib"
                    ],
                    "doctest": true,
                    "edition": "2015",
                    "kind": [
                        "lib"
                    ],
                    "name": "foo",
                    "src_path": "[..]"
                }
            ],
            "version": "0.0.1"
        },
        {
            "authors": [],
            "categories": [],
            "dependencies": [],
            "description": null,
            "edition": "2015",
            "features": {},
            "id": "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
            "keywords": [],
            "license": null,
            "license_file": null,
            "links": null,
            "manifest_path": "[..]",
            "metadata": null,
            "publish": null,
            "name": "bar",
            "readme": null,
            "repository": null,
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "targets": [
                {
                    "crate_types": [
                        "lib"
                    ],
                    "doctest": true,
                    "edition": "2015",
                    "kind": [
                        "lib"
                    ],
                    "name": "bar",
                    "src_path": "[..]"
                }
            ],
            "version": "0.1.0"
        },
        {
            "authors": [],
            "categories": [],
            "dependencies": [],
            "description": null,
            "edition": "2015",
            "features": {},
            "id": "bar 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
            "keywords": [],
            "license": null,
            "license_file": null,
            "links": null,
            "manifest_path": "[..]",
            "metadata": null,
            "publish": null,
            "name": "bar",
            "readme": null,
            "repository": null,
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "targets": [
                {
                    "crate_types": [
                        "lib"
                    ],
                    "doctest": true,
                    "edition": "2015",
                    "kind": [
                        "lib"
                    ],
                    "name": "bar",
                    "src_path": "[..]"
                }
            ],
            "version": "0.2.0"
        }
    ],
    "resolve": {
        "nodes": [
            {
                "dependencies": [],
                "deps": [],
                "features": [],
                "id": "bar 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)"
            },
            {
                "dependencies": [],
                "deps": [],
                "features": [],
                "id": "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)"
            },
            {
                "dependencies": [
                    "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
                    "bar 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)"
                ],
                "deps": [
                    {
                        "dep_kinds": [
                          {
                            "kind": null,
                            "target": null
                          }
                        ],
                        "name": "bar",
                        "pkg": "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)"
                    },
                    {
                        "dep_kinds": [
                          {
                            "kind": null,
                            "target": null
                          }
                        ],
                        "name": "baz",
                        "pkg": "bar 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)"
                    }
                ],
                "features": [],
                "id": "foo 0.0.1[..]"
            }
        ],
        "root": "foo 0.0.1[..]"
    },
    "target_directory": "[..]",
    "version": 1,
    "workspace_members": [
        "foo 0.0.1[..]"
    ],
    "workspace_root": "[..]"
}"#,
        )
        .run();
}

#[cargo_test]
fn metadata_links() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.5.0"
            links = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
{
  "packages": [
    {
      "authors": [],
      "categories": [],
      "dependencies": [],
      "description": null,
      "edition": "2015",
      "features": {},
      "id": "foo 0.5.0 [..]",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": "a",
      "manifest_path": "[..]/foo/Cargo.toml",
      "metadata": null,
      "publish": null,
      "name": "foo",
      "readme": null,
      "repository": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "foo",
          "src_path": "[..]/foo/src/lib.rs"
        },
        {
          "crate_types": [
            "bin"
          ],
          "doctest": false,
          "edition": "2015",
          "kind": [
            "custom-build"
          ],
          "name": "build-script-build",
          "src_path": "[..]/foo/build.rs"
        }
      ],
      "version": "0.5.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "foo 0.5.0 [..]"
      }
    ],
    "root": "foo 0.5.0 [..]"
  },
  "target_directory": "[..]/foo/target",
  "version": 1,
  "workspace_members": [
    "foo 0.5.0 [..]"
  ],
  "workspace_root": "[..]/foo"
}
"#,
        )
        .run()
}

#[cargo_test]
fn deps_with_bin_only() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [dependencies]
            bdep = { path = "bdep" }
        "#,
        )
        .file("src/lib.rs", "")
        .file("bdep/Cargo.toml", &basic_bin_manifest("bdep"))
        .file("bdep/src/main.rs", "fn main() {}")
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
{
  "packages": [
    {
      "name": "foo",
      "version": "0.1.0",
      "id": "foo 0.1.0 ([..])",
      "license": null,
      "license_file": null,
      "description": null,
      "source": null,
      "dependencies": [
        {
          "name": "bdep",
          "source": null,
          "req": "*",
          "kind": null,
          "rename": null,
          "optional": false,
          "uses_default_features": true,
          "features": [],
          "target": null,
          "registry": null
        }
      ],
      "targets": [
        {
          "kind": [
            "lib"
          ],
          "crate_types": [
            "lib"
          ],
          "name": "foo",
          "src_path": "[..]/foo/src/lib.rs",
          "edition": "2015",
          "doctest": true
        }
      ],
      "features": {},
      "manifest_path": "[..]/foo/Cargo.toml",
      "metadata": null,
      "publish": null,
      "authors": [],
      "categories": [],
      "keywords": [],
      "readme": null,
      "repository": null,
      "edition": "2015",
      "links": null
    }
  ],
  "workspace_members": [
    "foo 0.1.0 ([..])"
  ],
  "resolve": {
    "nodes": [
      {
        "id": "foo 0.1.0 ([..])",
        "dependencies": [],
        "deps": [],
        "features": []
      }
    ],
    "root": "foo 0.1.0 ([..])"
  },
  "target_directory": "[..]/foo/target",
  "version": 1,
  "workspace_root": "[..]foo"
}
"#,
        )
        .run();
}

#[cargo_test]
fn filter_platform() {
    // Testing the --filter-platform flag.
    Package::new("normal-dep", "0.0.1").publish();
    Package::new("host-dep", "0.0.1").publish();
    Package::new("alt-dep", "0.0.1").publish();
    Package::new("cfg-dep", "0.0.1").publish();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                normal-dep = "0.0.1"

                [target.{}.dependencies]
                host-dep = "0.0.1"

                [target.{}.dependencies]
                alt-dep = "0.0.1"

                [target.'cfg(foobar)'.dependencies]
                cfg-dep = "0.0.1"
                "#,
                rustc_host(),
                alternate()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    let alt_dep = r#"
    {
      "name": "alt-dep",
      "version": "0.0.1",
      "id": "alt-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
      "license": null,
      "license_file": null,
      "description": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "dependencies": [],
      "targets": [
        {
          "kind": [
            "lib"
          ],
          "crate_types": [
            "lib"
          ],
          "name": "alt-dep",
          "src_path": "[..]/alt-dep-0.0.1/src/lib.rs",
          "edition": "2015",
          "doctest": true
        }
      ],
      "features": {},
      "manifest_path": "[..]/alt-dep-0.0.1/Cargo.toml",
      "metadata": null,
      "publish": null,
      "authors": [],
      "categories": [],
      "keywords": [],
      "readme": null,
      "repository": null,
      "edition": "2015",
      "links": null
    },
    "#;

    let cfg_dep = r#"
    {
      "name": "cfg-dep",
      "version": "0.0.1",
      "id": "cfg-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
      "license": null,
      "license_file": null,
      "description": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "dependencies": [],
      "targets": [
        {
          "kind": [
            "lib"
          ],
          "crate_types": [
            "lib"
          ],
          "name": "cfg-dep",
          "src_path": "[..]/cfg-dep-0.0.1/src/lib.rs",
          "edition": "2015",
          "doctest": true
        }
      ],
      "features": {},
      "manifest_path": "[..]/cfg-dep-0.0.1/Cargo.toml",
      "metadata": null,
      "publish": null,
      "authors": [],
      "categories": [],
      "keywords": [],
      "readme": null,
      "repository": null,
      "edition": "2015",
      "links": null
    },
    "#;

    let host_dep = r#"
    {
      "name": "host-dep",
      "version": "0.0.1",
      "id": "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
      "license": null,
      "license_file": null,
      "description": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "dependencies": [],
      "targets": [
        {
          "kind": [
            "lib"
          ],
          "crate_types": [
            "lib"
          ],
          "name": "host-dep",
          "src_path": "[..]/host-dep-0.0.1/src/lib.rs",
          "edition": "2015",
          "doctest": true
        }
      ],
      "features": {},
      "manifest_path": "[..]/host-dep-0.0.1/Cargo.toml",
      "metadata": null,
      "publish": null,
      "authors": [],
      "categories": [],
      "keywords": [],
      "readme": null,
      "repository": null,
      "edition": "2015",
      "links": null
    },
    "#;

    let normal_dep = r#"
    {
      "name": "normal-dep",
      "version": "0.0.1",
      "id": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
      "license": null,
      "license_file": null,
      "description": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "dependencies": [],
      "targets": [
        {
          "kind": [
            "lib"
          ],
          "crate_types": [
            "lib"
          ],
          "name": "normal-dep",
          "src_path": "[..]/normal-dep-0.0.1/src/lib.rs",
          "edition": "2015",
          "doctest": true
        }
      ],
      "features": {},
      "manifest_path": "[..]/normal-dep-0.0.1/Cargo.toml",
      "metadata": null,
      "publish": null,
      "authors": [],
      "categories": [],
      "keywords": [],
      "readme": null,
      "repository": null,
      "edition": "2015",
      "links": null
    },
    "#;

    let foo = r#"
    {
      "name": "foo",
      "version": "0.1.0",
      "id": "foo 0.1.0 (path+file:[..]foo)",
      "license": null,
      "license_file": null,
      "description": null,
      "source": null,
      "dependencies": [
        {
          "name": "normal-dep",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "req": "^0.0.1",
          "kind": null,
          "rename": null,
          "optional": false,
          "uses_default_features": true,
          "features": [],
          "target": null,
          "registry": null
        },
        {
          "name": "cfg-dep",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "req": "^0.0.1",
          "kind": null,
          "rename": null,
          "optional": false,
          "uses_default_features": true,
          "features": [],
          "target": "cfg(foobar)",
          "registry": null
        },
        {
          "name": "alt-dep",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "req": "^0.0.1",
          "kind": null,
          "rename": null,
          "optional": false,
          "uses_default_features": true,
          "features": [],
          "target": "$ALT_TRIPLE",
          "registry": null
        },
        {
          "name": "host-dep",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "req": "^0.0.1",
          "kind": null,
          "rename": null,
          "optional": false,
          "uses_default_features": true,
          "features": [],
          "target": "$HOST_TRIPLE",
          "registry": null
        }
      ],
      "targets": [
        {
          "kind": [
            "lib"
          ],
          "crate_types": [
            "lib"
          ],
          "name": "foo",
          "src_path": "[..]/foo/src/lib.rs",
          "edition": "2015",
          "doctest": true
        }
      ],
      "features": {},
      "manifest_path": "[..]/foo/Cargo.toml",
      "metadata": null,
      "publish": null,
      "authors": [],
      "categories": [],
      "keywords": [],
      "readme": null,
      "repository": null,
      "edition": "2015",
      "links": null
    }
    "#
    .replace("$ALT_TRIPLE", &alternate())
    .replace("$HOST_TRIPLE", &rustc_host());

    // Normal metadata, no filtering, returns *everything*.
    p.cargo("metadata")
        .with_json(
            &r#"
{
  "packages": [
    $ALT_DEP
    $CFG_DEP
    $HOST_DEP
    $NORMAL_DEP
    $FOO
  ],
  "workspace_members": [
    "foo 0.1.0 (path+file:[..]foo)"
  ],
  "resolve": {
    "nodes": [
      {
        "id": "alt-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      },
      {
        "id": "cfg-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      },
      {
        "id": "foo 0.1.0 (path+file:[..]foo)",
        "dependencies": [
          "alt-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
          "cfg-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
          "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
          "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
        ],
        "deps": [
          {
            "name": "alt_dep",
            "pkg": "alt-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": "$ALT_TRIPLE"
              }
            ]
          },
          {
            "name": "cfg_dep",
            "pkg": "cfg-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": "cfg(foobar)"
              }
            ]
          },
          {
            "name": "host_dep",
            "pkg": "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": "$HOST_TRIPLE"
              }
            ]
          },
          {
            "name": "normal_dep",
            "pkg": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ]
          }
        ],
        "features": []
      },
      {
        "id": "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      },
      {
        "id": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      }
    ],
    "root": "foo 0.1.0 (path+file:[..]foo)"
  },
  "target_directory": "[..]/foo/target",
  "version": 1,
  "workspace_root": "[..]/foo"
}
"#
            .replace("$ALT_TRIPLE", &alternate())
            .replace("$HOST_TRIPLE", &rustc_host())
            .replace("$ALT_DEP", alt_dep)
            .replace("$CFG_DEP", cfg_dep)
            .replace("$HOST_DEP", host_dep)
            .replace("$NORMAL_DEP", normal_dep)
            .replace("$FOO", &foo),
        )
        .run();

    // Filter on alternate, removes cfg and host.
    p.cargo("metadata --filter-platform")
        .arg(alternate())
        .with_json(
            &r#"
{
  "packages": [
    $ALT_DEP
    $NORMAL_DEP
    $FOO
  ],
  "workspace_members": "{...}",
  "resolve": {
    "nodes": [
      {
        "id": "alt-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      },
      {
        "id": "foo 0.1.0 (path+file:[..]foo)",
        "dependencies": [
          "alt-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
          "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
        ],
        "deps": [
          {
            "name": "alt_dep",
            "pkg": "alt-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": "$ALT_TRIPLE"
              }
            ]
          },
          {
            "name": "normal_dep",
            "pkg": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ]
          }
        ],
        "features": []
      },
      {
        "id": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      }
    ],
    "root": "foo 0.1.0 (path+file:[..]foo)"
  },
  "target_directory": "[..]foo/target",
  "version": 1,
  "workspace_root": "[..]foo"
}
"#
            .replace("$ALT_TRIPLE", &alternate())
            .replace("$ALT_DEP", alt_dep)
            .replace("$NORMAL_DEP", normal_dep)
            .replace("$FOO", &foo),
        )
        .run();

    // Filter on host, removes alt and cfg.
    p.cargo("metadata --filter-platform")
        .arg(rustc_host())
        .with_json(
            &r#"
{
  "packages": [
    $HOST_DEP
    $NORMAL_DEP
    $FOO
  ],
  "workspace_members": "{...}",
  "resolve": {
    "nodes": [
      {
        "id": "foo 0.1.0 (path+file:[..]foo)",
        "dependencies": [
          "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
          "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
        ],
        "deps": [
          {
            "name": "host_dep",
            "pkg": "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": "$HOST_TRIPLE"
              }
            ]
          },
          {
            "name": "normal_dep",
            "pkg": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ]
          }
        ],
        "features": []
      },
      {
        "id": "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      },
      {
        "id": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      }
    ],
    "root": "foo 0.1.0 (path+file:[..]foo)"
  },
  "target_directory": "[..]foo/target",
  "version": 1,
  "workspace_root": "[..]foo"
}
"#
            .replace("$HOST_TRIPLE", &rustc_host())
            .replace("$HOST_DEP", host_dep)
            .replace("$NORMAL_DEP", normal_dep)
            .replace("$FOO", &foo),
        )
        .run();

    // Filter host with cfg, removes alt only
    p.cargo("metadata --filter-platform")
        .arg(rustc_host())
        .env("RUSTFLAGS", "--cfg=foobar")
        .with_json(
            &r#"
{
  "packages": [
    $CFG_DEP
    $HOST_DEP
    $NORMAL_DEP
    $FOO
  ],
  "workspace_members": "{...}",
  "resolve": {
    "nodes": [
      {
        "id": "cfg-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      },
      {
        "id": "foo 0.1.0 (path+file:[..]/foo)",
        "dependencies": [
          "cfg-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
          "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
          "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
        ],
        "deps": [
          {
            "name": "cfg_dep",
            "pkg": "cfg-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": "cfg(foobar)"
              }
            ]
          },
          {
            "name": "host_dep",
            "pkg": "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": "$HOST_TRIPLE"
              }
            ]
          },
          {
            "name": "normal_dep",
            "pkg": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ]
          }
        ],
        "features": []
      },
      {
        "id": "host-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      },
      {
        "id": "normal-dep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
        "dependencies": [],
        "deps": [],
        "features": []
      }
    ],
    "root": "foo 0.1.0 (path+file:[..]/foo)"
  },
  "target_directory": "[..]/foo/target",
  "version": 1,
  "workspace_root": "[..]/foo"
}
"#
            .replace("$HOST_TRIPLE", &rustc_host())
            .replace("$CFG_DEP", cfg_dep)
            .replace("$HOST_DEP", host_dep)
            .replace("$NORMAL_DEP", normal_dep)
            .replace("$FOO", &foo),
        )
        .run();
}

#[cargo_test]
fn dep_kinds() {
    Package::new("bar", "0.1.0").publish();
    Package::new("winapi", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = "0.1"

            [dev-dependencies]
            bar = "0.1"

            [build-dependencies]
            bar = "0.1"

            [target.'cfg(windows)'.dependencies]
            winapi = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("metadata")
        .with_json(
            r#"
{
  "packages": "{...}",
  "workspace_members": "{...}",
  "target_directory": "{...}",
  "version": 1,
  "workspace_root": "{...}",
  "resolve": {
    "nodes": [
      {
        "id": "bar 0.1.0 [..]",
        "dependencies": [],
        "deps": [],
        "features": []
      },
      {
        "id": "foo 0.1.0 [..]",
        "dependencies": [
          "bar 0.1.0 [..]",
          "winapi 0.1.0 [..]"
        ],
        "deps": [
          {
            "name": "bar",
            "pkg": "bar 0.1.0 [..]",
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              },
              {
                "kind": "dev",
                "target": null
              },
              {
                "kind": "build",
                "target": null
              }
            ]
          },
          {
            "name": "winapi",
            "pkg": "winapi 0.1.0 [..]",
            "dep_kinds": [
              {
                "kind": null,
                "target": "cfg(windows)"
              }
            ]
          }
        ],
        "features": []
      },
      {
        "id": "winapi 0.1.0 [..]",
        "dependencies": [],
        "deps": [],
        "features": []
      }
    ],
    "root": "foo 0.1.0 [..]"
  }
}
"#,
        )
        .run();
}
