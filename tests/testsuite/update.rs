//! Tests for the `cargo update` command.

use crate::prelude::*;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::registry::{self};
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{basic_lib_manifest, basic_manifest, git, project, str};

#[cargo_test]
fn minor_update_two_places() {
    Package::new("log", "0.1.0").publish();
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
                log = "0.1"
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
                log = "0.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check").run();
    Package::new("log", "0.1.1").publish();

    p.change_file(
        "foo/Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []

            [dependencies]
            log = "0.1.1"
        "#,
    );

    p.cargo("check").run();
}

#[cargo_test]
fn transitive_minor_update() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.1.0").dep("log", "0.1").publish();

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
                serde = "0.1"
                log = "0.1"
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

    Package::new("log", "0.1.1").publish();
    Package::new("serde", "0.1.1").dep("log", "0.1.1").publish();

    // Note that `serde` isn't actually updated here! The default behavior for
    // `update` right now is to as conservatively as possible attempt to satisfy
    // an update. In this case we previously locked the dependency graph to `log
    // 0.1.0`, but nothing on the command line says we're allowed to update
    // that. As a result the update of `serde` here shouldn't update to `serde
    // 0.1.1` as that would also force an update to `log 0.1.1`.
    //
    // Also note that this is probably counterintuitive and weird. We may wish
    // to change this one day.
    p.cargo("update serde")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 0 packages to latest compatible versions
[NOTE] pass `--verbose` to see 2 unchanged dependencies behind latest

"#]])
        .run();
}

#[cargo_test]
fn conservative() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.1.0").dep("log", "0.1").publish();

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
                serde = "0.1"
                log = "0.1"
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

    Package::new("log", "0.1.1").publish();
    Package::new("serde", "0.1.1").dep("log", "0.1").publish();

    p.cargo("update serde")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] serde v0.1.0 -> v0.1.1
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]])
        .run();
}

#[cargo_test]
fn update_via_new_dep() {
    Package::new("log", "0.1.0").publish();
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
                log = "0.1"
                # foo = { path = "foo" }
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
                log = "0.1.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check").run();
    Package::new("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    p.cargo("check").env("CARGO_LOG", "cargo=trace").run();
}

#[cargo_test]
fn update_via_new_member() {
    Package::new("log", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [workspace]
                # members = [ "foo" ]

                [dependencies]
                log = "0.1"
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
                log = "0.1.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check").run();
    Package::new("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    p.cargo("check").run();
}

#[cargo_test]
fn add_dep_deep_new_requirement() {
    Package::new("log", "0.1.0").publish();
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
                log = "0.1"
                # bar = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();

    Package::new("log", "0.1.1").publish();
    Package::new("bar", "0.1.0").dep("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    p.cargo("check").run();
}

#[cargo_test]
fn everything_real_deep() {
    Package::new("log", "0.1.0").publish();
    Package::new("foo", "0.1.0").dep("log", "0.1").publish();
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
                foo = "0.1"
                # bar = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();

    Package::new("log", "0.1.1").publish();
    Package::new("bar", "0.1.0").dep("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    p.cargo("check").run();
}

#[cargo_test]
fn change_package_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a-foo"
                version = "0.2.0-alpha"
                edition = "2015"
                authors = []

                [dependencies]
                bar = { path = "bar", version = "0.2.0-alpha" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.2.0-alpha"))
        .file("bar/src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
                [[package]]
                name = "foo"
                version = "0.2.0"
                dependencies = ["bar 0.2.0"]

                [[package]]
                name = "bar"
                version = "0.2.0"
            "#,
        )
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn update_precise() {
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

    p.cargo("update serde:0.2.1 --precise 0.2.0")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNGRADING] serde v0.2.1 -> v0.2.0

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
    p.cargo("update serde:1.2 --precise 1.6.0")
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
    p.cargo("update serde:1.2 --precise 1.9.0")
        // This terrible error message has been the same for a long time. A fix is more than welcome!
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `serde` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
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
    p.cargo("update serde --precise 0.0.1+first").run();

    p.cargo("update serde --precise 0.0.1+second")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] serde v0.0.1+first -> v0.0.1+second

"#]])
        .run();

    // This is not considered "Downgrading". Build metadata are not assumed to
    // be ordered.
    p.cargo("update serde --precise 0.0.1+first")
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

    p.cargo("update serde:0.2.1 --precise 0.2.2")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] serde v0.2.1 -> v0.2.2
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]])
        .run();
}

#[cargo_test]
fn update_recursive() {
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

    p.cargo("update serde:0.2.1 --recursive")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[UPDATING] log v0.1.0 -> v0.1.1
[UPDATING] serde v0.2.1 -> v0.2.2

"#]])
        .run();
}

#[cargo_test]
fn update_aggressive_alias_for_recursive() {
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

    p.cargo("update serde:0.2.1 --aggressive")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[UPDATING] log v0.1.0 -> v0.1.1
[UPDATING] serde v0.2.1 -> v0.2.2

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

    p.cargo("update serde:0.2.1 --precise 0.2.2 --recursive")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--precise <PRECISE>' cannot be used with '--recursive'

Usage: cargo[EXE] update --precise <PRECISE> <SPEC|--package [<SPEC>]>

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

    p.cargo("update serde --precise 0.2.0")
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
  "build_directory": "[ROOT]/foo/target",
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
            .is_json(),
        )
        .run();

    p.cargo("update serde --precise 0.2.0")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index

"#]])
        .run();
}

#[cargo_test]
fn preserve_top_comment() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("update").run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.starts_with("# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\n"));

    let mut lines = lockfile.lines().collect::<Vec<_>>();
    lines.insert(2, "# some other comment");
    let mut lockfile = lines.join("\n");
    lockfile.push('\n'); // .lines/.join loses the last newline
    println!("saving Cargo.lock contents:\n{}", lockfile);

    p.change_file("Cargo.lock", &lockfile);

    p.cargo("update").run();

    let lockfile2 = p.read_lockfile();
    println!("loaded Cargo.lock contents:\n{}", lockfile2);

    assert_eq!(lockfile, lockfile2);
}

