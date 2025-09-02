//! Tests for the `cargo metadata` command.

use crate::prelude::*;
use cargo_test_support::paths;
use cargo_test_support::registry::Package;
use cargo_test_support::{
    basic_bin_manifest, basic_lib_manifest, main_file, project, rustc_host, str,
};
use serde_json::json;

#[cargo_test]
fn cargo_metadata_simple() {
    let p = project()
        .file("src/foo.rs", "")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .build();

    p.cargo("metadata")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/foo.rs",
          "test": true
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
        "id": "path+[ROOTURL]/foo#0.5.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.5.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_warns_on_implicit_version() {
    let p = project()
        .file("src/foo.rs", "")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .build();

    p.cargo("metadata")
        .with_stderr_data(str![[r#"
[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems

"#]])
        .run();

    p.cargo("metadata --format-version 1")
        .with_stderr_data("")
        .run();
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib",
            "staticlib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib",
            "staticlib"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/lib.rs",
          "test": true
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
        "id": "path+[ROOTURL]/foo#0.5.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.5.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {
        "default": [
          "default_feat"
        ],
        "default_feat": [],
        "optional_feat": []
      },
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
      ],
      "version": "0.5.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [
          "default",
          "default_feat"
        ],
        "id": "path+[ROOTURL]/foo#0.5.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.5.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
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
                [package]
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
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
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#baz@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/baz-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "baz",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "baz",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/baz-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
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
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.5.0",
      "keywords": [],
      "license": "MIT",
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/foo.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/foobar-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "foobar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "foobar",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/foobar-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#baz@0.0.1"
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
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#baz@0.0.1"
          }
        ],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#baz@0.0.1"
      },
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1"
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
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": "dev",
                "target": null
              }
            ],
            "name": "foobar",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.5.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.5.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

