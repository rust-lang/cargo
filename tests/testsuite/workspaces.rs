//! Tests for workspaces.

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, git, project, sleep_ms};
use std::env;
use std::fs;

#[cargo_test]
fn simple_explicit() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                workspace = ".."
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());
    assert!(!p.bin("bar").is_file());

    p.cargo("build").cwd("bar").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());

    assert!(p.root().join("Cargo.lock").is_file());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
}

#[cargo_test]
fn simple_explicit_default_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
                default-members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                workspace = ".."
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("build").run();
    assert!(p.bin("bar").is_file());
    assert!(!p.bin("foo").is_file());
}

#[cargo_test]
fn non_virtual_default_members_build_other_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = [".", "bar", "baz"]
                default-members = ["baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "[CHECKING] baz v0.1.0 ([..])\n\
             [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();

    p.cargo("check --manifest-path bar/Cargo.toml")
        .with_stderr(
            "[CHECKING] bar v0.1.0 ([..])\n\
             [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();
}

#[cargo_test]
fn non_virtual_default_members_build_root_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
                default-members = ["."]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "[CHECKING] foo v0.1.0 ([..])\n\
             [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();
}

#[cargo_test]
fn inferred_root() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());
    assert!(!p.bin("bar").is_file());

    p.cargo("build").cwd("bar").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());

    assert!(p.root().join("Cargo.lock").is_file());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
}

#[cargo_test]
fn inferred_path_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "");
    let p = p.build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());
    assert!(!p.bin("bar").is_file());

    p.cargo("build").cwd("bar").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());

    assert!(p.root().join("Cargo.lock").is_file());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
}

#[cargo_test]
fn transitive_path_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dependencies]
                baz = { path = "../baz" }
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/main.rs", "fn main() {}")
        .file("baz/src/lib.rs", "");
    let p = p.build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());
    assert!(!p.bin("bar").is_file());
    assert!(!p.bin("baz").is_file());

    p.cargo("build").cwd("bar").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());
    assert!(!p.bin("baz").is_file());

    p.cargo("build").cwd("baz").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());
    assert!(p.bin("baz").is_file());

    assert!(p.root().join("Cargo.lock").is_file());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
    assert!(!p.root().join("baz/Cargo.lock").is_file());
}

#[cargo_test]
fn parent_pointer_works() {
    let p = project()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "../bar" }

                [workspace]
            "#,
        )
        .file("foo/src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                workspace = "../foo"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "");
    let p = p.build();

    p.cargo("build").cwd("foo").run();
    p.cargo("build").cwd("bar").run();
    assert!(p.root().join("foo/Cargo.lock").is_file());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
}

#[cargo_test]
fn same_names_in_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                workspace = ".."
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: two packages named `foo` in this workspace:
- [..]Cargo.toml
- [..]Cargo.toml
",
        )
        .run();
}

#[cargo_test]
fn parent_doesnt_point_to_child() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
error: current package believes it's in a workspace when it's not:
current: [..]Cargo.toml
workspace: [..]Cargo.toml

this may be fixable [..]
[..]
",
        )
        .run();
}

#[cargo_test]
fn invalid_parent_pointer() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                workspace = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: failed to read `[..]Cargo.toml`

Caused by:
  [..]
",
        )
        .run();
}

#[cargo_test]
fn invalid_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["foo"]
            "#,
        )
        .file("src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to load manifest for workspace member `[..]/foo`

Caused by:
  failed to read `[..]foo/foo/Cargo.toml`

Caused by:
  [..]
",
        )
        .run();
}

#[cargo_test]
fn bare_workspace_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check").run();
}

#[cargo_test]
fn two_roots() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [workspace]
                members = [".."]
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: multiple workspace roots found in the same workspace:
  [..]
  [..]
",
        )
        .run();
}

#[cargo_test]
fn workspace_isnt_root() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                workspace = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .with_status(101)
        .with_stderr("error: root of a workspace inferred but wasn't a root: [..]")
        .run();
}

#[cargo_test]
fn dangling_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                workspace = "../baz"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                authors = []
                workspace = "../baz"
            "#,
        )
        .file("baz/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: package `[..]` is a member of the wrong workspace
expected: [..]
actual: [..]
",
        )
        .run();
}