#[cargo_test]
fn dry_run_update() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.1.0").dep("log", "0.1").publish();

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
                serde = "0.1"
                log = "0.1"
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
    let old_lockfile = p.read_lockfile();

    Package::new("log", "0.1.1").publish();
    Package::new("serde", "0.1.1").dep("log", "0.1").publish();

    p.cargo("update serde --dry-run")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] serde v0.1.0 -> v0.1.1
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest
[WARNING] not updating lockfile due to dry run

"#]])
        .run();
    let new_lockfile = p.read_lockfile();
    assert_eq!(old_lockfile, new_lockfile)
}

#[cargo_test]
fn workspace_only() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("generate-lockfile").run();
    let lock1 = p.read_lockfile();

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.2"
            edition = "2015"
        "#,
    );
    p.cargo("update --workspace").run();
    let lock2 = p.read_lockfile();

    assert_ne!(lock1, lock2);
    assert!(lock1.contains("0.0.1"));
    assert!(lock2.contains("0.0.2"));
    assert!(!lock1.contains("0.0.2"));
    assert!(!lock2.contains("0.0.1"));
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

    p.cargo("update bar --precise 0.1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid version format for precise version `0.1`

Caused by:
  unexpected end of input while parsing minor version number

"#]])
        .run();

    p.cargo("update bar --precise 0.1.1+does-not-match")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `bar` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();

    p.cargo("update bar --precise 0.1.1")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] bar v0.1.0+extra-stuff.0 -> v0.1.1+extra-stuff.1

"#]])
        .run();

    Package::new("bar", "0.1.3").publish();
    p.cargo("update bar --precise 0.1.3+foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `bar` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();

    p.cargo("update bar --precise 0.1.3")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] bar v0.1.1+extra-stuff.1 -> v0.1.3

"#]])
        .run();
}

#[cargo_test]
fn update_only_members_order_one() {
    let git_project = git::new("rustdns", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("rustdns"))
            .file("src/lib.rs", "pub fn bar() {}")
    });

    let workspace_toml = format!(
        r#"
[workspace.package]
version = "2.29.8"
edition = "2021"
publish = false

[workspace]
members = [
    "rootcrate",
    "subcrate",
]
resolver = "2"

[workspace.dependencies]
# Internal crates
subcrate = {{ version = "*", path = "./subcrate" }}

# External dependencies
rustdns = {{ version = "0.5.0", default-features = false, git = "{}" }}
                "#,
        git_project.url()
    );
    let p = project()
        .file("Cargo.toml", &workspace_toml)
        .file(
            "rootcrate/Cargo.toml",
            r#"
[package]
name = "rootcrate"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
subcrate.workspace = true
"#,
        )
        .file("rootcrate/src/main.rs", "fn main() {}")
        .file(
            "subcrate/Cargo.toml",
            r#"
[package]
name = "subcrate"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
rustdns.workspace = true
"#,
        )
        .file("subcrate/src/lib.rs", "pub foo() {}")
        .build();

    // First time around we should compile both foo and bar
    p.cargo("generate-lockfile")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/rustdns`
[LOCKING] 1 package to latest compatible version

"#]])
        .run();
    // Modify a file manually, shouldn't trigger a recompile
    git_project.change_file("src/lib.rs", r#"pub fn bar() { println!("hello!"); }"#);
    // Commit the changes and make sure we don't trigger a recompile because the
    // lock file says not to change
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);
    p.change_file("Cargo.toml", &workspace_toml.replace("2.29.8", "2.29.81"));

    p.cargo("update -p rootcrate")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[UPDATING] rootcrate v2.29.8 ([ROOT]/foo/rootcrate) -> v2.29.81
[UPDATING] subcrate v2.29.8 ([ROOT]/foo/subcrate) -> v2.29.81

"#]])
        .run();
}

#[cargo_test]
fn update_only_members_order_two() {
    let git_project = git::new("rustdns", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("rustdns"))
            .file("src/lib.rs", "pub fn bar() {}")
    });

    let workspace_toml = format!(
        r#"
[workspace.package]
version = "2.29.8"
edition = "2021"
publish = false

[workspace]
members = [
    "crate2",
    "crate1",
]
resolver = "2"

[workspace.dependencies]
# Internal crates
crate1 = {{ version = "*", path = "./crate1" }}

# External dependencies
rustdns = {{ version = "0.5.0", default-features = false, git = "{}" }}
                "#,
        git_project.url()
    );
    let p = project()
        .file("Cargo.toml", &workspace_toml)
        .file(
            "crate2/Cargo.toml",
            r#"
[package]
name = "crate2"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
crate1.workspace = true
"#,
        )
        .file("crate2/src/main.rs", "fn main() {}")
        .file(
            "crate1/Cargo.toml",
            r#"
[package]
name = "crate1"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
rustdns.workspace = true
"#,
        )
        .file("crate1/src/lib.rs", "pub foo() {}")
        .build();

    // First time around we should compile both foo and bar
    p.cargo("generate-lockfile")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/rustdns`
[LOCKING] 1 package to latest compatible version

"#]])
        .run();
    // Modify a file manually, shouldn't trigger a recompile
    git_project.change_file("src/lib.rs", r#"pub fn bar() { println!("hello!"); }"#);
    // Commit the changes and make sure we don't trigger a recompile because the
    // lock file says not to change
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);
    p.change_file("Cargo.toml", &workspace_toml.replace("2.29.8", "2.29.81"));

    p.cargo("update -p crate2")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[UPDATING] crate1 v2.29.8 ([ROOT]/foo/crate1) -> v2.29.81
