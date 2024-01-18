//! Tests for the `cargo pkgid` command.

use cargo_test_support::basic_lib_manifest;
use cargo_test_support::compare;
use cargo_test_support::git;
use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn local() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2018"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile").run();

    p.cargo("pkgid foo")
        .with_stdout(format!(
            "path+file://[..]{}#0.1.0",
            p.root().to_str().unwrap()
        ))
        .run();

    // Bad file URL.
    p.cargo("pkgid ./Cargo.toml")
        .with_status(101)
        .with_stderr(
            "\
error: invalid package ID specification: `./Cargo.toml`

Caused by:
  package ID specification `./Cargo.toml` looks like a file path, maybe try file://[..]/Cargo.toml
",
        )
        .run();

    // Bad file URL with similar name.
    p.cargo("pkgid './bar'")
        .with_status(101)
        .with_stderr(
            "\
error: invalid package ID specification: `./bar`

<tab>Did you mean `bar`?

Caused by:
  package ID specification `./bar` looks like a file path, maybe try file://[..]/bar
",
        )
        .run();
}

#[cargo_test]
fn registry() {
    Package::new("crates-io", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                crates-io = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("cratesio", "")
        .build();

    p.cargo("generate-lockfile").run();

    p.cargo("pkgid crates-io")
        .with_stdout("registry+https://github.com/rust-lang/crates.io-index#crates-io@0.1.0")
        .run();

    // Bad URL.
    p.cargo("pkgid https://example.com/crates-io")
        .with_status(101)
        .with_stderr(
            "\
error: package ID specification `https://example.com/crates-io` did not match any packages
Did you mean one of these?

  crates-io@0.1.0
",
        )
        .run();

    // Bad name.
    p.cargo("pkgid crates_io")
        .with_status(101)
        .with_stderr(
            "\
error: package ID specification `crates_io` did not match any packages

<tab>Did you mean `crates-io`?
",
        )
        .run();
}

#[cargo_test]
fn multiple_versions() {
    Package::new("two-ver", "0.1.0").publish();
    Package::new("two-ver", "0.2.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                two-ver = "0.1.0"
                two-ver2 = { package = "two-ver", version = "0.2.0" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("cratesio", "")
        .build();

    p.cargo("generate-lockfile").run();

    p.cargo("pkgid two-ver:0.2.0")
        .with_stdout("registry+https://github.com/rust-lang/crates.io-index#two-ver@0.2.0")
        .run();

    // Incomplete version.
    p.cargo("pkgid two-ver@0")
        .with_status(101)
        .with_stderr(
            "\
error: There are multiple `two-ver` packages in your project, and the specification `two-ver@0` is ambiguous.
Please re-run this command with one of the following specifications:
  two-ver@0.1.0
  two-ver@0.2.0
",
        )
        .run();

    // Incomplete version.
    p.cargo("pkgid two-ver@0.2")
        .with_stdout(
            "\
registry+https://github.com/rust-lang/crates.io-index#two-ver@0.2.0
",
        )
        .run();

    // Ambiguous.
    p.cargo("pkgid two-ver")
        .with_status(101)
        .with_stderr(
            "\
error: There are multiple `two-ver` packages in your project, and the specification `two-ver` is ambiguous.
Please re-run this command with one of the following specifications:
  two-ver@0.1.0
  two-ver@0.2.0
",
        )
        .run();

    // Bad version.
    p.cargo("pkgid two-ver:0.3.0")
        .with_status(101)
        .with_stderr(
            "\
error: package ID specification `two-ver@0.3.0` did not match any packages
Did you mean one of these?

  two-ver@0.1.0
  two-ver@0.2.0
",
        )
        .run();
}

// Not for `cargo pkgid` but the `PackageIdSpec` format
#[cargo_test]
fn multiple_git_same_version() {
    // Test what happens if different packages refer to the same git repo with
    // different refs, and the package version is the same.
    let (xyz_project, xyz_repo) = git::new_repo("xyz", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("xyz"))
            .file("src/lib.rs", "fn example() {}")
    });
    let rev1 = xyz_repo.revparse_single("HEAD").unwrap().id();
    xyz_project.change_file("src/lib.rs", "pub fn example() {}");
    git::add(&xyz_repo);
    let rev2 = git::commit(&xyz_repo);
    // Both rev1 and rev2 point to version 0.1.0.

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    bar = {{ path = "bar" }}
                    xyz = {{ git = "{}", rev = "{}" }}

                "#,
                xyz_project.url(),
                rev1
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "bar"
                    version = "0.1.0"

                    [dependencies]
                    xyz = {{ git = "{}", rev = "{}" }}
                "#,
                xyz_project.url(),
                rev2
            ),
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check").run();
    p.cargo("tree")
        .with_stdout(&format!(
            "\
foo v0.1.0 ([..]/foo)
├── bar v0.1.0 ([..]/foo/bar)
│   └── xyz v0.5.0 (file://[..]/xyz?rev={}#{})
└── xyz v0.5.0 (file://[..]/xyz?rev={}#{})
",
            rev2,
            &rev2.to_string()[..8],
            rev1,
            &rev1.to_string()[..8]
        ))
        .run();
    // FIXME: This fails since xyz is ambiguous, but the
    // possible pkgids are also ambiguous.
    p.cargo("pkgid xyz")
        .with_status(101)
        .with_stderr(
            "\
error: There are multiple `xyz` packages in your project, and the specification `xyz` is ambiguous.
Please re-run this command with one of the following specifications:
  git+file://[..]/xyz?rev=[..]#0.5.0
  git+file://[..]/xyz?rev=[..]#0.5.0
",
        )
        .run();
    // TODO, what should the `-p` value be here?
    //p.cargo("update -p")
}

// Keep Package ID format in sync among
//
// * Package ID specifications
// * machine-readable message via `--message-format=json`
// * `cargo metadata` output
#[cargo_test]
fn pkgid_json_message_metadata_consistency() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "fn unused() {}")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile").run();

    let output = p.cargo("pkgid").arg("foo").exec_with_output().unwrap();
    let pkgid = String::from_utf8(output.stdout).unwrap();
    let pkgid = pkgid.trim();
    compare::assert_match_exact("path+file://[..]/foo#0.5.0", &pkgid);

    p.cargo("check --message-format=json")
        .with_json(
            &r#"
{
  "reason": "compiler-artifact",
  "package_id": "$PKGID",
  "manifest_path": "[..]",
  "target": "{...}",
  "profile": "{...}",
  "features": [],
  "filenames": "{...}",
  "executable": null,
  "fresh": false
}

{
  "reason": "build-script-executed",
  "package_id": "$PKGID",
  "linked_libs": [],
  "linked_paths": [],
  "cfgs": [],
  "env": [],
  "out_dir": "[..]"
}

{
  "manifest_path": "[..]",
  "message": "{...}",
  "package_id": "$PKGID",
  "reason": "compiler-message",
  "target": "{...}"
}

{
  "reason": "compiler-message",
  "package_id": "$PKGID",
  "manifest_path": "[..]",
  "target": "{...}",
  "message": "{...}"
}

{
  "reason": "compiler-artifact",
  "package_id": "$PKGID",
  "manifest_path": "[..]",
  "target": "{...}",
  "profile": "{...}",
  "features": [],
  "filenames": "{...}",
  "executable": null,
  "fresh": false
}

{
  "reason": "build-finished",
  "success": true
}
            "#
            .replace("$PKGID", pkgid),
        )
        .run();

    p.cargo("metadata")
        .with_json(
            &r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": "{...}",
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "$PKGID",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[..]",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": "{...}",
      "version": "0.5.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "$PKGID"
      }
    ],
    "root": "$PKGID"
  },
  "target_directory": "[..]",
  "version": 1,
  "workspace_default_members": [
    "$PKGID"
  ],
  "workspace_members": [
    "$PKGID"
  ],
  "workspace_root": "[..]"
}
            "#
            .replace("$PKGID", pkgid),
        )
        .run()
}