/// The `public` field should not show up in `cargo metadata` output if `-Zpublic-dependency`
/// is not enabled
#[cargo_test]
fn cargo_metadata_public_private_dependencies_disabled() {
    let p = project()
        .file("src/foo.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = []
                license = "MIT"
                description = "foo"

                [[bin]]
                name = "foo"

                [dependencies]
                bar = { version = "*", public = false }
                foobar = { version = "*", public = true }
                baz = "*"
            "#,
        )
        .build();
    Package::new("bar", "0.0.1").publish();
    Package::new("foobar", "0.0.2").publish();
    Package::new("baz", "0.0.3").publish();

    p.cargo("metadata -q --format-version 1")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "name": "bar",
      "...": "{...}"
    },
    {
      "name": "baz",
      "...": "{...}"
    },
    {
      "name": "foo",
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
          "kind": null,
          "name": "baz",
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
          "kind": null,
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
      "...": "{...}"
    },
    {
      "name": "foobar",
      "...": "{...}"
    }
  ],
  "...": "{...}"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_public_private_dependencies_enabled() {
    let p = project()
        .file("src/foo.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = []
                license = "MIT"
                description = "foo"

                [[bin]]
                name = "foo"

                [dependencies]
                bar = { version = "*", public = false }
                foobar = { version = "*", public = true }
                baz = "*"
            "#,
        )
        .build();
    Package::new("bar", "0.0.1").publish();
    Package::new("foobar", "0.0.2").publish();
    Package::new("baz", "0.0.3").publish();

    p.cargo("metadata -q --format-version 1 -Zpublic-dependency")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "name": "bar",
      "...": "{...}"
    },
    {
      "name": "baz",
      "...": "{...}"
    },
    {
      "name": "foo",
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "bar",
          "optional": false,
          "public": false,
          "registry": null,
          "rename": null,
          "req": "*",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "baz",
          "optional": false,
          "public": false,
          "registry": null,
          "rename": null,
          "req": "*",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "foobar",
          "optional": false,
          "public": true,
          "registry": null,
          "rename": null,
          "req": "*",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        }
      ],
      "...": "{...}"
    },
    {
      "name": "foobar",
      "...": "{...}"
    }
  ],
  "...": "{...}"
}
"#]]
            .is_json(),
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": false,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "example"
          ],
          "name": "ex",
          "src_path": "[ROOT]/foo/examples/ex.rs",
          "test": false
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
        "id": "path+[ROOTURL]/foo#0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
        },
        {
          "crate_types": [
            "rlib",
            "dylib"
          ],
          "doc": false,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "example"
          ],
          "name": "ex",
          "src_path": "[ROOT]/foo/examples/ex.rs",
          "test": false
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
        "id": "path+[ROOTURL]/foo#0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
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

                [workspace.metadata]
                tool1 = "hello"
                tool2 = [1, 2, 3]

                [workspace.metadata.foo]
                bar = 3

            "#,
        )
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("metadata")
        .with_stdout_data(
            str![[r#"
{
  "metadata": {
    "foo": {
      "bar": 3
    },
    "tool1": "hello",
    "tool2": [
      1,
      2,
      3
    ]
  },
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/bar#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/bar/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/foo/bar/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/baz#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/baz/Cargo.toml",
      "metadata": null,
      "name": "baz",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "baz",
          "src_path": "[ROOT]/foo/baz/src/lib.rs",
          "test": true
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
        "id": "path+[ROOTURL]/foo/bar#0.5.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "path+[ROOTURL]/foo/baz#0.5.0"
      }
    ],
    "root": null
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo/bar#0.5.0",
    "path+[ROOTURL]/foo/baz#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo/bar#0.5.0",
    "path+[ROOTURL]/foo/baz#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_metadata_with_dependencies_no_deps() {
    let p = project()
        // NOTE that 'artifact' isn't mentioned in the workspace here, yet it shows up as member.
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                baz = { path = "../baz/" }
                artifact = { path = "../artifact/", artifact = "bin" }
           "#,
        )
        .file("bar/src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/lib.rs", "")
        .file("artifact/Cargo.toml", &basic_bin_manifest("artifact"))
        .file("artifact/src/main.rs", "fn main() {}")
        .build();

    p.cargo("metadata --no-deps -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "artifact": {
            "kinds": [
              "bin"
            ],
            "lib": false,
            "target": null
          },
          "features": [],
          "kind": null,
          "name": "artifact",
          "optional": false,
          "path": "[ROOT]/foo/artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "baz",
          "optional": false,
          "path": "[ROOT]/foo/baz",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/bar#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/bar/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/foo/bar/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/artifact#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/artifact/Cargo.toml",
      "metadata": null,
      "name": "artifact",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "artifact",
          "src_path": "[ROOT]/foo/artifact/src/main.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/baz#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/baz/Cargo.toml",
      "metadata": null,
      "name": "baz",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "baz",
          "src_path": "[ROOT]/foo/baz/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo/bar#0.5.0",
    "path+[ROOTURL]/foo/artifact#0.5.0",
    "path+[ROOTURL]/foo/baz#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo/bar#0.5.0",
    "path+[ROOTURL]/foo/artifact#0.5.0",
    "path+[ROOTURL]/foo/baz#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_metadata_with_dependencies_and_resolve() {
    let alt_target = "wasm32-unknown-unknown";
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "artifact", "non-artifact", "bin-only-artifact"]
            "#,
        )
        .file(
            "bar/Cargo.toml",
            &r#"
                [package]

                name = "bar"
                version = "0.5.0"
                authors = []

                [build-dependencies]
                artifact = { path = "../artifact/", artifact = "bin", target = "target" }
                bin-only-artifact = { path = "../bin-only-artifact/", artifact = "bin", target = "$ALT_TARGET" }
                non-artifact = { path = "../non-artifact" }

                [dependencies]
                artifact = { path = "../artifact/", artifact = ["cdylib", "staticlib", "bin:baz-name"], lib = true, target = "$ALT_TARGET" }
                bin-only-artifact = { path = "../bin-only-artifact/", artifact = "bin:a-name" }
                non-artifact = { path = "../non-artifact" }

                [dev-dependencies]
                artifact = { path = "../artifact/" }
                non-artifact = { path = "../non-artifact" }
                bin-only-artifact = { path = "../bin-only-artifact/", artifact = "bin:b-name" }
           "#.replace("$ALT_TARGET", alt_target),
        )
        .file("bar/src/lib.rs", "")
        .file("bar/build.rs", "fn main() {}")
        .file(
            "artifact/Cargo.toml",
            r#"
                [package]
                name = "artifact"
                version = "0.5.0"
                authors = []

                [lib]
                crate-type = ["staticlib", "cdylib", "rlib"]

                [[bin]]
                name = "bar-name"

                [[bin]]
                name = "baz-name"
            "#,
        )
        .file("artifact/src/main.rs", "fn main() {}")
        .file("artifact/src/lib.rs", "")
        .file(
            "bin-only-artifact/Cargo.toml",
            r#"
                [package]
                name = "bin-only-artifact"
                version = "0.5.0"
                authors = []

                [[bin]]
                name = "a-name"

                [[bin]]
                name = "b-name"
            "#,
        )
        .file("bin-only-artifact/src/main.rs", "fn main() {}")
        .file("non-artifact/Cargo.toml",
              r#"
                [package]

                name = "non-artifact"
                version = "0.5.0"
                authors = []
            "#,
        )
        .file("non-artifact/src/lib.rs", "")
        .build();

    p.cargo("metadata -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/artifact#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/artifact/Cargo.toml",
      "metadata": null,
      "name": "artifact",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "staticlib",
            "cdylib",
            "rlib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "staticlib",
            "cdylib",
            "rlib"
          ],
          "name": "artifact",
          "src_path": "[ROOT]/foo/artifact/src/lib.rs",
          "test": true
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "bar-name",
          "src_path": "[ROOT]/foo/artifact/src/main.rs",
          "test": true
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "baz-name",
          "src_path": "[ROOT]/foo/artifact/src/main.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "artifact": {
            "kinds": [
              "cdylib",
              "staticlib",
              "bin:baz-name"
            ],
            "lib": true,
            "target": "wasm32-unknown-unknown"
          },
          "features": [],
          "kind": null,
          "name": "artifact",
          "optional": false,
          "path": "[ROOT]/foo/artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "artifact": {
            "kinds": [
              "bin:a-name"
            ],
            "lib": false,
            "target": null
          },
          "features": [],
          "kind": null,
          "name": "bin-only-artifact",
          "optional": false,
          "path": "[ROOT]/foo/bin-only-artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "non-artifact",
          "optional": false,
          "path": "[ROOT]/foo/non-artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": "dev",
          "name": "artifact",
          "optional": false,
          "path": "[ROOT]/foo/artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "artifact": {
            "kinds": [
              "bin:b-name"
            ],
            "lib": false,
            "target": null
          },
          "features": [],
          "kind": "dev",
          "name": "bin-only-artifact",
          "optional": false,
          "path": "[ROOT]/foo/bin-only-artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": "dev",
          "name": "non-artifact",
          "optional": false,
          "path": "[ROOT]/foo/non-artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "artifact": {
            "kinds": [
              "bin"
            ],
            "lib": false,
            "target": "target"
          },
          "features": [],
          "kind": "build",
          "name": "artifact",
          "optional": false,
          "path": "[ROOT]/foo/artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "artifact": {
            "kinds": [
              "bin"
            ],
            "lib": false,
            "target": "wasm32-unknown-unknown"
          },
          "features": [],
          "kind": "build",
          "name": "bin-only-artifact",
          "optional": false,
          "path": "[ROOT]/foo/bin-only-artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": "build",
          "name": "non-artifact",
          "optional": false,
          "path": "[ROOT]/foo/non-artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/bar#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/bar/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/foo/bar/src/lib.rs",
          "test": true
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": false,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "custom-build"
          ],
          "name": "build-script-build",
          "src_path": "[ROOT]/foo/bar/build.rs",
          "test": false
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/bin-only-artifact#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/bin-only-artifact/Cargo.toml",
      "metadata": null,
      "name": "bin-only-artifact",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "a-name",
          "src_path": "[ROOT]/foo/bin-only-artifact/src/main.rs",
          "test": true
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "b-name",
          "src_path": "[ROOT]/foo/bin-only-artifact/src/main.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/non-artifact#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/non-artifact/Cargo.toml",
      "metadata": null,
      "name": "non-artifact",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "non_artifact",
          "src_path": "[ROOT]/foo/non-artifact/src/lib.rs",
          "test": true
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
        "id": "path+[ROOTURL]/foo/artifact#0.5.0"
      },
      {
        "dependencies": [
          "path+[ROOTURL]/foo/artifact#0.5.0",
          "path+[ROOTURL]/foo/bin-only-artifact#0.5.0",
          "path+[ROOTURL]/foo/non-artifact#0.5.0"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "extern_name": "artifact",
                "kind": null,
                "target": null
              },
              {
                "artifact": "cdylib",
                "compile_target": "wasm32-unknown-unknown",
                "extern_name": "artifact",
                "kind": null,
                "target": null
              },
              {
                "artifact": "staticlib",
                "compile_target": "wasm32-unknown-unknown",
                "extern_name": "artifact",
                "kind": null,
                "target": null
              },
              {
                "artifact": "bin",
                "bin_name": "baz-name",
                "compile_target": "wasm32-unknown-unknown",
                "extern_name": "baz_name",
                "kind": null,
                "target": null
              },
              {
                "kind": "dev",
                "target": null
              },
              {
                "artifact": "bin",
                "bin_name": "bar-name",
                "compile_target": "<target>",
                "extern_name": "bar_name",
                "kind": "build",
                "target": null
              },
              {
                "artifact": "bin",
                "bin_name": "baz-name",
                "compile_target": "<target>",
                "extern_name": "baz_name",
                "kind": "build",
                "target": null
              }
            ],
            "name": "artifact",
            "pkg": "path+[ROOTURL]/foo/artifact#0.5.0"
          },
          {
            "dep_kinds": [
              {
                "artifact": "bin",
                "bin_name": "a-name",
                "extern_name": "a_name",
                "kind": null,
                "target": null
              },
              {
                "artifact": "bin",
                "bin_name": "b-name",
                "extern_name": "b_name",
                "kind": "dev",
                "target": null
              },
              {
                "artifact": "bin",
                "bin_name": "a-name",
                "compile_target": "wasm32-unknown-unknown",
                "extern_name": "a_name",
                "kind": "build",
                "target": null
              },
              {
                "artifact": "bin",
                "bin_name": "b-name",
                "compile_target": "wasm32-unknown-unknown",
                "extern_name": "b_name",
                "kind": "build",
                "target": null
              }
            ],
            "name": "",
            "pkg": "path+[ROOTURL]/foo/bin-only-artifact#0.5.0"
          },
          {
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
            ],
            "name": "non_artifact",
            "pkg": "path+[ROOTURL]/foo/non-artifact#0.5.0"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo/bar#0.5.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "path+[ROOTURL]/foo/bin-only-artifact#0.5.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "path+[ROOTURL]/foo/non-artifact#0.5.0"
      }
    ],
    "root": null
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo/bar#0.5.0",
    "path+[ROOTURL]/foo/artifact#0.5.0",
    "path+[ROOTURL]/foo/bin-only-artifact#0.5.0",
    "path+[ROOTURL]/foo/non-artifact#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo/bar#0.5.0",
    "path+[ROOTURL]/foo/artifact#0.5.0",
    "path+[ROOTURL]/foo/bin-only-artifact#0.5.0",
    "path+[ROOTURL]/foo/non-artifact#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn cargo_metadata_with_invalid_manifest() {
    let p = project().file("Cargo.toml", "").build();

    p.cargo("metadata --format-version 1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  manifest is missing either a `[package]` or a `[workspace]`

"#]])
        .run();
}

