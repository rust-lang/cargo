//! Duplicating tests for `cargo update --precise` with unstable-options
//! enabled. This will make sure we check backward compatibility when the
//! capability of making breaking changes has been implemented. When that
//! feature is stabilized, this file can be deleted.

use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, git, project, str};

#[cargo_test]
fn update_precise_downgrade() {
    Package::new("serde", "0.1.0").publish();
    Package::new("serde", "0.2.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                serde = "0.2"
                foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                serde = "0.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check").run();

    Package::new("serde", "0.2.0").publish();

    p.cargo("update -Zunstable-options serde:0.2.1 --precise 0.2.0")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNGRADING] serde v0.2.1 -> v0.2.0
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]])
        .run();
}

#[cargo_test]
fn update_precise_mismatched() {
    Package::new("serde", "1.2.0").publish();
    Package::new("serde", "1.2.1").publish();
    Package::new("serde", "1.6.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                serde = "~1.2"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();

    // `1.6.0` does not match `"~1.2"`
    p.cargo("update -Zunstable-options serde:1.2 --precise 1.6.0")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `serde = "~1.2"`
candidate versions found which didn't match: 1.6.0
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `bar v0.0.1 ([ROOT]/foo)`
perhaps a crate was updated and forgotten to be re-vendored?

"#]])
        .with_status(101)
        .run();

    // `1.9.0` does not exist
    p.cargo("update -Zunstable-options serde:1.2 --precise 1.9.0")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        // This terrible error message has been the same for a long time. A fix is more than welcome!
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `serde` found
location searched: registry `crates-io`
required by package `bar v0.0.1 ([ROOT]/foo)`

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn update_precise_build_metadata() {
    Package::new("serde", "0.0.1+first").publish();
    Package::new("serde", "0.0.1+second").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"

                [dependencies]
                serde = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("update -Zunstable-options serde --precise 0.0.1+first")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .run();

    p.cargo("update -Zunstable-options serde --precise 0.0.1+second")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] serde v0.0.1+first -> v0.0.1+second

"#]])
        .run();

    // This is not considered "Downgrading". Build metadata are not assumed to
    // be ordered.
    p.cargo("update -Zunstable-options serde --precise 0.0.1+first")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] serde v0.0.1+second -> v0.0.1+first

"#]])
        .run();
}

#[cargo_test]
fn update_precise_do_not_force_update_deps() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.2.1").dep("log", "0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                serde = "0.2"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();

    Package::new("log", "0.1.1").publish();
    Package::new("serde", "0.2.2").dep("log", "0.1").publish();

    p.cargo("update -Zunstable-options serde:0.2.1 --precise 0.2.2")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] serde v0.2.1 -> v0.2.2
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]])
        .run();
}

#[cargo_test]
fn update_recursive_conflicts_with_precise() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.2.1").dep("log", "0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                serde = "0.2"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();

    Package::new("log", "0.1.1").publish();
    Package::new("serde", "0.2.2").dep("log", "0.1").publish();

    p.cargo("update -Zunstable-options serde:0.2.1 --precise 0.2.2 --recursive")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--precise <PRECISE>' cannot be used with '--recursive'

Usage: cargo[EXE] update -Z <FLAG> --precise <PRECISE> <SPEC|--package [<SPEC>]>

For more information, try '--help'.

"#]])
        .run();
}

// cargo update should respect its arguments even without a lockfile.
// See issue "Running cargo update without a Cargo.lock ignores arguments"
// at <https://github.com/rust-lang/cargo/issues/6872>.
#[cargo_test]
fn update_precise_first_run() {
    Package::new("serde", "0.1.0").publish();
    Package::new("serde", "0.2.0").publish();
    Package::new("serde", "0.2.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                serde = "0.2"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("update -Zunstable-options serde --precise 0.2.0")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNGRADING] serde v0.2.1 -> v0.2.0

"#]])
        .run();

    // Assert `cargo metadata` shows serde 0.2.0
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
          "name": "serde",
          "optional": false,
          "registry": null,
          "rename": null,
          "req": "^0.2",
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
      "id": "path+[ROOTURL]/foo#bar@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
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
          "src_path": "[ROOT]/foo/src/lib.rs",
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
      "id": "registry+https://github.com/rust-lang/crates.io-index#serde@0.2.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/serde-0.2.0/Cargo.toml",
      "metadata": null,
      "name": "serde",
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
          "name": "serde",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/serde-0.2.0/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.2.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#serde@0.2.0"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "serde",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#serde@0.2.0"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#bar@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#serde@0.2.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#bar@0.0.1"
  },
  "target_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#bar@0.0.1"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#bar@0.0.1"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .json(),
        )
        .run();

    p.cargo("update -Zunstable-options serde --precise 0.2.0")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index

"#]])
        .run();
}