#[cargo_test]
fn cycle() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                workspace = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                workspace = ".."
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "[ERROR] root of a workspace inferred but wasn't a root: [..]/foo/bar/Cargo.toml",
        )
        .run();
}

#[cargo_test]
fn share_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep1 = "0.1"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep1 = "< 0.1.5"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    Package::new("dep1", "0.1.3").publish();
    Package::new("dep1", "0.1.8").publish();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep1 v0.1.3 ([..])
[CHECKING] dep1 v0.1.3
[CHECKING] foo v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn fetch_fetches_all() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep1 = "*"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    Package::new("dep1", "0.1.3").publish();

    p.cargo("fetch")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep1 v0.1.3 ([..])
",
        )
        .run();
}

#[cargo_test]
fn lock_works_for_everyone() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep2 = "0.1"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dependencies]
                dep1 = "0.1"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    Package::new("dep1", "0.1.0").publish();
    Package::new("dep2", "0.1.0").publish();

    p.cargo("generate-lockfile")
        .with_stderr("[UPDATING] `[..]` index")
        .run();

    Package::new("dep1", "0.1.1").publish();
    Package::new("dep2", "0.1.1").publish();

    p.cargo("check")
        .with_stderr(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] dep2 v0.1.0 ([..])
[CHECKING] dep2 v0.1.0
[CHECKING] foo v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("check")
        .cwd("bar")
        .with_stderr(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] dep1 v0.1.0 ([..])
[CHECKING] dep1 v0.1.0
[CHECKING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn virtual_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();
    p.cargo("build").cwd("bar").run();
    assert!(p.root().join("Cargo.lock").is_file());
    assert!(p.bin("bar").is_file());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
}

#[cargo_test]
fn explicit_package_argument_works_with_virtual_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();
    p.cargo("build --package bar").run();
    assert!(p.root().join("Cargo.lock").is_file());
    assert!(p.bin("bar").is_file());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
}

#[cargo_test]
fn virtual_misconfigure() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();
    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
error: current package believes it's in a workspace when it's not:
current:   [CWD]/Cargo.toml
workspace: [..]Cargo.toml

this may be fixable by adding `bar` to the `workspace.members` array of the \
manifest located at: [..]
[..]
",
        )
        .run();
}

#[cargo_test]
fn virtual_build_all_implied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();
    p.cargo("check").run();
}

#[cargo_test]
fn virtual_default_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
                default-members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("baz/src/main.rs", "fn main() {}");
    let p = p.build();
    p.cargo("build").run();
    assert!(p.bin("bar").is_file());
    assert!(!p.bin("baz").is_file());
}

#[cargo_test]
fn virtual_default_member_is_not_a_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
                default-members = ["something-else"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();
    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: package `[..]something-else` is listed in workspace’s default-members \
but is not a member.
",
        )
        .run();
}

#[cargo_test]
fn virtual_default_members_build_other_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
                default-members = ["baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check --manifest-path bar/Cargo.toml")
        .with_stderr(
            "[CHECKING] bar v0.1.0 ([..])\n\
             [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();
}

#[cargo_test]
fn virtual_build_no_members() {
    let p = project().file(
        "Cargo.toml",
        r#"
            [workspace]
        "#,
    );
    let p = p.build();
    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: manifest path `[..]` contains no package: The manifest is virtual, \
and the workspace has no members.
",
        )
        .run();
}

#[cargo_test]
fn include_virtual() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [workspace]
            "#,
        );
    let p = p.build();
    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: multiple workspace roots found in the same workspace:
  [..]
  [..]
",
        )
        .run();
}