#[cargo_test]
fn cargo_metadata_with_invalid_authors_field() {
    let p = project()
        .file("src/foo.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                authors = ""
            "#,
        )
        .build();

    p.cargo("metadata")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: string "", expected a vector of strings or workspace
 --> Cargo.toml:3:27
  |
3 |                 authors = ""
  |                           ^^

"#]])
        .run();
}

#[cargo_test]
fn cargo_metadata_with_invalid_version_field() {
    let p = project()
        .file("src/foo.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                version = 1
            "#,
        )
        .build();

    p.cargo("metadata")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: integer `1`, expected SemVer version
 --> Cargo.toml:3:27
  |
3 |                 version = 1
  |                           ^

"#]])
        .run();
}

#[cargo_test]
fn cargo_metadata_with_invalid_publish_field() {
    let p = project()
        .file("src/foo.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                publish = "foo"
            "#,
        )
        .build();

    p.cargo("metadata")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: string "foo", expected a boolean, a vector of strings, or workspace
 --> Cargo.toml:3:27
  |
3 |                 publish = "foo"
  |                           ^^^^^

"#]])
        .run();
}

#[cargo_test]
fn cargo_metadata_with_invalid_artifact_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"

                [dependencies]
                artifact = { path = "artifact", artifact = "bin:notfound" }
           "#,
        )
        .file("src/lib.rs", "")
        .file("artifact/Cargo.toml", &basic_bin_manifest("artifact"))
        .file("artifact/src/main.rs", "fn main() {}")
        .build();

    p.cargo("metadata -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[LOCKING] 1 package to latest compatible version
[ERROR] dependency `artifact` in package `foo` requires a `bin:notfound` artifact to be present.

"#]])
        .run();
}