#[cargo_test]
fn precise_with_build_metadata() {
    // +foo syntax shouldn't be necessary with --precise
    Package::new("bar", "0.1.0+extra-stuff.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("generate-lockfile").run();
    Package::new("bar", "0.1.1+extra-stuff.1").publish();
    Package::new("bar", "0.1.2+extra-stuff.2").publish();

    p.cargo("update -Zunstable-options bar --precise 0.1")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid version format for precise version `0.1`

Caused by:
  unexpected end of input while parsing minor version number

"#]])
        .run();

    p.cargo("update -Zunstable-options bar --precise 0.1.1+does-not-match")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `bar` found
location searched: registry `crates-io`
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();

    p.cargo("update -Zunstable-options bar --precise 0.1.1")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] bar v0.1.0+extra-stuff.0 -> v0.1.1+extra-stuff.1

"#]])
        .run();

    Package::new("bar", "0.1.3").publish();
    p.cargo("update -Zunstable-options bar --precise 0.1.3+foo")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `bar` found
location searched: registry `crates-io`
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();

    p.cargo("update -Zunstable-options bar --precise 0.1.3")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] bar v0.1.1+extra-stuff.1 -> v0.1.3

"#]])
        .run();
}

#[cargo_test]
fn update_precise_git_revisions() {
    let (git_project, git_repo) = git::new_repo("git", |p| {
        p.file("Cargo.toml", &basic_lib_manifest("git"))
            .file("src/lib.rs", "")
    });
    let tag_name = "Nazgûl";
    git::tag(&git_repo, tag_name);
    let tag_commit_id = git_repo.head().unwrap().target().unwrap().to_string();

    git_project.change_file("src/lib.rs", "fn f() {}");
    git::add(&git_repo);
    let head_id = git::commit(&git_repo).to_string();
    let short_id = &head_id[..8];
    let url = git_project.url();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"
                    edition = "2015"

                    [dependencies]
                    git = {{ git = '{url}' }}
                "#
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/git`
[LOCKING] 2 packages to latest compatible versions

"#]])
        .run();

    assert!(p.read_lockfile().contains(&head_id));

    p.cargo("update -Zunstable-options git --precise")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .arg(tag_name)
        .with_stderr_data(format!(
            "\
[UPDATING] git repository `[ROOTURL]/git`
[UPDATING] git v0.5.0 ([ROOTURL]/git#[..]) -> #{}
",
            &tag_commit_id[..8],
        ))
        .run();

    assert!(p.read_lockfile().contains(&tag_commit_id));
    assert!(!p.read_lockfile().contains(&head_id));

    p.cargo("update -Zunstable-options git --precise")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .arg(short_id)
        .with_stderr_data(format!(
            "\
[UPDATING] git repository `[ROOTURL]/git`
[UPDATING] git v0.5.0 ([ROOTURL]/git[..]) -> #{short_id}
",
        ))
        .run();

    assert!(p.read_lockfile().contains(&head_id));
    assert!(!p.read_lockfile().contains(&tag_commit_id));

    // updating back to tag still requires a git fetch,
    // as the ref may change over time.
    p.cargo("update -Zunstable-options git --precise")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .arg(tag_name)
        .with_stderr_data(format!(
            "\
[UPDATING] git repository `[ROOTURL]/git`
[UPDATING] git v0.5.0 ([ROOTURL]/git#[..]) -> #{}
",
            &tag_commit_id[..8],
        ))
        .run();

    assert!(p.read_lockfile().contains(&tag_commit_id));
    assert!(!p.read_lockfile().contains(&head_id));

    // Now make a tag looks like an oid.
    // It requires a git fetch, as the oid cannot be found in preexisting git db.
    let arbitrary_tag: String = std::iter::repeat('a').take(head_id.len()).collect();
    git::tag(&git_repo, &arbitrary_tag);

    p.cargo("update -Zunstable-options git --precise")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .arg(&arbitrary_tag)
        .with_stderr_data(format!(
            "\
[UPDATING] git repository `[ROOTURL]/git`
[UPDATING] git v0.5.0 ([ROOTURL]/git#[..]) -> #{}
",
            &head_id[..8],
        ))
        .run();

    assert!(p.read_lockfile().contains(&head_id));
    assert!(!p.read_lockfile().contains(&tag_commit_id));
}

#[cargo_test]
fn update_precise_yanked() {
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.1.1").yanked(true).publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"

                [dependencies]
                bar = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    // Use non-yanked version.
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"bar\"\nversion = \"0.1.0\""));

    p.cargo("update -Zunstable-options --precise 0.1.1 bar")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[WARNING] selected package `bar@0.1.1` was yanked by the author
[NOTE] if possible, try a compatible non-yanked version
[UPDATING] bar v0.1.0 -> v0.1.1

"#]])
        .run();

    // Use yanked version.
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"bar\"\nversion = \"0.1.1\""));
}

#[cargo_test]
fn update_precise_yanked_multiple_presence() {
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.1.1").yanked(true).publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"

                [dependencies]
                bar = "0.1"
                baz = { package = "bar", version = "0.1" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    // Use non-yanked version.
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"bar\"\nversion = \"0.1.0\""));

    p.cargo("update -Zunstable-options --precise 0.1.1 bar")
        .masquerade_as_nightly_cargo(&["update-precise-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[WARNING] selected package `bar@0.1.1` was yanked by the author
[NOTE] if possible, try a compatible non-yanked version
[UPDATING] bar v0.1.0 -> v0.1.1

"#]])
        .run();

    // Use yanked version.
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"bar\"\nversion = \"0.1.1\""));
}