#[cargo_test]
fn members_include_path_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["p1"]

                [dependencies]
                p3 = { path = "p3" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "p1/Cargo.toml",
            r#"
                [package]
                name = "p1"
                version = "0.1.0"
                authors = []

                [dependencies]
                p2 = { path = "../p2" }
            "#,
        )
        .file("p1/src/lib.rs", "")
        .file("p2/Cargo.toml", &basic_manifest("p2", "0.1.0"))
        .file("p2/src/lib.rs", "")
        .file("p3/Cargo.toml", &basic_manifest("p3", "0.1.0"))
        .file("p3/src/lib.rs", "");
    let p = p.build();

    p.cargo("check").cwd("p1").run();
    p.cargo("check").cwd("p2").run();
    p.cargo("check").cwd("p3").run();
    p.cargo("check").run();

    assert!(p.root().join("target").is_dir());
    assert!(!p.root().join("p1/target").is_dir());
    assert!(!p.root().join("p2/target").is_dir());
    assert!(!p.root().join("p3/target").is_dir());
}

#[cargo_test]
fn new_warns_you_this_will_not_work() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
            "#,
        )
        .file("src/lib.rs", "");
    let p = p.build();

    p.cargo("new --lib bar")
        .with_stderr(
            "\
warning: compiling this new package may not work due to invalid workspace configuration

current package believes it's in a workspace when it's not:
current: [..]
workspace: [..]

this may be fixable by ensuring that this crate is depended on by the workspace \
root: [..]
[..]
[CREATED] library `bar` package
",
        )
        .run();
}

#[cargo_test]
fn new_warning_with_corrupt_ws() {
    let p = project().file("Cargo.toml", "asdf").build();
    p.cargo("new bar")
        .with_stderr(
            "\
[WARNING] compiling this new package may not work due to invalid workspace configuration

failed to parse manifest at `[..]foo/Cargo.toml`

Caused by:
  TOML parse error at line 1, column 5
    |
  1 | asdf
    |     ^
  expected `.`, `=`
     Created binary (application) `bar` package
",
        )
        .run();
}

#[cargo_test]
fn lock_doesnt_change_depending_on_crate() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ['baz']

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("baz/src/lib.rs", "");
    let p = p.build();

    Package::new("foo", "1.0.0").publish();
    Package::new("bar", "1.0.0").publish();

    p.cargo("check").run();

    let lockfile = p.read_lockfile();

    p.cargo("check").cwd("baz").run();

    let lockfile2 = p.read_lockfile();

    assert_eq!(lockfile, lockfile2);
}

#[cargo_test]
fn rebuild_please() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ['lib', 'bin']
            "#,
        )
        .file("lib/Cargo.toml", &basic_manifest("lib", "0.1.0"))
        .file(
            "lib/src/lib.rs",
            r#"
                pub fn foo() -> u32 { 0 }
            "#,
        )
        .file(
            "bin/Cargo.toml",
            r#"
                [package]
                name = "bin"
                version = "0.1.0"

                [dependencies]
                lib = { path = "../lib" }
            "#,
        )
        .file(
            "bin/src/main.rs",
            r#"
                extern crate lib;

                fn main() {
                    assert_eq!(lib::foo(), 0);
                }
            "#,
        );
    let p = p.build();

    p.cargo("run").cwd("bin").run();

    sleep_ms(1000);

    p.change_file("lib/src/lib.rs", "pub fn foo() -> u32 { 1 }");

    p.cargo("build").cwd("lib").run();

    p.cargo("run")
        .cwd("bin")
        .with_status(101)
        .with_stderr_contains("[..]assertion[..]")
        .run();
}

#[cargo_test]
fn workspace_in_git() {
    let git_project = git::new("dep1", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [workspace]
                    members = ["foo"]
                "#,
            )
            .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
            .file("foo/src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "lib"
                    version = "0.1.0"

                    [dependencies.foo]
                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() -> u32 { 0 }
            "#,
        );
    let p = p.build();

    p.cargo("check").run();
}

#[cargo_test]
fn lockfile_can_specify_nonexistent_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/main.rs", "fn main() {}")
        .file(
            "Cargo.lock",
            r#"
                [[package]]
                name = "a"
                version = "0.1.0"

                [[package]]
                name = "b"
                version = "0.1.0"
            "#,
        );

    let p = p.build();

    p.cargo("check").cwd("a").run();
}

#[cargo_test]
fn you_cannot_generate_lockfile_for_empty_workspaces() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("update")
        .with_status(101)
        .with_stderr("error: you can't generate a lockfile for an empty workspace.")
        .run();
}

