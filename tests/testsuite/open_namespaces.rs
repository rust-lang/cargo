use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `open-namespaces` is required

  The package requires the Cargo feature called `open-namespaces`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["open-namespaces"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#open-namespaces for more information about the status of this feature.

"#]])
        .run();
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
        .with_stdout_data(
            str![[r#"
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
  "id": "path+[ROOTURL]/foo#foo::bar@0.0.1",
  "keywords": [],
  "license": null,
  "license_file": null,
  "links": null,
  "manifest_path": "[ROOT]/foo/Cargo.toml",
  "metadata": null,
  "name": "foo::bar",
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
      "name": "foo::bar",
      "src_path": "[ROOT]/foo/src/lib.rs",
      "test": true
    }
  ],
  "version": "0.0.1"
}
"#]]
            .is_json(),
        )
        .with_stderr_data("")
        .run();
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
        .with_stdout_data(
            str![[r#"
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
  "id": "path+[ROOTURL]/foo#foo::bar@0.0.1",
  "keywords": [],
  "license": null,
  "license_file": null,
  "links": null,
  "manifest_path": "[ROOT]/foo/Cargo.toml",
  "metadata": null,
  "name": "foo::bar",
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
      "name": "foo::bar",
      "src_path": "[ROOT]/foo/src/main.rs",
      "test": true
    }
  ],
  "version": "0.0.1"
}
"#]]
            .is_json(),
        )
        .with_stderr_data("")
        .run();
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
        .file("src/bin/foo-bar/main.rs", "fn main() {}")
        .build();

    p.cargo("read-manifest")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .with_stdout_data(
            str![[r#"
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
  "id": "path+[ROOTURL]/foo#foo::bar@0.0.1",
  "keywords": [],
  "license": null,
  "license_file": null,
  "links": null,
  "manifest_path": "[ROOT]/foo/Cargo.toml",
  "metadata": null,
  "name": "foo::bar",
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
      "name": "foo::bar",
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
      "name": "foo-bar",
      "src_path": "[ROOT]/foo/src/bin/foo-bar/main.rs",
      "test": true
    }
  ],
  "version": "0.0.1"
}
"#]]
            .is_json(),
        )
        .with_stderr_data("")
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
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
        .with_stdout_data(
            str![[r#"
{
  "authors": [],
  "categories": [],
  "default_run": null,
  "dependencies": [],
  "description": null,
  "documentation": null,
  "edition": "2021",
  "features": {},
  "homepage": null,
  "id": "path+[ROOTURL]/foo/foo::bar.rs#foo::bar@0.0.0",
  "keywords": [],
  "license": null,
  "license_file": null,
  "links": null,
  "manifest_path": "[ROOT]/foo/foo::bar.rs",
  "metadata": null,
  "name": "foo::bar",
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
      "name": "foo::bar",
      "src_path": "[ROOT]/foo/foo::bar.rs",
      "test": true
    }
  ],
  "version": "0.0.0"
}
"#]]
            .is_json(),
        )
        .with_stderr_data("")
        .run();
}

#[cargo_test]
fn generate_pkgid_with_namespace() {
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

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .run();
    p.cargo("pkgid")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .with_stdout_data(str![[r#"
path+[ROOTURL]/foo#foo::bar@0.0.1

"#]])
        .with_stderr_data("")
        .run();
}

#[cargo_test]
fn update_spec_accepts_namespaced_name() {
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

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .run();
    p.cargo("update foo::bar")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .with_stdout_data(str![""])
        .with_stderr_data(str![[r#"
[LOCKING] 0 packages to latest compatible versions

"#]])
        .run();
}

#[cargo_test]
fn update_spec_accepts_namespaced_pkgid() {
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

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .run();
    p.cargo(&format!("update path+{}#foo::bar@0.0.1", p.url()))
        .masquerade_as_nightly_cargo(&["open-namespaces"])
        .with_stdout_data(str![""])
        .with_stderr_data(str![[r#"
[LOCKING] 0 packages to latest compatible versions

"#]])
        .run();
}

#[cargo_test]
#[cfg(unix)] // until we get proper packaging support
fn publish_namespaced() {
    use cargo_test_support::registry::RegistryBuilder;
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
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo::bar v0.0.1 ([ROOT]/foo)
[ERROR] failed to prepare local package for uploading

Caused by:
  cannot publish with `open-namespaces`

"#]])
        .run();
}
