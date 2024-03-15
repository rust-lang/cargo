use cargo_test_support::project;
use cargo_test_support::registry::RegistryBuilder;

#[cargo_test]
fn within_namespace_requires_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo::bar"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("read-manifest")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `:` in package name: `foo::bar`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
 --> Cargo.toml:3:24
  |
3 |                 name = \"foo::bar\"
  |                        ^^^^^^^^^^
  |
",
        )
        .run()
}

#[cargo_test]
fn implicit_lib_within_namespace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["open-namespaces"]

                [package]
                name = "foo::bar"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("read-manifest")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `:` in package name: `foo::bar`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
 --> Cargo.toml:5:24
  |
5 |                 name = \"foo::bar\"
  |                        ^^^^^^^^^^
  |
",
        )
        .run()
}

#[cargo_test]
fn implicit_bin_within_namespace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["open-namespaces"]

                [package]
                name = "foo::bar"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("read-manifest")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `:` in package name: `foo::bar`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
 --> Cargo.toml:5:24
  |
5 |                 name = \"foo::bar\"
  |                        ^^^^^^^^^^
  |
",
        )
        .run()
}

#[cargo_test]
fn explicit_bin_within_namespace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["open-namespaces"]

                [package]
                name = "foo::bar"
                version = "0.0.1"
                edition = "2015"

                [[bin]]
                name = "foo-bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/foo-bar/main.rs", "fn main() {}")
        .build();

    p.cargo("read-manifest")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `:` in package name: `foo::bar`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
 --> Cargo.toml:5:24
  |
5 |                 name = \"foo::bar\"
  |                        ^^^^^^^^^^
  |
",
        )
        .run()
}

#[cargo_test]
#[cfg(unix)]
fn namespaced_script_name() {
    let p = cargo_test_support::project()
        .file(
            "foo::bar.rs",
            r#"---
cargo-features = ["open-namespaces"]
package.edition = "2021"
---

fn main() {}
"#,
        )
        .build();

    p.cargo("read-manifest -Zscript --manifest-path foo::bar.rs")
        .masquerade_as_nightly_cargo(&["script", "open-namespaces"])
        .with_json(
            r#"{
  "authors": [],
  "categories": [],
  "default_run": null,
  "dependencies": [],
  "description": null,
  "documentation": null,
  "edition": "2021",
  "features": {},
  "homepage": null,
  "id": "path+file://[..]#foo--bar@0.0.0",
  "keywords": [],
  "license": null,
  "license_file": null,
  "links": null,
  "manifest_path": "[CWD]/foo::bar.rs",
  "metadata": null,
  "name": "foo--bar",
  "publish": [],
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
      "edition": "2021",
      "kind": [
        "bin"
      ],
      "name": "foo--bar",
      "src_path": "[..]/foo::bar.rs",
      "test": true
    }
  ],
  "version": "0.0.0"
}
"#,
        )
        .with_stderr(
            "\
",
        )
        .run();
}

#[cargo_test]
fn publish_namespaced() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["open-namespaces"]

                [package]
                name = "foo::bar"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/lib.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .masquerade_as_nightly_cargo(&["script", "open-namespaces"])
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `:` in package name: `foo::bar`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
 --> Cargo.toml:5:24
  |
5 |                 name = \"foo::bar\"
  |                        ^^^^^^^^^^
  |
",
        )
        .run();
}