#[cargo_test]
fn cargo_metadata_with_invalid_duplicate_renamed_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"

                [dependencies]
                bar = { path = "bar" }
                baz = { path = "bar", package = "bar" }
           "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("metadata")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[LOCKING] 1 package to latest compatible version
[ERROR] the crate `foo v0.5.0 ([ROOT]/foo)` depends on crate `bar v0.5.0 ([ROOT]/foo/bar)` multiple times with different names

"#]])
        .run();
}

#[cargo_test]
fn cargo_metadata_no_deps_path_to_cargo_toml_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps --manifest-path foo/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/foo.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/foo.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
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
        .with_stderr_data(str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]])
        .run();
}

#[cargo_test]
fn cargo_metadata_no_deps_cwd() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("metadata --no-deps")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/foo.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
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
        .with_stderr_data(str![[r#"
[ERROR] invalid value '2' for '--format-version <VERSION>'
  [possible values: 1]

...
"#]])
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
                homepage = "https://rust-lang.org"
                documentation = "https://doc.rust-lang.org/stable/std/"

                [package.metadata.bar]
                baz = "quux"
            "#,
        )
        .file("README.md", "")
        .file("src/lib.rs", "")
        .build();

    p.cargo("metadata --no-deps")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [
        "database"
      ],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": "https://doc.rust-lang.org/stable/std/",
      "edition": "2015",
      "features": {},
      "homepage": "https://rust-lang.org",
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [
        "database"
      ],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": {
        "bar": {
          "baz": "quux"
        }
      },
      "name": "foo",
      "publish": null,
      "readme": "README.md",
      "repository": "https://github.com/rust-lang/cargo",
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
      ],
      "version": "0.1.0"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
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
        .file("README.md", "")
        .file("src/lib.rs", "")
        .build();

    p.cargo("metadata --no-deps")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [
        "database"
      ],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [
        "database"
      ],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": [
        "my-registry"
      ],
      "readme": "README.md",
      "repository": "https://github.com/rust-lang/cargo",
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
      ],
      "version": "0.1.0"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/target/package/bar-0.5.0#bar@0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/target/package/bar-0.5.0/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/foo/target/package/bar-0.5.0/src/lib.rs",
          "test": true
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
        "id": "path+[ROOTURL]/foo/target/package/bar-0.5.0#bar@0.5.0"
      }
    ],
    "root": "path+[ROOTURL]/foo/target/package/bar-0.5.0#bar@0.5.0"
  },
  "target_directory": "[ROOT]/foo/target/package/bar-0.5.0/target",
  "build_directory": "[ROOT]/foo/target/package/bar-0.5.0/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo/target/package/bar-0.5.0#bar@0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo/target/package/bar-0.5.0#bar@0.5.0"
  ],
  "workspace_root": "[ROOT]/foo/target/package/bar-0.5.0"
}
"#]]
            .is_json(),
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2018",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2018",
          "kind": [
            "lib"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/lib.rs",
          "test": true
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
        "id": "path+[ROOTURL]/foo#0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn package_default_run() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", r#"fn main() { println!("hello A"); }"#)
        .file("src/bin/b.rs", r#"fn main() { println!("hello B"); }"#)
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = ["wycats@example.com"]
                edition = "2018"
                default-run = "a"
            "#,
        )
        .build();
    let json = p.cargo("metadata").run_json();
    assert_eq!(json["packages"][0]["default_run"], json!("a"));
}

