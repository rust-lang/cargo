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
",
        )
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
      "id": "bar 0.0.1 (path+file://[..]/foo)",
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
      "id": "serde 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
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
          "serde 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)"
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
            "pkg": "serde 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)"
          }
        ],
        "features": [],
        "id": "bar 0.0.1 (path+file://[..]/foo)"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "serde 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)"
      }
    ],
    "root": "bar 0.0.1 (path+file://[..]/foo)"
  },
  "target_directory": "[..]/foo/target",
  "version": 1,
  "workspace_members": [
    "bar 0.0.1 (path+file://[..]/foo)"
  ],
  "workspace_default_members": [
    "bar 0.0.1 (path+file://[..]/foo)"
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
[UPDATING] git repository `{}`
[UPDATING] rootcrate v2.29.8 ([CWD]/rootcrate) -> v2.29.81
[UPDATING] rustdns v0.5.0 ([..]) -> [..]
[UPDATING] subcrate v2.29.8 ([CWD]/subcrate) -> v2.29.81",
            git_project.url(),
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
[UPDATING] git repository `{}`
[UPDATING] crate1 v2.29.8 ([CWD]/crate1) -> v2.29.81
[UPDATING] crate2 v2.29.8 ([CWD]/crate2) -> v2.29.81
[UPDATING] rustdns v0.5.0 ([..]) -> [..]",
            git_project.url(),
        ))
        .run();
}
