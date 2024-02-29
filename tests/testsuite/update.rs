//! Tests for the `cargo update` command.

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, git, project};

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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[NOTE] pass `--verbose` to see 2 unchanged dependencies behind latest
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] serde v0.1.0 -> v0.1.1
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNGRADING] serde v0.2.1 -> v0.2.0
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[ERROR] failed to select a version for the requirement `serde = \"~1.2\"`
candidate versions found which didn't match: 1.6.0
location searched: `[..]` index (which is replacing registry `crates-io`)
required by package `bar v0.0.1 ([..]/foo)`
perhaps a crate was updated and forgotten to be re-vendored?
",
        )
        .with_status(101)
        .run();

    // `1.9.0` does not exist
    p.cargo("update serde:1.2 --precise 1.9.0")
        // This terrible error message has been the same for a long time. A fix is more than welcome!
        .with_stderr(
            "\
[UPDATING] `[..]` index
[ERROR] no matching package named `serde` found
location searched: registry `crates-io`
required by package `bar v0.0.1 ([..]/foo)`
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] serde v0.0.1+first -> v0.0.1+second
",
        )
        .run();

    // This is not considered "Downgrading". Build metadata are not assumed to
    // be ordered.
    p.cargo("update serde --precise 0.0.1+first")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] serde v0.0.1+second -> v0.0.1+first
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] serde v0.2.1 -> v0.2.2
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] log v0.1.0 -> v0.1.1
[UPDATING] serde v0.2.1 -> v0.2.2
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] log v0.1.0 -> v0.1.1
[UPDATING] serde v0.2.1 -> v0.2.2
",
        )
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
        .with_stderr(
            "\
error: the argument '--precise <PRECISE>' cannot be used with '--recursive'

Usage: cargo[EXE] update --precise <PRECISE> <SPEC|--package [<SPEC>]>

For more information, try '--help'.
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNGRADING] serde v0.2.1 -> v0.2.0
",
        )
        .run();

    // Assert `cargo metadata` shows serde 0.2.0
    p.cargo("metadata")
        .with_json(
            r#"{
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
      "id": "path+file://[..]/foo#bar@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[..]/foo/Cargo.toml",
      "metadata": null,
      "publish": null,
      "name": "bar",
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
          "test": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[..]/foo/src/lib.rs"
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
      "manifest_path": "[..]/home/.cargo/registry/src/-[..]/serde-0.2.0/Cargo.toml",
      "metadata": null,
      "publish": null,
      "name": "serde",
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
          "src_path": "[..]/home/.cargo/registry/src/-[..]/serde-0.2.0/src/lib.rs",
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
        "id": "path+file://[..]/foo#bar@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#serde@0.2.0"
      }
    ],
    "root": "path+file://[..]/foo#bar@0.0.1"
  },
  "target_directory": "[..]/foo/target",
  "version": 1,
  "workspace_members": [
    "path+file://[..]/foo#bar@0.0.1"
  ],
  "workspace_default_members": [
    "path+file://[..]/foo#bar@0.0.1"
  ],
  "workspace_root": "[..]/foo",
  "metadata": null
}"#,
        )
        .run();

    p.cargo("update serde --precise 0.2.0")
        .with_stderr(
            "\
[UPDATING] `[..]` index
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] serde v0.1.0 -> v0.1.1
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest
[WARNING] not updating lockfile due to dry run
",
        )
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
        .with_stderr(
            "\
error: invalid version format for precise version `0.1`

Caused by:
  unexpected end of input while parsing minor version number
",
        )
        .run();

    p.cargo("update bar --precise 0.1.1+does-not-match")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
error: no matching package named `bar` found
location searched: registry `crates-io`
required by package `foo v0.1.0 ([ROOT]/foo)`
",
        )
        .run();

    p.cargo("update bar --precise 0.1.1")
        .with_stderr(
            "\
[UPDATING] [..] index
[UPDATING] bar v0.1.0+extra-stuff.0 -> v0.1.1+extra-stuff.1
",
        )
        .run();

    Package::new("bar", "0.1.3").publish();
    p.cargo("update bar --precise 0.1.3+foo")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