#[cargo_test]
fn workspace_with_transitive_dev_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["mbrubeck@example.com"]

                [dependencies.bar]
                path = "bar"

                [workspace]
            "#,
        )
        .file("src/main.rs", r#"fn main() {}"#)
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                authors = ["mbrubeck@example.com"]

                [dev-dependencies.baz]
                path = "../baz"
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                pub fn init() {}

                #[cfg(test)]

                #[test]
                fn test() {
                    extern crate baz;
                    baz::do_stuff();
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.5.0"))
        .file("baz/src/lib.rs", r#"pub fn do_stuff() {}"#);
    let p = p.build();

    p.cargo("test -p bar").run();
}

#[cargo_test]
fn error_if_parent_cargo_toml_is_invalid() {
    let p = project()
        .file("Cargo.toml", "Totally not a TOML file")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr_contains("[ERROR] failed to parse manifest at `[..]`")
        .run();
}

#[cargo_test]
fn relative_path_for_member_works() {
    let p = project()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["../bar"]
            "#,
        )
        .file("foo/src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                workspace = "../foo"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check").cwd("foo").run();
    p.cargo("check").cwd("bar").run();
}

#[cargo_test]
fn relative_path_for_root_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]

                [dependencies]
                subproj = { path = "./subproj" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("subproj/Cargo.toml", &basic_manifest("subproj", "0.1.0"))
        .file("subproj/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check --manifest-path ./Cargo.toml").run();

    p.cargo("check --manifest-path ../Cargo.toml")
        .cwd("subproj")
        .run();
}

#[cargo_test]
fn path_dep_outside_workspace_is_not_member() {
    let p = project()
        .no_manifest()
        .file(
            "ws/Cargo.toml",
            r#"
                [package]
                name = "ws"
                version = "0.1.0"
                authors = []

                [dependencies]
                foo = { path = "../foo" }

                [workspace]
            "#,
        )
        .file("ws/src/lib.rs", "extern crate foo;")
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "");
    let p = p.build();

    p.cargo("check").cwd("ws").run();
}

#[cargo_test]
fn test_in_and_out_of_workspace() {
    let p = project()
        .no_manifest()
        .file(
            "ws/Cargo.toml",
            r#"
                [package]
                name = "ws"
                version = "0.1.0"
                authors = []

                [dependencies]
                foo = { path = "../foo" }

                [workspace]
                members = [ "../bar" ]
            "#,
        )
        .file("ws/src/lib.rs", "extern crate foo; pub fn f() { foo::f() }")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "../bar" }
            "#,
        )
        .file(
            "foo/src/lib.rs",
            "extern crate bar; pub fn f() { bar::f() }",
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                workspace = "../ws"
                name = "bar"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("bar/src/lib.rs", "pub fn f() { }");
    let p = p.build();

    p.cargo("check").cwd("ws").run();

    assert!(p.root().join("ws/Cargo.lock").is_file());
    assert!(p.root().join("ws/target").is_dir());
    assert!(!p.root().join("foo/Cargo.lock").is_file());
    assert!(!p.root().join("foo/target").is_dir());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
    assert!(!p.root().join("bar/target").is_dir());

    p.cargo("check").cwd("foo").run();
    assert!(p.root().join("foo/Cargo.lock").is_file());
    assert!(p.root().join("foo/target").is_dir());
    assert!(!p.root().join("bar/Cargo.lock").is_file());
    assert!(!p.root().join("bar/target").is_dir());
}

#[cargo_test]
fn test_path_dependency_under_member() {
    let p = project()
        .file(
            "ws/Cargo.toml",
            r#"
                [package]
                name = "ws"
                version = "0.1.0"
                authors = []

                [dependencies]
                foo = { path = "../foo" }

                [workspace]
            "#,
        )
        .file("ws/src/lib.rs", "extern crate foo; pub fn f() { foo::f() }")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                workspace = "../ws"
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "./bar" }
            "#,
        )
        .file(
            "foo/src/lib.rs",
            "extern crate bar; pub fn f() { bar::f() }",
        )
        .file("foo/bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("foo/bar/src/lib.rs", "pub fn f() { }");
    let p = p.build();

    p.cargo("check").cwd("ws").run();

    assert!(!p.root().join("foo/bar/Cargo.lock").is_file());
    assert!(!p.root().join("foo/bar/target").is_dir());

    p.cargo("check").cwd("foo/bar").run();

    assert!(!p.root().join("foo/bar/Cargo.lock").is_file());
    assert!(!p.root().join("foo/bar/target").is_dir());
}