#[cargo_test]
fn package_rust_version() {
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
                rust-version = "1.56"
            "#,
        )
        .build();
    let json = p.cargo("metadata").run_json();
    assert_eq!(json["packages"][0]["rust_version"], json!("1.56"));
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2018",
          "kind": [
            "lib"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/lib.rs",
          "test": true
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/main.rs",
          "test": true
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
        "id": "path+[ROOTURL]/foo#0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
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
                [package]
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.1.0/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.1.0/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.1.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.2.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.2.0/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.2.0/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.2.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "bar",
          "optional": false,
          "registry": null,
          "rename": null,
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
          "registry": null,
          "rename": "baz",
          "req": "^0.2.0",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
        "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.2.0"
      },
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0",
          "registry+https://github.com/rust-lang/crates.io-index#bar@0.2.0"
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
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "baz",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#bar@0.2.0"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.0.1"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.0.1"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn metadata_links() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.5.0"
            links = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("metadata")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": "a",
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": false,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "custom-build"
          ],
          "name": "build-script-build",
          "src_path": "[ROOT]/foo/build.rs",
          "test": false
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
        "id": "path+[ROOTURL]/foo#0.5.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.5.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "bdep",
          "optional": false,
          "path": "[ROOT]/foo/bdep",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
        "id": "path+[ROOTURL]/foo#0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
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
    // Just needs to be a valid target that is different from host.
    // Presumably nobody runs these tests on wasm. 🙃
    let alt_target = "wasm32-unknown-unknown";
    let host_target = rustc_host();
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
                host_target, alt_target
            ),
        )
        .file("src/lib.rs", "")
        .build();

    // We're going to be checking that we don't download excessively,
    // so we need to ensure that downloads will happen.
    let clear = || {
        paths::cargo_home().join("registry/cache").rm_rf();
        paths::cargo_home().join("registry/src").rm_rf();
        p.build_dir().rm_rf();
    };

    // Normal metadata, no filtering, returns *everything*.
    p.cargo("metadata")
        .with_stderr_data(
            str![[r#"
[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[UPDATING] `dummy-registry` index
[LOCKING] 4 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] normal-dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] host-dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] cfg-dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] alt-dep v0.0.1 (registry `dummy-registry`)

