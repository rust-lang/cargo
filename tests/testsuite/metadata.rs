use crate::support::registry::Package;
use crate::support::{basic_bin_manifest, basic_lib_manifest, main_file, project};

#[test]
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
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/foo.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml",
                "metadata": null
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

#[test]
fn cargo_metadata_warns_on_implicit_version() {
    let p = project()
        .file("src/foo.rs", "")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .build();

    p.cargo("metadata").with_stderr("[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems").run();

    p.cargo("metadata --format-version 1").with_stderr("").run();
}

#[test]
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
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml",
                "metadata": null
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

#[test]
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
                "metadata": null
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

#[test]
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
                "name": "baz",
                "readme": null,
                "repository": null,
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "targets": [
                    {
                        "crate_types": [
                            "lib"
                        ],
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
                "name": "foo",
                "readme": null,
                "repository": null,
                "source": null,
                "targets": [
                    {
                        "crate_types": [
                            "bin"
                        ],
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
                "name": "foobar",
                "readme": null,
                "repository": null,
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "targets": [
                    {
                        "crate_types": [
                            "lib"
                        ],
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
                "name": "bar",
                "readme": null,
                "repository": null,
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "targets": [
                    {
                        "crate_types": [
                            "lib"
                        ],
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
                            "name": "bar",
                            "pkg": "bar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)"
                        },
                        {
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

#[test]
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
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/lib.rs"
                    },
                    {
                        "kind": [ "example" ],
                        "crate_types": [ "bin" ],
                        "edition": "2015",
                        "name": "ex",
                        "src_path": "[..]/foo/examples/ex.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml",
                "metadata": null
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

#[test]
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
                        "edition": "2015",
                        "name": "foo",
                        "src_path": "[..]/foo/src/lib.rs"
                    },
                    {
                        "kind": [ "example" ],
                        "crate_types": [ "rlib", "dylib" ],
                        "edition": "2015",
                        "name": "ex",
                        "src_path": "[..]/foo/examples/ex.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]Cargo.toml",
                "metadata": null
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

#[test]
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
                        "edition": "2015",
                        "name": "bar",
                        "src_path": "[..]bar/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]bar/Cargo.toml",
                "metadata": null
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
                        "edition": "2015",
                        "name": "baz",
                        "src_path": "[..]baz/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]baz/Cargo.toml",
                "metadata": null
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

#[test]
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
                        "edition": "2015",
                        "name": "bar",
                        "src_path": "[..]bar/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]bar/Cargo.toml",
                "metadata": null
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
                        "edition": "2015",
                        "name": "baz",
                        "src_path": "[..]baz/src/lib.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]baz/Cargo.toml",
                "metadata": null
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

#[test]
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
            "edition": "2015",
            "name":"foo",
            "src_path":"[..]/foo/src/foo.rs"
        }],
        "features":{},
        "manifest_path":"[..]Cargo.toml",
        "metadata": null,
        "readme": null,
        "repository": null
    }],
    "workspace_members": [ "foo 0.5.0 (path+file:[..]foo)" ],
    "resolve": null,
    "target_directory": "[..]foo/target",
    "version": 1,
    "workspace_root": "[..]/foo"
}"#;

#[test]
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

#[test]
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

#[test]
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

#[test]
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

#[test]
fn cargo_metadata_no_deps_cwd() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps")
        .with_json(MANIFEST_OUTPUT)
        .run();
}

#[test]
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

#[test]
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

#[test]
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
                }
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

#[test]
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
                "name": "bar",
                "readme": null,
                "repository": null,
                "source": null,
                "targets": [
                {
                    "crate_types": [
                        "lib"
                    ],
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

#[test]
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
                    "name": "foo",
                    "readme": null,
                    "repository": null,
                    "source": null,
                    "targets": [
                        {
                            "crate_types": [
                                "lib"
                            ],
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

#[test]
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
                    "name": "foo",
                    "readme": null,
                    "repository": null,
                    "source": null,
                    "targets": [
                        {
                            "crate_types": [
                                "lib"
                            ],
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

#[test]
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
            "name": "foo",
            "readme": null,
            "repository": null,
            "source": null,
            "targets": [
                {
                    "crate_types": [
                        "lib"
                    ],
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
            "name": "bar",
            "readme": null,
            "repository": null,
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "targets": [
                {
                    "crate_types": [
                        "lib"
                    ],
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
            "name": "bar",
            "readme": null,
            "repository": null,
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "targets": [
                {
                    "crate_types": [
                        "lib"
                    ],
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
                        "name": "bar",
                        "pkg": "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)"
                    },
                    {
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

#[test]
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
      "name": "foo",
      "readme": null,
      "repository": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
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

#[test]
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

    let output = p
        .cargo("metadata")
        .exec_with_output()
        .expect("cargo metadata failed");
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let meta: serde_json::Value = serde_json::from_str(stdout).expect("failed to parse json");
    let nodes = &meta["resolve"]["nodes"];
    assert!(nodes[0]["deps"].as_array().unwrap().is_empty());
    assert!(nodes[1]["deps"].as_array().unwrap().is_empty());
}