#[cargo_test]
fn excluded_simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "ws"
                version = "0.1.0"
                authors = []

                [workspace]
                exclude = ["foo"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "");
    let p = p.build();

    p.cargo("check").run();
    assert!(p.root().join("target").is_dir());
    p.cargo("check").cwd("foo").run();
    assert!(p.root().join("foo/target").is_dir());
}

#[cargo_test]
fn exclude_members_preferred() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "ws"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["foo/bar"]
                exclude = ["foo"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "")
        .file("foo/bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("foo/bar/src/lib.rs", "");
    let p = p.build();

    p.cargo("check").run();
    assert!(p.root().join("target").is_dir());
    p.cargo("check").cwd("foo").run();
    assert!(p.root().join("foo/target").is_dir());
    p.cargo("check").cwd("foo/bar").run();
    assert!(!p.root().join("foo/bar/target").is_dir());
}

#[cargo_test]
fn exclude_but_also_depend() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "ws"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "foo/bar" }

                [workspace]
                exclude = ["foo"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "")
        .file("foo/bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("foo/bar/src/lib.rs", "");
    let p = p.build();

    p.cargo("check").run();
    assert!(p.root().join("target").is_dir());
    p.cargo("check").cwd("foo").run();
    assert!(p.root().join("foo/target").is_dir());
    p.cargo("check").cwd("foo/bar").run();
    assert!(p.root().join("foo/bar/target").is_dir());
}

#[cargo_test]
fn excluded_default_members_still_must_be_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo"]
                default-members = ["foo", "bar"]
                exclude = ["bar"]
            "#,
        )
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "")
        .file("bar/something.txt", "");
    let p = p.build();
    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: package `[..]bar` is listed in workspace’s default-members \
but is not a member.
",
        )
        .run();
}

#[cargo_test]
fn excluded_default_members_crate_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar/*"]
                default-members = ["bar/*"]
                exclude = ["bar/quux"]
            "#,
        )
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/main.rs", "fn main() {}")
        .file("bar/baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("bar/baz/src/main.rs", "fn main() {}")
        .file("bar/quux/Cargo.toml", &basic_manifest("quux", "0.1.0"))
        .file("bar/quux/src/main.rs", "fn main() {}");

    let p = p.build();
    p.cargo("build").run();

    assert!(p.root().join("target").is_dir());
    assert!(!p.bin("foo").is_file());
    assert!(p.bin("baz").is_file());
    assert!(!p.bin("quux").exists());

    p.cargo("build --workspace").run();
    assert!(p.root().join("target").is_dir());
    assert!(p.bin("foo").is_file());
    assert!(!p.bin("quux").exists());

    p.cargo("build").cwd("bar/quux").run();
    assert!(p.root().join("bar/quux/target").is_dir());
}

#[cargo_test]
fn excluded_default_members_not_crate_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar/*"]
                default-members = ["bar/*"]
                exclude = ["bar/docs"]
            "#,
        )
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/main.rs", "fn main() {}")
        .file("bar/baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("bar/baz/src/main.rs", "fn main() {}")
        .file("bar/docs/readme.txt", "This folder is not a crate!");

    let p = p.build();
    p.cargo("build").run();

    assert!(!p.bin("foo").is_file());
    assert!(p.bin("baz").is_file());
    p.cargo("build --workspace").run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn glob_syntax() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["crates/*"]
                exclude = ["crates/qux"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "crates/bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                workspace = "../.."
            "#,
        )
        .file("crates/bar/src/main.rs", "fn main() {}")
        .file(
            "crates/baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                authors = []
                workspace = "../.."
            "#,
        )
        .file("crates/baz/src/main.rs", "fn main() {}")
        .file(
            "crates/qux/Cargo.toml",
            r#"
                [package]
                name = "qux"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("crates/qux/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());
    assert!(!p.bin("bar").is_file());
    assert!(!p.bin("baz").is_file());

    p.cargo("build").cwd("crates/bar").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());

    p.cargo("build").cwd("crates/baz").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("baz").is_file());

    p.cargo("build").cwd("crates/qux").run();
    assert!(!p.bin("qux").is_file());

    assert!(p.root().join("Cargo.lock").is_file());
    assert!(!p.root().join("crates/bar/Cargo.lock").is_file());
    assert!(!p.root().join("crates/baz/Cargo.lock").is_file());
    assert!(p.root().join("crates/qux/Cargo.lock").is_file());
}

/*FIXME: This fails because of how workspace.exclude and workspace.members are working.
#[cargo_test]
fn glob_syntax_2() {
    let p = project()
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [workspace]
            members = ["crates/b*"]
            exclude = ["crates/q*"]
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("crates/bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
            workspace = "../.."
        "#)
        .file("crates/bar/src/main.rs", "fn main() {}")
        .file("crates/baz/Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.1.0"
            authors = []
            workspace = "../.."
        "#)
        .file("crates/baz/src/main.rs", "fn main() {}")
        .file("crates/qux/Cargo.toml", r#"
            [package]
            name = "qux"
            version = "0.1.0"
            authors = []
        "#)
        .file("crates/qux/src/main.rs", "fn main() {}");
    p.build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());
    assert!(!p.bin("bar").is_file());
    assert!(!p.bin("baz").is_file());

    p.cargo("build").cwd("crates/bar").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());

    p.cargo("build").cwd("crates/baz").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("baz").is_file());

    p.cargo("build").cwd("crates/qux").run();
    assert!(!p.bin("qux").is_file());

    assert!(p.root().join("Cargo.lock").is_file());
    assert!(!p.root().join("crates/bar/Cargo.lock").is_file());
    assert!(!p.root().join("crates/baz/Cargo.lock").is_file());
    assert!(p.root().join("crates/qux/Cargo.lock").is_file());
}
*/