error: no matching package named `bar` found
location searched: registry `crates-io`
required by package `foo v0.1.0 ([ROOT]/foo)`
",
        )
        .run();

    p.cargo("update bar --precise 0.1.3")
        .with_stderr(
            "\
[UPDATING] [..] index
[UPDATING] bar v0.1.1+extra-stuff.1 -> v0.1.3
",
        )
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
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n",
            git_project.url(),
        ))
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
        .with_stderr(&format!(
            "\
[UPDATING] rootcrate v2.29.8 ([CWD]/rootcrate) -> v2.29.81
[UPDATING] subcrate v2.29.8 ([CWD]/subcrate) -> v2.29.81",
        ))
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
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n",
            git_project.url(),
        ))
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
        .with_stderr(&format!(
            "\
[UPDATING] crate1 v2.29.8 ([CWD]/crate1) -> v2.29.81
[UPDATING] crate2 v2.29.8 ([CWD]/crate2) -> v2.29.81",
        ))
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
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n",
            git_project.url(),
        ))
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
        .with_stderr(
            "\
[UPDATING] crate1 v2.29.8 ([CWD]/crate1) -> v2.29.81
[UPDATING] crate2 v2.29.8 ([CWD]/crate2) -> v2.29.81",
        )
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
        .with_stderr(format!("[UPDATING] git repository `{url}`"))
        .run();

    assert!(p.read_lockfile().contains(&head_id));

    p.cargo("update git --precise")
        .arg(tag_name)
        .with_stderr(format!(
            "\
[UPDATING] git repository `{url}`
[UPDATING] git v0.5.0 ([..]) -> #{}",
            &tag_commit_id[..8],
        ))
        .run();

    assert!(p.read_lockfile().contains(&tag_commit_id));
    assert!(!p.read_lockfile().contains(&head_id));

    p.cargo("update git --precise")
        .arg(short_id)
        .with_stderr(format!(
            "\
[UPDATING] git repository `{url}`
[UPDATING] git v0.5.0 ([..]) -> #{short_id}",
        ))
        .run();

    assert!(p.read_lockfile().contains(&head_id));
    assert!(!p.read_lockfile().contains(&tag_commit_id));

    // updating back to tag still requires a git fetch,
    // as the ref may change over time.
    p.cargo("update git --precise")
        .arg(tag_name)
        .with_stderr(format!(
            "\
[UPDATING] git repository `{url}`
[UPDATING] git v0.5.0 ([..]) -> #{}",
            &tag_commit_id[..8],
        ))
        .run();

    assert!(p.read_lockfile().contains(&tag_commit_id));
    assert!(!p.read_lockfile().contains(&head_id));

    // Now make a tag looks like an oid.
    // It requires a git fetch, as the oid cannot be found in preexisting git db.
    let arbitrary_tag: String = std::iter::repeat('a').take(head_id.len()).collect();
    git::tag(&git_repo, &arbitrary_tag);

    p.cargo("update git --precise")
        .arg(&arbitrary_tag)
        .with_stderr(format!(
            "\
[UPDATING] git repository `{url}`
[UPDATING] git v0.5.0 ([..]) -> #{}",
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
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.0 ([CWD])`

Caused by:
  failed to query replaced source registry `crates-io`

Caused by:
  the `--precise <yanked-version>` flag is unstable[..]
  See [..]
  See [..]
",
        )
        .run();

    p.cargo("update --precise 0.1.1 bar")
        .masquerade_as_nightly_cargo(&["--precise <yanked-version>"])
        .arg("-Zunstable-options")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[WARNING] selected package `bar@0.1.1` was yanked by the author
[NOTE] if possible, try a compatible non-yanked version
[UPDATING] bar v0.1.0 -> v0.1.1
",
        )
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
        .masquerade_as_nightly_cargo(&["--precise <yanked-version>"])
        .arg("-Zunstable-options")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[WARNING] selected package `bar@0.1.1` was yanked by the author
[NOTE] if possible, try a compatible non-yanked version
[UPDATING] bar v0.1.0 -> v0.1.1
",
        )
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
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[UPDATING] breaking v0.1.0 -> v0.1.1 (latest: v0.2.0)
[NOTE] pass `--verbose` to see 2 unchanged dependencies behind latest
[WARNING] not updating lockfile due to dry run
",
        )
        .run();

    p.cargo("update --dry-run --verbose")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[UPDATING] breaking v0.1.0 -> v0.1.1 (latest: v0.2.0)
[UNCHANGED] pre v1.0.0-alpha.0 (latest: v1.0.0-alpha.1)
[UNCHANGED] two-ver v0.1.0 (latest: v0.2.0)
[NOTE] to see how you depend on a package, run `cargo tree --invert --package <dep>@<ver>`
[WARNING] not updating lockfile due to dry run
",
        )
        .run();

    p.cargo("update").run();

    p.cargo("update --dry-run")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[NOTE] pass `--verbose` to see 3 unchanged dependencies behind latest
[WARNING] not updating lockfile due to dry run
",
        )
        .run();

    p.cargo("update --dry-run --verbose")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[UNCHANGED] breaking v0.1.1 (latest: v0.2.0)
[UNCHANGED] pre v1.0.0-alpha.0 (latest: v1.0.0-alpha.1)
[UNCHANGED] two-ver v0.1.0 (latest: v0.2.0)
[NOTE] to see how you depend on a package, run `cargo tree --invert --package <dep>@<ver>`
[WARNING] not updating lockfile due to dry run
",
        )
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest
",
        )
        .run();

    // Publish a fixed version, should not warn.
    Package::new("bar", "0.1.2").feature("feat1", &[]).publish();
    p.cargo("update")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] bar v0.1.0 -> v0.1.2
",
        )
        .run();
}