"#]]
            .unordered(),
        )
        .with_stdout_data(
            str![[r#"
{
  "packages": [
    {
      "name": "alt-dep",
      "dependencies": [],
      "...": "{...}"
    },
    {
      "name": "cfg-dep",
      "dependencies": [],
      "...": "{...}"
    },
    {
      "name": "foo",
      "dependencies": [
        {
          "name": "normal-dep",
          "target": null,
          "...": "{...}"
        },
        {
          "name": "cfg-dep",
          "target": "cfg(foobar)",
          "...": "{...}"
        },
        {
          "name": "alt-dep",
          "target": "wasm32-unknown-unknown",
          "...": "{...}"
        },
        {
          "name": "host-dep",
          "target": "[HOST_TARGET]",
          "...": "{...}"
        }
      ],
      "...": "{...}"
    },
    {
      "name": "host-dep",
      "dependencies": [],
      "...": "{...}"
    },
    {
      "name": "normal-dep",
      "dependencies": [],
      "...": "{...}"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#alt-dep@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#cfg-dep@0.0.1"
      },
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#alt-dep@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#cfg-dep@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": "wasm32-unknown-unknown"
              }
            ],
            "name": "alt_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#alt-dep@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": "cfg(foobar)"
              }
            ],
            "name": "cfg_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#cfg-dep@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": "[HOST_TARGET]"
              }
            ],
            "name": "host_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "normal_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.1.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "...": "{...}"
}
"#]]
            .is_json()
            .unordered(),
        )
        .run();
    clear();

    // Filter on alternate, removes cfg and host.
    p.cargo("metadata --filter-platform")
        .arg(alt_target)
        .with_stderr_data(
            str![[r#"
[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[DOWNLOADING] crates ...
[DOWNLOADED] normal-dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] host-dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] alt-dep v0.0.1 (registry `dummy-registry`)

"#]]
            .unordered(),
        )
        .with_stdout_data(
            str![[r#"
{
  "packages": [
    {
      "name": "alt-dep",
      "dependencies": [],
      "...": "{...}"
    },
    {
      "name": "foo",
      "dependencies": [
        {
          "name": "normal-dep",
          "target": null,
          "...": "{...}"
        },
        {
          "name": "cfg-dep",
          "target": "cfg(foobar)",
          "...": "{...}"
        },
        {
          "name": "alt-dep",
          "target": "wasm32-unknown-unknown",
          "...": "{...}"
        },
        {
          "name": "host-dep",
          "target": "[HOST_TARGET]",
          "...": "{...}"
        }
      ],
      "...": "{...}"
    },
    {
      "name": "normal-dep",
      "dependencies": [],
      "...": "{...}"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#alt-dep@0.0.1"
      },
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#alt-dep@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": "wasm32-unknown-unknown"
              }
            ],
            "name": "alt_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#alt-dep@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "normal_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.1.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "...": "{...}"
}
"#]]
            .is_json()
            .unordered(),
        )
        .run();
    clear();

    // Filter on host, removes alt and cfg.
    p.cargo("metadata --filter-platform")
        .arg(&host_target)
        .with_stderr_data(
            str![[r#"
[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[DOWNLOADING] crates ...
[DOWNLOADED] normal-dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] host-dep v0.0.1 (registry `dummy-registry`)

"#]]
            .unordered(),
        )
        .with_stdout_data(
            str![[r#"
{
  "packages": [
    {
      "name": "foo",
      "dependencies": [
        {
          "name": "normal-dep",
          "target": null,
          "...": "{...}"
        },
        {
          "name": "cfg-dep",
          "target": "cfg(foobar)",
          "...": "{...}"
        },
        {
          "name": "alt-dep",
          "target": "wasm32-unknown-unknown",
          "...": "{...}"
        },
        {
          "name": "host-dep",
          "target": "[HOST_TARGET]",
          "...": "{...}"
        }
      ],
      "...": "{...}"
    },
    {
      "name": "host-dep",
      "dependencies": [],
      "...": "{...}"
    },
    {
      "name": "normal-dep",
      "dependencies": [],
      "...": "{...}"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": "[HOST_TARGET]"
              }
            ],
            "name": "host_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "normal_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.1.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "...": "{...}"
}
"#]]
            .is_json()
            .unordered(),
        )
        .run();
    clear();

    // Filter host with cfg, removes alt only
    p.cargo("metadata --filter-platform")
        .arg(&host_target)
        .env("RUSTFLAGS", "--cfg=foobar")
        .with_stderr_data(
            str![[r#"
[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[DOWNLOADING] crates ...
[DOWNLOADED] normal-dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] host-dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] cfg-dep v0.0.1 (registry `dummy-registry`)

"#]]
            .unordered(),
        )
        .with_stdout_data(
            str![[r#"
{
  "packages": [
    {
      "name": "cfg-dep",
      "dependencies": [],
      "...": "{...}"
    },
    {
      "name": "foo",
      "dependencies": [
        {
          "name": "normal-dep",
          "target": null,
          "...": "{...}"
        },
        {
          "name": "cfg-dep",
          "target": "cfg(foobar)",
          "...": "{...}"
        },
        {
          "name": "alt-dep",
          "target": "wasm32-unknown-unknown",
          "...": "{...}"
        },
        {
          "name": "host-dep",
          "target": "[HOST_TARGET]",
          "...": "{...}"
        }
      ],
      "...": "{...}"
    },
    {
      "name": "host-dep",
      "dependencies": [],
      "...": "{...}"
    },
    {
      "name": "normal-dep",
      "dependencies": [],
      "...": "{...}"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#cfg-dep@0.0.1"
      },
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#cfg-dep@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": "cfg(foobar)"
              }
            ],
            "name": "cfg_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#cfg-dep@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": "[HOST_TARGET]"
              }
            ],
            "name": "host_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "normal_dep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.1.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#host-dep@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#normal-dep@0.0.1"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "...": "{...}"
}
"#]]
            .is_json()
            .unordered(),
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
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": "{...}",
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0"
      },
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0",
          "registry+https://github.com/rust-lang/crates.io-index#winapi@0.1.0"
        ],
        "deps": [
          {
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
            ],
            "name": "bar",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": "cfg(windows)"
              }
            ],
            "name": "winapi",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#winapi@0.1.0"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.1.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#winapi@0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn dep_kinds_workspace() {
    // Check for bug with duplicate dep kinds in a workspace.
    // If different members select different features for the same package,
    // they show up multiple times in the resolver `deps`.
    //
    // Here:
    //     foo -> dep
    //     bar -> foo[feat1] -> dep
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [features]
                feat1 = []

                [dependencies]
                dep = { path="dep" }

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"

            [dependencies]
            foo = { path="..", features=["feat1"] }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file("dep/Cargo.toml", &basic_lib_manifest("dep"))
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("metadata")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": "{...}",
  "resolve": {
    "nodes": [
      {
        "dependencies": [
          "path+[ROOTURL]/foo#0.1.0"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "foo",
            "pkg": "path+[ROOTURL]/foo#0.1.0"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo/bar#0.1.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "path+[ROOTURL]/foo/dep#0.5.0"
      },
      {
        "dependencies": [
          "path+[ROOTURL]/foo/dep#0.5.0"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "dep",
            "pkg": "path+[ROOTURL]/foo/dep#0.5.0"
          }
        ],
        "features": [
          "feat1"
        ],
        "id": "path+[ROOTURL]/foo#0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo/bar#0.1.0",
    "path+[ROOTURL]/foo#0.1.0",
    "path+[ROOTURL]/foo/dep#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn build_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("metadata")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": "{...}",
  "resolve": "{...}",
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/build-dir",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

// Creating non-utf8 path is an OS-specific pain, so let's run this only on
// linux, where arbitrary bytes work.
#[cfg(target_os = "linux")]
#[cargo_test]
fn cargo_metadata_non_utf8() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    let base = PathBuf::from(OsString::from_vec(vec![255]));

    let p = project()
        .no_manifest()
        .file(base.join("./src/lib.rs"), "")
        .file(base.join("./Cargo.toml"), &basic_lib_manifest("foo"))
        .build();

    p.cargo("metadata")
        .cwd(p.root().join(base))
        .arg("--format-version")
        .arg("1")
        .with_stderr_data(str![[r#"
[ERROR] path contains invalid UTF-8 characters

"#]])
        .with_status(101)
        .run();
}

// TODO: Consider using this test instead of the version without the 'artifact' suffix or merge them because they should be pretty much the same.
#[cargo_test]
fn workspace_metadata_with_dependencies_no_deps_artifact() {
    let p = project()
        // NOTE that 'artifact' isn't mentioned in the workspace here, yet it shows up as member.
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies]
                baz = { path = "../baz/" }
                baz-renamed = { path = "../baz/" }
                artifact = { path = "../artifact/", artifact = "bin" }
           "#,
        )
        .file("bar/src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file("baz/src/lib.rs", "")
        .file("artifact/Cargo.toml", &basic_bin_manifest("artifact"))
        .file("artifact/src/main.rs", "fn main() {}")
        .build();

    p.cargo("metadata --no-deps -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "artifact": {
            "kinds": [
              "bin"
            ],
            "lib": false,
            "target": null
          },
          "features": [],
          "kind": null,
          "name": "artifact",
          "optional": false,
          "path": "[ROOT]/foo/artifact",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "baz",
          "optional": false,
          "path": "[ROOT]/foo/baz",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "baz-renamed",
          "optional": false,
          "path": "[ROOT]/foo/baz",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/bar#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/bar/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/foo/bar/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/artifact#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/artifact/Cargo.toml",
      "metadata": null,
      "name": "artifact",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "artifact",
          "src_path": "[ROOT]/foo/artifact/src/main.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    },
    {
      "authors": [
        "wycats@example.com"
      ],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/baz#0.5.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/baz/Cargo.toml",
      "metadata": null,
      "name": "baz",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "baz",
          "src_path": "[ROOT]/foo/baz/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.5.0"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo/bar#0.5.0",
    "path+[ROOTURL]/foo/artifact#0.5.0",
    "path+[ROOTURL]/foo/baz#0.5.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo/bar#0.5.0",
    "path+[ROOTURL]/foo/artifact#0.5.0",
    "path+[ROOTURL]/foo/baz#0.5.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn versionless_packages() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"

                [dependencies]
                foobar = "0.0.1"
                baz = { path = "../baz/" }
           "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"

                [dependencies]
                foobar = "0.0.1"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();
    Package::new("foobar", "0.0.1").publish();

    p.cargo("metadata -q --format-version 1")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "baz",
          "optional": false,
          "path": "[ROOT]/foo/baz",
          "registry": null,
          "rename": null,
          "req": "*",
          "source": null,
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "foobar",
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
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/bar#0.0.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/bar/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": [],
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/foo/bar/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "foobar",
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
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/baz#0.0.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/baz/Cargo.toml",
      "metadata": null,
      "name": "baz",
      "publish": [],
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "baz",
          "src_path": "[ROOT]/foo/baz/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.0"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/foobar-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "foobar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "foobar",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/foobar-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [
          "path+[ROOTURL]/foo/baz#0.0.0",
          "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1"
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
            "pkg": "path+[ROOTURL]/foo/baz#0.0.0"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "foobar",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo/bar#0.0.0"
      },
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "foobar",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo/baz#0.0.0"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#foobar@0.0.1"
      }
    ],
    "root": null
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo/bar#0.0.0",
    "path+[ROOTURL]/foo/baz#0.0.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo/bar#0.0.0",
    "path+[ROOTURL]/foo/baz#0.0.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

/// Record how TOML-specific types are deserialized by `toml` so we can make sure we know if these change and
/// can have a conversation about what should be done.
#[cargo_test]
fn cargo_metadata_toml_types() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            "
[package]
name = 'foo'
edition = '2015'

[package.metadata]
offset-datetime = 1979-05-27T07:32:00Z
local-datetime = 1979-05-27T07:32:00
local-date = 1979-05-27
local-time = 1979-05-27
",
        )
        .build();

    p.cargo("metadata")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.0.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": {
        "local-date": {
          "$__toml_private_datetime": "1979-05-27"
        },
        "local-datetime": {
          "$__toml_private_datetime": "1979-05-27T07:32:00"
        },
        "local-time": {
          "$__toml_private_datetime": "1979-05-27"
        },
        "offset-datetime": {
          "$__toml_private_datetime": "1979-05-27T07:32:00Z"
        }
      },
      "name": "foo",
      "publish": [],
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
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
      ],
      "version": "0.0.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.0.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.0.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.0.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.0.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn metadata_ignores_build_target_configuration() -> anyhow::Result<()> {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"

                [target.'cfg(something)'.dependencies]
                foobar = "0.0.1"
           "#,
        )
        .file("src/lib.rs", "")
        .build();
    Package::new("foobar", "0.0.1").publish();

    let output1 = p
        .cargo("metadata -q --format-version 1")
        .exec_with_output()?;
    let output2 = p
        .cargo("metadata -q --format-version 1")
        .env("CARGO_BUILD_TARGET", rustc_host())
        .exec_with_output()?;
    assert!(
        output1.stdout == output2.stdout,
        "metadata should not change when `CARGO_BUILD_TARGET` is set",
    );
    Ok(())
}