#[cargo_test]
fn glob_syntax_invalid_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["crates/*"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("crates/bar/src/main.rs", "fn main() {}");
    let p = p.build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to load manifest for workspace member `[..]/crates/bar`

Caused by:
  failed to read `[..]foo/crates/bar/Cargo.toml`

Caused by:
  [..]
",
        )
        .run();
}

/// This is a freshness test for feature use with workspaces.
///
/// `feat_lib` is used by `caller1` and `caller2`, but with different features enabled.
/// This test ensures that alternating building `caller1`, `caller2` doesn't force
/// recompile of `feat_lib`.
///
/// Ideally, once we solve rust-lang/cargo#3620, then a single Cargo build at the top level
/// will be enough.
#[cargo_test]
fn dep_used_with_separate_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["feat_lib", "caller1", "caller2"]
            "#,
        )
        .file(
            "feat_lib/Cargo.toml",
            r#"
                [package]
                name = "feat_lib"
                version = "0.1.0"
                authors = []

                [features]
                myfeature = []
            "#,
        )
        .file("feat_lib/src/lib.rs", "")
        .file(
            "caller1/Cargo.toml",
            r#"
                [package]
                name = "caller1"
                version = "0.1.0"
                authors = []

                [dependencies]
                feat_lib = { path = "../feat_lib" }
            "#,
        )
        .file("caller1/src/main.rs", "fn main() {}")
        .file("caller1/src/lib.rs", "")
        .file(
            "caller2/Cargo.toml",
            r#"
                [package]
                name = "caller2"
                version = "0.1.0"
                authors = []

                [dependencies]
                feat_lib = { path = "../feat_lib", features = ["myfeature"] }
                caller1 = { path = "../caller1" }
            "#,
        )
        .file("caller2/src/main.rs", "fn main() {}")
        .file("caller2/src/lib.rs", "");
    let p = p.build();

    // Build the entire workspace.
    p.cargo("build --workspace")
        .with_stderr(
            "\
[..]Compiling feat_lib v0.1.0 ([..])
[..]Compiling caller1 v0.1.0 ([..])
[..]Compiling caller2 v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    assert!(p.bin("caller1").is_file());
    assert!(p.bin("caller2").is_file());

    // Build `caller1`. Should build the dep library. Because the features
    // are different than the full workspace, it rebuilds.
    // Ideally once we solve rust-lang/cargo#3620, then a single Cargo build at the top level
    // will be enough.
    p.cargo("build")
        .cwd("caller1")
        .with_stderr(
            "\
[..]Compiling feat_lib v0.1.0 ([..])
[..]Compiling caller1 v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // Alternate building `caller2`/`caller1` a few times, just to make sure
    // features are being built separately. Should not rebuild anything.
    p.cargo("build")
        .cwd("caller2")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
    p.cargo("build")
        .cwd("caller1")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
    p.cargo("build")
        .cwd("caller2")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
}

#[cargo_test]
fn dont_recurse_out_of_cargo_home() {
    let git_project = git::new("dep", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep", "0.1.0"))
            .file("src/lib.rs", "")
            .file(
                "build.rs",
                r#"
                    use std::env;
                    use std::path::Path;
                    use std::process::{self, Command};

                    fn main() {
                        let cargo = env::var_os("CARGO").unwrap();
                        let cargo_manifest_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();
                        let output = Command::new(cargo)
                            .args(&["metadata", "--format-version", "1", "--manifest-path"])
                            .arg(&Path::new(&cargo_manifest_dir).join("Cargo.toml"))
                            .output()
                            .unwrap();
                        if !output.status.success() {
                            eprintln!("{}", String::from_utf8(output.stderr).unwrap());
                            process::exit(1);
                        }
                    }
                "#,
            )
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies.dep]
                    git = "{}"

                    [workspace]
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "");
    let p = p.build();

    p.cargo("check")
        .env("CARGO_HOME", p.root().join(".cargo"))
        .run();
}