[UPDATING] crate2 v2.29.8 ([ROOT]/foo/crate2) -> v2.29.81

"#]])
        .run();
}

#[cargo_test]
fn update_only_members_with_workspace() {
    let git_project = git::new("rustdns", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("rustdns"))
            .file("src/lib.rs", "pub fn bar() {}")
    });

    let workspace_toml = format!(
        r#"
[workspace.package]
version = "2.29.8"
edition = "2021"
publish = false

[workspace]
members = [
    "crate2",
    "crate1",
]
resolver = "2"

[workspace.dependencies]
# Internal crates
crate1 = {{ version = "*", path = "./crate1" }}

# External dependencies
rustdns = {{ version = "0.5.0", default-features = false, git = "{}" }}
                "#,
        git_project.url()
    );
    let p = project()
        .file("Cargo.toml", &workspace_toml)
        .file(
            "crate2/Cargo.toml",
            r#"
[package]
name = "crate2"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
crate1.workspace = true
"#,
        )
        .file("crate2/src/main.rs", "fn main() {}")
        .file(
            "crate1/Cargo.toml",
            r#"
[package]
name = "crate1"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
rustdns.workspace = true
"#,
        )
        .file("crate1/src/lib.rs", "pub foo() {}")
        .build();

    // First time around we should compile both foo and bar
    p.cargo("generate-lockfile")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/rustdns`
[LOCKING] 1 package to latest compatible version

"#]])
        .run();
    // Modify a file manually, shouldn't trigger a recompile
    git_project.change_file("src/lib.rs", r#"pub fn bar() { println!("hello!"); }"#);
    // Commit the changes and make sure we don't trigger a recompile because the
    // lock file says not to change
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);
    p.change_file("Cargo.toml", &workspace_toml.replace("2.29.8", "2.29.81"));

    p.cargo("update --workspace")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[UPDATING] crate1 v2.29.8 ([ROOT]/foo/crate1) -> v2.29.81
[UPDATING] crate2 v2.29.8 ([ROOT]/foo/crate2) -> v2.29.81

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
[LOCKING] 1 package to latest compatible version

"#]])
        .run();

    assert!(p.read_lockfile().contains(&head_id));

    p.cargo("update git --precise")
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

    p.cargo("update git --precise")
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
    p.cargo("update git --precise")
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
    let arbitrary_tag: String = "a".repeat(head_id.len());
    git::tag(&git_repo, &arbitrary_tag);

    p.cargo("update git --precise")
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
fn precise_yanked() {
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

    p.cargo("update --precise 0.1.1 bar")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[WARNING] selected package `bar@0.1.1` was yanked by the author
  |
  = [HELP] if possible, try a compatible non-yanked version
[UPDATING] bar v0.1.0 -> v0.1.1

"#]])
        .run();

    // Use yanked version.
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"bar\"\nversion = \"0.1.1\""));
}

#[cargo_test]
fn precise_yanked_multiple_presence() {
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

    p.cargo("update --precise 0.1.1 bar")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[WARNING] selected package `bar@0.1.1` was yanked by the author
  |
  = [HELP] if possible, try a compatible non-yanked version
[UPDATING] bar v0.1.0 -> v0.1.1

"#]])
        .run();

    // Use yanked version.
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"bar\"\nversion = \"0.1.1\""));
}

#[cargo_test]
fn report_behind() {
    Package::new("two-ver", "0.1.0").publish();
    Package::new("two-ver", "0.2.0").publish();
    Package::new("pre", "1.0.0-alpha.0").publish();
    Package::new("pre", "1.0.0-alpha.1").publish();
    Package::new("breaking", "0.1.0").publish();
    Package::new("breaking", "0.2.0").publish();
    Package::new("breaking", "0.2.1-alpha.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"

                [dependencies]
                breaking = "0.1"
                pre = "=1.0.0-alpha.0"
                two-ver = "0.2.0"
                two-ver-one = { version = "0.1.0", package = "two-ver" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    Package::new("breaking", "0.1.1").publish();

    p.cargo("update --dry-run")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] breaking v0.1.0 -> v0.1.1 (available: v0.2.0)
[NOTE] pass `--verbose` to see 2 unchanged dependencies behind latest
[WARNING] not updating lockfile due to dry run

"#]])
        .run();

    p.cargo("update --dry-run --verbose")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] breaking v0.1.0 -> v0.1.1 (available: v0.2.0)
[UNCHANGED] pre v1.0.0-alpha.0 (available: v1.0.0-alpha.1)
[UNCHANGED] two-ver v0.1.0 (available: v0.2.0)
[NOTE] to see how you depend on a package, run `cargo tree --invert <dep>@<ver>`
[WARNING] not updating lockfile due to dry run

"#]])
        .run();

    p.cargo("update").run();

    p.cargo("update --dry-run")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 0 packages to latest compatible versions
[NOTE] pass `--verbose` to see 3 unchanged dependencies behind latest
[WARNING] not updating lockfile due to dry run

"#]])
        .run();

    p.cargo("update --dry-run --verbose")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 0 packages to latest compatible versions
[UNCHANGED] breaking v0.1.1 (available: v0.2.0)
[UNCHANGED] pre v1.0.0-alpha.0 (available: v1.0.0-alpha.1)
[UNCHANGED] two-ver v0.1.0 (available: v0.2.0)
[NOTE] to see how you depend on a package, run `cargo tree --invert <dep>@<ver>`
[WARNING] not updating lockfile due to dry run

"#]])
        .run();
}

#[cargo_test]
fn update_with_missing_feature() {
    // Attempting to update a package to a version with a missing feature
    // should produce a warning.
    Package::new("bar", "0.1.0").feature("feat1", &[]).publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar = {version="0.1", features=["feat1"]}
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("generate-lockfile").run();

    // Publish an update that is missing the feature.
    Package::new("bar", "0.1.1").publish();

    p.cargo("update")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 0 packages to latest compatible versions
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]])
        .run();

    // Publish a fixed version, should not warn.
    Package::new("bar", "0.1.2").feature("feat1", &[]).publish();
    p.cargo("update")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.1.0 -> v0.1.2

"#]])
        .run();
}

#[cargo_test]
fn update_breaking_unstable() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name  =  "foo"
                version  =  "0.0.1"
                edition  =  "2015"
                authors  =  []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("update --breaking")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `--breaking` flag is unstable, pass `-Z unstable-options` to enable it
See https://github.com/rust-lang/cargo/issues/12425 for more information about the `--breaking` flag.

"#]])
        .run();
}

#[cargo_test]
fn update_breaking_dry_run() {
    Package::new("incompatible", "1.0.0").publish();
    Package::new("ws", "1.0.0").publish();

    let root_manifest = r#"
        # Check if formatting is preserved. Nothing here should change, due to dry-run.

        [workspace]
        members  =  ["foo"]

        [workspace.dependencies]
        ws  =  "1.0"  # Preserve formatting
    "#;

    let crate_manifest = r#"
        # Check if formatting is preserved. Nothing here should change, due to dry-run.

        [package]
        name  =  "foo"
        version  =  "0.0.1"
        edition  =  "2015"
        authors  =  []

        [dependencies]
        incompatible  =  "1.0"  # Preserve formatting
        ws.workspace  =  true  # Preserve formatting
    "#;

    let p = project()
        .file("Cargo.toml", root_manifest)
        .file("foo/Cargo.toml", crate_manifest)
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    let lock_file = p.read_file("Cargo.lock");

    Package::new("incompatible", "1.0.1").publish();
    Package::new("ws", "1.0.1").publish();

    Package::new("incompatible", "2.0.0").publish();
    Package::new("ws", "2.0.0").publish();

    p.cargo("update -Zunstable-options --dry-run --breaking")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPGRADING] incompatible ^1.0 -> ^2.0
[UPGRADING] ws ^1.0 -> ^2.0
[LOCKING] 2 packages to latest compatible versions
[UPDATING] incompatible v1.0.0 -> v2.0.0
[UPDATING] ws v1.0.0 -> v2.0.0
[WARNING] aborting update due to dry run

"#]])
        .run();

    let root_manifest_after = p.read_file("Cargo.toml");
    assert_e2e().eq(&root_manifest_after, root_manifest);

    let crate_manifest_after = p.read_file("foo/Cargo.toml");
    assert_e2e().eq(&crate_manifest_after, crate_manifest);

    let lock_file_after = p.read_file("Cargo.lock");
    assert_e2e().eq(&lock_file_after, lock_file);
}

#[cargo_test]
fn update_breaking() {
    registry::alt_init();
    Package::new("compatible", "1.0.0").publish();
    Package::new("incompatible", "1.0.0").publish();
    Package::new("pinned", "1.0.0").publish();
    Package::new("less-than", "1.0.0").publish();
    Package::new("renamed-from", "1.0.0").publish();
    Package::new("pre-release", "1.0.0").publish();
    Package::new("yanked", "1.0.0").publish();
    Package::new("ws", "1.0.0").publish();
    Package::new("shared", "1.0.0").publish();
    Package::new("multiple-locations", "1.0.0").publish();
    Package::new("multiple-versions", "1.0.0").publish();
    Package::new("multiple-versions", "2.0.0").publish();
    Package::new("alternative-1", "1.0.0")
        .alternative(true)
        .publish();
    Package::new("alternative-2", "1.0.0")
        .alternative(true)
        .publish();
    Package::new("bar", "1.0.0").alternative(true).publish();
    Package::new("multiple-registries", "1.0.0").publish();
    Package::new("multiple-registries", "2.0.0")
        .alternative(true)
        .publish();
    Package::new("multiple-source-types", "1.0.0").publish();
    Package::new("platform-specific", "1.0.0").publish();
    Package::new("dev", "1.0.0").publish();
    Package::new("build", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                # Check if formatting is preserved

                [workspace]
                members  =  ["foo", "bar"]

                [workspace.dependencies]
                ws  =  "1.0"  # This line gets partially rewritten
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                # Check if formatting is preserved

                [package]
                name  =  "foo"
                version  =  "0.0.1"
                edition  =  "2015"
                authors  =  []

                [dependencies]
                compatible  =  "1.0"  # Comment
                incompatible  =  "1.0"  # Comment
                pinned  =  "=1.0"  # Comment
                less-than  =  "<99.0"  # Comment
                renamed-to  =  { package  =  "renamed-from", version  =  "1.0" }  # Comment
                pre-release  =  "1.0"  # Comment
                yanked  =  "1.0"  # Comment
                ws.workspace  =  true  # Comment
                shared  =  "1.0"  # Comment
                multiple-locations  =  { path  =  "../multiple-locations", version  =  "1.0" }  # Comment
                multiple-versions  =  "1.0"  # Comment
                alternative-1  =  { registry  =  "alternative", version  =  "1.0" }  # Comment
                multiple-registries  =  "1.0"  # Comment
                bar  =  { path  =  "../bar", registry  =  "alternative", version  =  "1.0.0" }  # Comment
                multiple-source-types  =  { path  =  "../multiple-source-types", version  =  "1.0.0" }  # Comment

                [dependencies.alternative-2]  # Comment
                version  =  "1.0"  # Comment
                registry  =  "alternative"  # Comment

                [target.'cfg(unix)'.dependencies]
                platform-specific  =  "1.0"  # Comment

                [dev-dependencies]
                dev  =  "1.0"  # Comment

                [build-dependencies]
                build  =  "1.0"  # Comment
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "1.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                shared = "1.0"
                multiple-versions = "2.0"
                multiple-registries  =  { registry  =  "alternative", version  =  "2.0" }  # Comment
                multiple-source-types  =  "1.0"  # Comment
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "multiple-locations/Cargo.toml",
            r#"
                [package]
                name = "multiple-locations"
                version = "1.0.0"
                edition = "2015"
                authors = []
            "#,
        )
        .file("multiple-locations/src/lib.rs", "")
        .file(
            "multiple-source-types/Cargo.toml",
            r#"
                [package]
                name = "multiple-source-types"
                version = "1.0.0"
                edition = "2015"
                authors = []
            "#,
        )
        .file("multiple-source-types/src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    Package::new("compatible", "1.0.1").publish();
    Package::new("incompatible", "1.0.1").publish();
    Package::new("pinned", "1.0.1").publish();
    Package::new("less-than", "1.0.1").publish();
    Package::new("renamed-from", "1.0.1").publish();
    Package::new("ws", "1.0.1").publish();
    Package::new("multiple-locations", "1.0.1").publish();
    Package::new("multiple-versions", "1.0.1").publish();
    Package::new("multiple-versions", "2.0.1").publish();
    Package::new("alternative-1", "1.0.1")
        .alternative(true)
        .publish();
    Package::new("alternative-2", "1.0.1")
        .alternative(true)
        .publish();
    Package::new("platform-specific", "1.0.1").publish();
    Package::new("dev", "1.0.1").publish();
    Package::new("build", "1.0.1").publish();

    Package::new("incompatible", "2.0.0").publish();
    Package::new("pinned", "2.0.0").publish();
    Package::new("less-than", "2.0.0").publish();
    Package::new("renamed-from", "2.0.0").publish();
    Package::new("pre-release", "2.0.0-alpha").publish();
    Package::new("yanked", "2.0.0").yanked(true).publish();
    Package::new("ws", "2.0.0").publish();
    Package::new("shared", "2.0.0").publish();
    Package::new("multiple-locations", "2.0.0").publish();
    Package::new("multiple-versions", "3.0.0").publish();
    Package::new("alternative-1", "2.0.0")
        .alternative(true)
        .publish();
    Package::new("alternative-2", "2.0.0")
        .alternative(true)
        .publish();
    Package::new("bar", "2.0.0").alternative(true).publish();
    Package::new("multiple-registries", "2.0.0").publish();
    Package::new("multiple-registries", "3.0.0")
        .alternative(true)
        .publish();
    Package::new("multiple-source-types", "2.0.0").publish();
    Package::new("platform-specific", "2.0.0").publish();
    Package::new("dev", "2.0.0").publish();
    Package::new("build", "2.0.0").publish();

    p.cargo("update -Zunstable-options --breaking")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[UPGRADING] multiple-registries ^2.0 -> ^3.0
[UPDATING] `dummy-registry` index
[UPGRADING] multiple-source-types ^1.0 -> ^2.0
[UPGRADING] multiple-versions ^2.0 -> ^3.0
[UPGRADING] shared ^1.0 -> ^2.0
[UPGRADING] alternative-1 ^1.0 -> ^2.0
[UPGRADING] alternative-2 ^1.0 -> ^2.0
[UPGRADING] incompatible ^1.0 -> ^2.0
[UPGRADING] multiple-registries ^1.0 -> ^2.0
[UPGRADING] multiple-versions ^1.0 -> ^3.0
[UPGRADING] ws ^1.0 -> ^2.0
[UPGRADING] dev ^1.0 -> ^2.0
[UPGRADING] build ^1.0 -> ^2.0
[UPGRADING] platform-specific ^1.0 -> ^2.0
[LOCKING] 12 packages to latest compatible versions
[UPDATING] alternative-1 v1.0.0 (registry `alternative`) -> v2.0.0
[UPDATING] alternative-2 v1.0.0 (registry `alternative`) -> v2.0.0
[UPDATING] build v1.0.0 -> v2.0.0
[UPDATING] dev v1.0.0 -> v2.0.0
[UPDATING] incompatible v1.0.0 -> v2.0.0
[UPDATING] multiple-registries v2.0.0 (registry `alternative`) -> v3.0.0
[UPDATING] multiple-registries v1.0.0 -> v2.0.0
[UPDATING] multiple-source-types v1.0.0 -> v2.0.0
[ADDING] multiple-versions v3.0.0
[UPDATING] platform-specific v1.0.0 -> v2.0.0
[UPDATING] shared v1.0.0 -> v2.0.0
[UPDATING] ws v1.0.0 -> v2.0.0

"#]])
        .run();

    let root_manifest = p.read_file("Cargo.toml");
    assert_e2e().eq(
        &root_manifest,
        str![[r#"

                # Check if formatting is preserved

                [workspace]
                members  =  ["foo", "bar"]

                [workspace.dependencies]
                ws  =  "2.0"  # This line gets partially rewritten
            "#]],
    );

    let foo_manifest = p.read_file("foo/Cargo.toml");

    assert_e2e().eq(
        &foo_manifest,
        str![[r#"

                # Check if formatting is preserved

                [package]
                name  =  "foo"
                version  =  "0.0.1"
                edition  =  "2015"
                authors  =  []

                [dependencies]
                compatible  =  "1.0"  # Comment
                incompatible  =  "2.0"  # Comment
                pinned  =  "=1.0"  # Comment
                less-than  =  "<99.0"  # Comment
                renamed-to  =  { package  =  "renamed-from", version  =  "1.0" }  # Comment
                pre-release  =  "1.0"  # Comment
                yanked  =  "1.0"  # Comment
                ws.workspace  =  true  # Comment
                shared  =  "2.0"  # Comment
                multiple-locations  =  { path  =  "../multiple-locations", version  =  "1.0" }  # Comment
                multiple-versions  =  "3.0"  # Comment
                alternative-1  =  { registry  =  "alternative", version  =  "2.0" }  # Comment
                multiple-registries  =  "2.0"  # Comment
                bar  =  { path  =  "../bar", registry  =  "alternative", version  =  "1.0.0" }  # Comment
                multiple-source-types  =  { path  =  "../multiple-source-types", version  =  "1.0.0" }  # Comment

                [dependencies.alternative-2]  # Comment
                version  =  "2.0"  # Comment
                registry  =  "alternative"  # Comment

                [target.'cfg(unix)'.dependencies]
                platform-specific  =  "2.0"  # Comment

                [dev-dependencies]
                dev  =  "2.0"  # Comment

                [build-dependencies]
                build  =  "2.0"  # Comment
            "#]],
    );

    let bar_manifest = p.read_file("bar/Cargo.toml");

    assert_e2e().eq(
        &bar_manifest,
        str![[r#"

                [package]
                name = "bar"
                version = "1.0.0"
                edition = "2015"
                authors = []

                [dependencies]
                shared = "2.0"
                multiple-versions = "3.0"
                multiple-registries  =  { registry  =  "alternative", version  =  "3.0" }  # Comment
                multiple-source-types  =  "2.0"  # Comment
            "#]],
    );

    p.cargo("update")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[UPDATING] `dummy-registry` index
[LOCKING] 4 packages to latest compatible versions
[UPDATING] compatible v1.0.0 -> v1.0.1
[UPDATING] less-than v1.0.0 -> v2.0.0
[UPDATING] pinned v1.0.0 -> v1.0.1 (available: v2.0.0)
[UPDATING] renamed-from v1.0.0 -> v1.0.1 (available: v2.0.0)

"#]])
        .run();
}

#[cargo_test]
fn update_breaking_specific_packages() {
    Package::new("just-foo", "1.0.0")
        .add_dep(Dependency::new("transitive-compatible", "1.0.0").build())
        .add_dep(Dependency::new("transitive-incompatible", "1.0.0").build())
        .publish();
    Package::new("just-bar", "1.0.0").publish();
    Package::new("shared", "1.0.0").publish();
    Package::new("ws", "1.0.0").publish();
    Package::new("transitive-compatible", "1.0.0").publish();
    Package::new("transitive-incompatible", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]

                [workspace.dependencies]
                ws = "1.0"
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                just-foo = "1.0"
                shared = "1.0"
                ws.workspace = true
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                just-bar = "1.0"
                shared = "1.0"
                ws.workspace = true
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    Package::new("just-foo", "1.0.1")
        .add_dep(Dependency::new("transitive-compatible", "1.0.0").build())
        .add_dep(Dependency::new("transitive-incompatible", "1.0.0").build())
        .publish();
    Package::new("just-bar", "1.0.1").publish();
    Package::new("shared", "1.0.1").publish();
    Package::new("ws", "1.0.1").publish();
    Package::new("transitive-compatible", "1.0.1").publish();
    Package::new("transitive-incompatible", "1.0.1").publish();

    Package::new("just-foo", "2.0.0")
        // Upgrading just-foo implies accepting an update of transitive-compatible.
        .add_dep(Dependency::new("transitive-compatible", "1.0.1").build())
        // Upgrading just-foo implies accepting a major update of transitive-incompatible.
        .add_dep(Dependency::new("transitive-incompatible", "2.0.0").build())
        .publish();
    Package::new("just-bar", "2.0.0").publish();
    Package::new("shared", "2.0.0").publish();
    Package::new("ws", "2.0.0").publish();
    Package::new("transitive-incompatible", "2.0.0").publish();

    p.cargo("update -Zunstable-options --breaking just-foo shared ws")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPGRADING] shared ^1.0 -> ^2.0
[UPGRADING] ws ^1.0 -> ^2.0
[UPGRADING] just-foo ^1.0 -> ^2.0
[LOCKING] 5 packages to latest compatible versions
[UPDATING] just-foo v1.0.0 -> v2.0.0
[UPDATING] shared v1.0.0 -> v2.0.0
[UPDATING] transitive-compatible v1.0.0 -> v1.0.1
[UPDATING] transitive-incompatible v1.0.0 -> v2.0.0
[UPDATING] ws v1.0.0 -> v2.0.0

"#]])
        .run();
}

#[cargo_test]
fn update_breaking_specific_packages_that_wont_update() {
    Package::new("compatible", "1.0.0").publish();
    Package::new("renamed-from", "1.0.0").publish();
    Package::new("non-semver", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("transitive-compatible", "1.0.0").build())
        .add_dep(Dependency::new("transitive-incompatible", "1.0.0").build())
        .publish();
    Package::new("transitive-compatible", "1.0.0").publish();
    Package::new("transitive-incompatible", "1.0.0").publish();

    let crate_manifest = r#"
        # Check if formatting is preserved

        [package]
        name  =  "foo"
        version  =  "0.0.1"
        edition  =  "2015"
        authors  =  []

        [dependencies]
        compatible  =  "1.0"  # Comment
        renamed-to  =  { package  =  "renamed-from", version  =  "1.0" }  # Comment
        non-semver  =  "~1.0"  # Comment
        bar  =  "1.0"  # Comment
    "#;

    let p = project()
        .file("Cargo.toml", crate_manifest)
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    let lock_file = p.read_file("Cargo.lock");

    Package::new("compatible", "1.0.1").publish();
    Package::new("renamed-from", "1.0.1").publish();
    Package::new("non-semver", "1.0.1").publish();
    Package::new("transitive-compatible", "1.0.1").publish();
    Package::new("transitive-incompatible", "1.0.1").publish();

    Package::new("renamed-from", "2.0.0").publish();
    Package::new("non-semver", "2.0.0").publish();
    Package::new("transitive-incompatible", "2.0.0").publish();

    p.cargo("update -Zunstable-options --breaking compatible renamed-from non-semver transitive-compatible transitive-incompatible")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index

"#]])
        .run();

    let crate_manifest_after = p.read_file("Cargo.toml");
    assert_e2e().eq(&crate_manifest_after, crate_manifest);

    let lock_file_after = p.read_file("Cargo.lock");
    assert_e2e().eq(&lock_file_after, lock_file);

    p.cargo(
        "update compatible renamed-from non-semver transitive-compatible transitive-incompatible",
    )
    .with_stderr_data(str![[r#"
[UPDATING] `[..]` index
[LOCKING] 5 packages to latest compatible versions
[UPDATING] compatible v1.0.0 -> v1.0.1
[UPDATING] non-semver v1.0.0 -> v1.0.1 (available: v2.0.0)
[UPDATING] renamed-from v1.0.0 -> v1.0.1 (available: v2.0.0)
[UPDATING] transitive-compatible v1.0.0 -> v1.0.1
[UPDATING] transitive-incompatible v1.0.0 -> v1.0.1

"#]])
    .run();
}

#[cargo_test]
fn update_breaking_without_lock_file() {
    Package::new("compatible", "1.0.0").publish();
    Package::new("incompatible", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name  =  "foo"
            version  =  "0.0.1"
            edition  =  "2015"
            authors  =  []

            [dependencies]
            compatible  =  "1.0"  # Comment
            incompatible  =  "1.0"  # Comment
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("compatible", "1.0.1").publish();
    Package::new("incompatible", "1.0.1").publish();

    Package::new("incompatible", "2.0.0").publish();

    p.cargo("update -Zunstable-options --breaking")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index
[UPGRADING] incompatible ^1.0 -> ^2.0
[LOCKING] 2 packages to latest compatible versions

"#]])
        .run();
}

#[cargo_test]
fn update_breaking_spec_version() {
    Package::new("compatible", "1.0.0").publish();
    Package::new("incompatible", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name  =  "foo"
            version  =  "0.0.1"
            edition  =  "2015"
            authors  =  []

            [dependencies]
            compatible  =  "1.0"  # Comment
            incompatible  =  "1.0"  # Comment
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    Package::new("compatible", "1.0.1").publish();
    Package::new("incompatible", "1.0.1").publish();

    Package::new("incompatible", "2.0.0").publish();

    // Invalid spec
    p.cargo("update -Zunstable-options --breaking incompatible@foo")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid package ID specification: `incompatible@foo`

Caused by:
  expected a version like "1.32"

"#]])
        .run();

    // Spec version not matching our current dependencies
    p.cargo("update -Zunstable-options --breaking incompatible@2.0.0")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#""#]])
        .run();

    // Spec source not matching our current dependencies
    p.cargo("update -Zunstable-options --breaking https://alternative.com#incompatible@1.0.0")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#""#]])
        .run();

    // Accepted spec
    p.cargo("update -Zunstable-options --breaking incompatible@1.0.0")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index
[UPGRADING] incompatible ^1.0 -> ^2.0
[LOCKING] 1 package to latest compatible version
[UPDATING] incompatible v1.0.0 -> v2.0.0

"#]])
        .run();

    // Accepted spec, full format
    Package::new("incompatible", "3.0.0").publish();
    p.cargo("update -Zunstable-options --breaking https://github.com/rust-lang/crates.io-index#incompatible@2.0.0")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index
[UPGRADING] incompatible ^2.0 -> ^3.0
[LOCKING] 1 package to latest compatible version
[UPDATING] incompatible v2.0.0 -> v3.0.0

"#]])
        .run();

    // Spec matches a dependency that will not be upgraded
    p.cargo("update -Zunstable-options --breaking compatible@1.0.0")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index

"#]])
        .run();

    // Non-existing versions
    p.cargo("update -Zunstable-options --breaking incompatible@9.0.0")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#""#]])
        .run();

    p.cargo("update -Zunstable-options --breaking compatible@9.0.0")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#""#]])
        .run();
}

#[cargo_test]
fn update_breaking_spec_version_transitive() {
    Package::new("dep", "1.0.0").publish();
    Package::new("dep", "1.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name  =  "foo"
                version  =  "0.0.1"
                edition  =  "2015"
                authors  =  []

                [dependencies]
                dep  =  "1.0"
                bar = { path = "bar", version = "0.0.1" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name  =  "bar"
                version  =  "0.0.1"
                edition  =  "2015"
                authors  =  []

                [dependencies]
                dep  =  "1.1"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    Package::new("dep", "1.1.1").publish();
    Package::new("dep", "2.0.0").publish();

    // Will upgrade the direct dependency
    p.cargo("update -Zunstable-options --breaking dep@1.0")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index
[UPGRADING] dep ^1.0 -> ^2.0
[LOCKING] 1 package to latest compatible version
[ADDING] dep v2.0.0

"#]])
        .run();

    // But not the transitive one, because bar is not a workspace member
    p.cargo("update -Zunstable-options --breaking dep@1.1")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index

"#]])
        .run();

    // A non-breaking update is different, as it will update transitive dependencies
    p.cargo("update dep@1.1")
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index
[LOCKING] 1 package to latest compatible version
[UPDATING] dep v1.1.0 -> v1.1.1

"#]])
        .run();
}

#[cargo_test]
fn update_breaking_mixed_compatibility() {
    Package::new("mixed-compatibility", "1.0.0").publish();
    Package::new("mixed-compatibility", "2.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                mixed-compatibility = "1.0"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                mixed-compatibility = "2.0"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    Package::new("mixed-compatibility", "2.0.1").publish();

    p.cargo("update -Zunstable-options --breaking")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index
[UPGRADING] mixed-compatibility ^1.0 -> ^2.0
[LOCKING] 1 package to latest compatible version
[ADDING] mixed-compatibility v2.0.1

"#]])
        .run();
}

#[cargo_test]
fn update_breaking_mixed_pinning_renaming() {
    Package::new("mixed-pinned", "1.0.0").publish();
    Package::new("mixed-ws-pinned", "1.0.0").publish();
    Package::new("renamed-from", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["pinned", "unpinned", "mixed"]

                [workspace.dependencies]
                mixed-ws-pinned = "=1.0"
            "#,
        )
        .file(
            "pinned/Cargo.toml",
            r#"
                [package]
                name = "pinned"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                mixed-pinned = "=1.0"
                mixed-ws-pinned.workspace = true
                renamed-to = { package = "renamed-from", version = "1.0" }
            "#,
        )
        .file("pinned/src/lib.rs", "")
        .file(
            "unpinned/Cargo.toml",
            r#"
                [package]
                name = "unpinned"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                mixed-pinned = "1.0"
                mixed-ws-pinned = "1.0"
                renamed-from = "1.0"
            "#,
        )
        .file("unpinned/src/lib.rs", "")
        .file(
            "mixed/Cargo.toml",
            r#"
                [package]
                name = "mixed"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [target.'cfg(windows)'.dependencies]
                mixed-pinned = "1.0"

                [target.'cfg(unix)'.dependencies]
                mixed-pinned = "=1.0"
            "#,
        )
        .file("mixed/src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    Package::new("mixed-pinned", "2.0.0").publish();
    Package::new("mixed-ws-pinned", "2.0.0").publish();
    Package::new("renamed-from", "2.0.0").publish();

    p.cargo("update -Zunstable-options --breaking")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `[..]` index
[UPGRADING] mixed-pinned ^1.0 -> ^2.0
[UPGRADING] mixed-ws-pinned ^1.0 -> ^2.0
[UPGRADING] renamed-from ^1.0 -> ^2.0
[LOCKING] 3 packages to latest compatible versions
[ADDING] mixed-pinned v2.0.0
[ADDING] mixed-ws-pinned v2.0.0
[ADDING] renamed-from v2.0.0

"#]])
        .run();

    let root_manifest = p.read_file("Cargo.toml");
    assert_e2e().eq(
        &root_manifest,
        str![[r#"

                [workspace]
                members = ["pinned", "unpinned", "mixed"]

                [workspace.dependencies]
                mixed-ws-pinned = "=1.0"
            "#]],
    );

    let pinned_manifest = p.read_file("pinned/Cargo.toml");
    assert_e2e().eq(
        &pinned_manifest,
        str![[r#"

                [package]
                name = "pinned"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                mixed-pinned = "=1.0"
                mixed-ws-pinned.workspace = true
                renamed-to = { package = "renamed-from", version = "1.0" }
            "#]],
    );

    let unpinned_manifest = p.read_file("unpinned/Cargo.toml");
    assert_e2e().eq(
        &unpinned_manifest,
        str![[r#"

                [package]
                name = "unpinned"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                mixed-pinned = "2.0"
                mixed-ws-pinned = "2.0"
                renamed-from = "2.0"
            "#]],
    );

    let mixed_manifest = p.read_file("mixed/Cargo.toml");
    assert_e2e().eq(
        &mixed_manifest,
        str![[r#"

                [package]
                name = "mixed"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [target.'cfg(windows)'.dependencies]
                mixed-pinned = "2.0"

                [target.'cfg(unix)'.dependencies]
                mixed-pinned = "=1.0"
            "#]],
    );
}

#[cargo_test]
fn update_breaking_pre_release_downgrade() {
    Package::new("bar", "2.0.0-beta.21").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name  =  "foo"
        version  =  "0.0.1"
        edition  =  "2015"
        authors  =  []

        [dependencies]
        bar = "2.0.0-beta.21"
    "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    // The purpose of this test is
    // to demonstrate that `update --breaking` will not try to downgrade to the latest stable version (1.7.0),
    // but will rather keep the latest pre-release (2.0.0-beta.21).
    Package::new("bar", "1.7.0").publish();
    p.cargo("update -Zunstable-options --breaking bar")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index

"#]])
        .run();
}

#[cargo_test]
fn update_breaking_pre_release_upgrade() {
    Package::new("bar", "2.0.0-beta.21").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name  =  "foo"
        version  =  "0.0.1"
        edition  =  "2015"
        authors  =  []

        [dependencies]
        bar = "2.0.0-beta.21"
    "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    // TODO: `2.0.0-beta.21` can be upgraded to `2.0.0-beta.22`
    Package::new("bar", "2.0.0-beta.22").publish();
    p.cargo("update -Zunstable-options --breaking bar")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index

"#]])
        .run();
    // TODO: `2.0.0-beta.21` can be upgraded to `2.0.0`
    Package::new("bar", "2.0.0").publish();
    p.cargo("update -Zunstable-options --breaking bar")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index

"#]])
        .run();

    Package::new("bar", "3.0.0").publish();
    p.cargo("update -Zunstable-options --breaking bar")
        .masquerade_as_nightly_cargo(&["update-breaking"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPGRADING] bar ^2.0.0-beta.21 -> ^3.0.0
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v2.0.0-beta.21 -> v3.0.0

"#]])
        .run();
}

#[cargo_test]
fn prefixed_v_in_version() {
    Package::new("bar", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name  =  "foo"
        version  =  "0.0.1"
        edition  =  "2015"
        authors  =  []

        [dependencies]
        bar = "1.0.0"
    "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    Package::new("bar", "1.0.1").publish();
    p.cargo("update bar --precise v1.0.1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the version provided, `v1.0.1` is not a valid SemVer version

[HELP] try changing the version to `1.0.1`

Caused by:
  unexpected character 'v' while parsing major version number

"#]])
        .run();
}