// FIXME: this fails because of how workspace.exclude and workspace.members are working.
/*
#[cargo_test]
fn include_and_exclude() {
    let p = project()
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo"]
            exclude = ["foo/bar"]
            "#)
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "")
        .file("foo/bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("foo/bar/src/lib.rs", "");
    p.build();

    p.cargo("build").cwd("foo").run();
    assert!(p.root().join("target").is_dir());
    assert!(!p.root().join("foo/target").is_dir());
    p.cargo("build").cwd("foo/bar").run();
    assert!(p.root().join("foo/bar/target").is_dir());
}
*/

#[cargo_test]
fn cargo_home_at_root_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["a"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "");
    let p = p.build();

    p.cargo("check").run();
    p.cargo("check --frozen").env("CARGO_HOME", p.root()).run();
}

#[cargo_test]
fn relative_rustc() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                use std::process::Command;
                use std::env;

                fn main() {
                    let mut cmd = Command::new("rustc");
                    for arg in env::args_os().skip(1) {
                        cmd.arg(arg);
                    }
                    std::process::exit(cmd.status().unwrap().code().unwrap());
                }
            "#,
        )
        .build();
    p.cargo("build").run();

    let src = p
        .root()
        .join("target/debug/foo")
        .with_extension(env::consts::EXE_EXTENSION);

    Package::new("a", "0.1.0").publish();

    let p = project()
        .at("lib")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "lib"
                version = "0.1.0"

                [dependencies]
                a = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    fs::copy(&src, p.root().join(src.file_name().unwrap())).unwrap();

    let file = format!("./foo{}", env::consts::EXE_SUFFIX);
    p.cargo("build").env("RUSTC", &file).run();
}

#[cargo_test]
fn ws_rustc_err() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a"]
            "#,
        )
        .file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("rustc")
        .with_status(101)
        .with_stderr("[ERROR] [..]against an actual package[..]")
        .run();

    p.cargo("rustdoc")
        .with_status(101)
        .with_stderr("[ERROR] [..]against an actual package[..]")
        .run();
}

#[cargo_test]
fn ws_err_unused() {
    for key in &[
        "[lib]",
        "[[bin]]",
        "[[example]]",
        "[[test]]",
        "[[bench]]",
        "[dependencies]",
        "[dev-dependencies]",
        "[build-dependencies]",
        "[features]",
        "[target]",
        "[badges]",
    ] {
        let p = project()
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                    [workspace]
                    members = ["a"]

                    {}
                    "#,
                    key
                ),
            )
            .file("a/Cargo.toml", &basic_lib_manifest("a"))
            .file("a/src/lib.rs", "")
            .build();
        p.cargo("check")
            .with_status(101)
            .with_stderr(&format!(
                "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  this virtual manifest specifies a {} section, which is not allowed
",
                key
            ))
            .run();
    }
}

#[cargo_test]
fn ws_warn_unused() {
    for (key, name) in &[
        ("[profile.dev]\nopt-level = 1", "profiles"),
        ("[replace]\n\"bar:0.1.0\" = { path = \"bar\" }", "replace"),
        ("[patch.crates-io]\nbar = { path = \"bar\" }", "patch"),
    ] {
        let p = project()
            .file(
                "Cargo.toml",
                r#"
                [workspace]
                members = ["a"]
                "#,
            )
            .file(
                "a/Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "a"
                    version = "0.1.0"

                    {}
                    "#,
                    key
                ),
            )
            .file("a/src/lib.rs", "")
            .build();
        p.cargo("check")
            .with_stderr_contains(&format!(
                "\
[WARNING] {} for the non root package will be ignored, specify {} at the workspace root:
package:   [..]/foo/a/Cargo.toml
workspace: [..]/foo/Cargo.toml
",
                name, name
            ))
            .run();
    }
}

#[cargo_test]
fn ws_warn_path() {
    // Warnings include path to manifest.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            cargo-features = ["edition"]
            [package]
            name = "foo"
            version = "0.1.0"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_contains("[WARNING] [..]/foo/a/Cargo.toml: the cargo feature `edition`[..]")
        .run();
}

#[cargo_test]
fn invalid_missing() {
    // Make sure errors are not suppressed with -q.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                x = { path = 'x' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -q")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to get `x` as a dependency of package `foo v0.1.0 [..]`

Caused by:
  failed to load source for dependency `x`

Caused by:
  Unable to update [..]/foo/x

Caused by:
  failed to read `[..]foo/x/Cargo.toml`

Caused by:
  [..]
",
        )
        .run();
}

#[cargo_test]
fn member_dep_missing() {
    // Make sure errors are not suppressed with -q.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -q")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to load manifest for workspace member `[..]/bar`

Caused by:
  failed to load manifest for dependency `baz`

Caused by:
  failed to read `[..]foo/bar/baz/Cargo.toml`

Caused by:
  [..]
",
        )
        .run();
}

#[cargo_test]
fn simple_primary_package_env_var() {
    let is_primary_package = r#"
        #[test]
        fn verify_primary_package() {{
            assert!(option_env!("CARGO_PRIMARY_PACKAGE").is_some());
        }}
    "#;

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/lib.rs", is_primary_package)
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
                workspace = ".."
            "#,
        )
        .file("bar/src/lib.rs", is_primary_package);
    let p = p.build();

    p.cargo("test").run();

    // Again, this time selecting a specific crate
    p.cargo("clean").run();
    p.cargo("test -p bar").run();

    // Again, this time selecting all crates
    p.cargo("clean").run();
    p.cargo("test --all").run();
}

#[cargo_test]
fn virtual_primary_package_env_var() {
    let is_primary_package = r#"
        #[test]
        fn verify_primary_package() {{
            assert!(option_env!("CARGO_PRIMARY_PACKAGE").is_some());
        }}
    "#;

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", is_primary_package)
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", is_primary_package);
    let p = p.build();

    p.cargo("test").run();

    // Again, this time selecting a specific crate
    p.cargo("clean").run();
    p.cargo("test -p foo").run();
}

#[cargo_test]
fn ensure_correct_workspace_when_nested() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]

                [package]
                name = "bar"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "sub/Cargo.toml",
            r#"
                [workspace]
                members = ["foo"]
            "#,
        )
        .file(
            "sub/foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "../.."}
            "#,
        )
        .file("sub/foo/src/main.rs", "fn main() {}");
    let p = p.build();
    p.cargo("tree")
        .cwd("sub/foo")
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo/sub/foo)
└── bar v0.1.0 ([..]/foo)\
        ",
        )
        .run();
}
