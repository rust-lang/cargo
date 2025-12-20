//! Tests for the `cargo locate-project` command.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn simple() {
    let p = project().build();

    p.cargo("locate-project")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn message_format() {
    let p = project().build();

    p.cargo("locate-project --message-format plain")
        .with_stdout_data(str![[r#"
[ROOT]/foo/Cargo.toml

"#]])
        .run();

    p.cargo("locate-project --message-format json")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project --message-format cryptic")
        .with_stderr_data(str![[r#"
[ERROR] invalid value 'cryptic' for '--message-format <FMT>'
  [possible values: json, plain]

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "outer"
                version = "0.0.0"

                [workspace]
                members = ["inner"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "inner/Cargo.toml",
            r#"
                [package]
                name = "inner"
                version = "0.0.0"
            "#,
        )
        .file("inner/src/lib.rs", "")
        .build();

    p.cargo("locate-project")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project")
        .cwd("inner")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/inner/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project --workspace")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project --workspace")
        .cwd("inner")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_missing_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "root"
                version = "0.0.0"

                [workspace]
                members = ["missing_member"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("locate-project --workspace")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to load manifest for workspace member `[ROOT]/foo/missing_member`
referenced by workspace at `[ROOT]/foo/Cargo.toml`

Caused by:
  failed to read `[ROOT]/foo/missing_member/Cargo.toml`

Caused by:
  [NOT_FOUND]

"#]])
        .run();
}

#[cargo_test]
fn workspace_nested_with_explicit_pointer() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "root"
                version = "0.0.0"

                [workspace]
                members = ["nested"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "nested/Cargo.toml",
            r#"
                [package]
                name = "nested"
                version = "0.0.0"
                workspace = ".."
            "#,
        )
        .file("nested/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("nested")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_not_a_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["member"]
            "#,
        )
        .file(
            "member/Cargo.toml",
            r#"
                [package]
                name = "member"
                version = "0.0.0"
            "#,
        )
        .file("member/src/lib.rs", "")
        .file(
            "not-member/Cargo.toml",
            r#"
                [package]
                name = "not-member"
                version = "0.0.0"
            "#,
        )
        .file("not-member/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("not-member")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] current package believes it's in a workspace when it's not:
current:   [ROOT]/foo/not-member/Cargo.toml
workspace: [ROOT]/foo/Cargo.toml

this may be fixable by adding `not-member` to the `workspace.members` array of the manifest located at: [ROOT]/foo/Cargo.toml
Alternatively, to keep it out of the workspace, add the package to the `workspace.exclude` array, or add an empty `[workspace]` table to the package's manifest.

"#]])
        .run();

    p.cargo("locate-project --workspace")
        .cwd("not-member/src")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] current package believes it's in a workspace when it's not:
current:   [ROOT]/foo/not-member/Cargo.toml
workspace: [ROOT]/foo/Cargo.toml

this may be fixable by adding `not-member` to the `workspace.members` array of the manifest located at: [ROOT]/foo/Cargo.toml
Alternatively, to keep it out of the workspace, add the package to the `workspace.exclude` array, or add an empty `[workspace]` table to the package's manifest.

"#]])
        .run();
}

#[cargo_test]
fn workspace_pointer_to_sibling_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["outer-member"]
            "#,
        )
        .file(
            "outer-member/Cargo.toml",
            r#"
                [package]
                name = "outer-member"
                version = "0.0.0"
            "#,
        )
        .file("outer-member/src/lib.rs", "")
        .file(
            "sibling-workspace/Cargo.toml",
            r#"
                [workspace]
                members = ["../pkg"]
            "#,
        )
        .file(
            "pkg/Cargo.toml",
            r#"
                [package]
                name = "pkg"
                version = "0.0.0"
                workspace = "../sibling-workspace"
            "#,
        )
        .file("pkg/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("pkg")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/sibling-workspace/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_member_in_both_members_and_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["pkg"]
                exclude = ["pkg"]
            "#,
        )
        .file(
            "pkg/Cargo.toml",
            r#"
                [package]
                name = "pkg"
                version = "0.0.0"
            "#,
        )
        .file("pkg/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("pkg")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_default_members_not_in_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = []
                default-members = ["pkg"]
            "#,
        )
        .file(
            "pkg/Cargo.toml",
            r#"
                [package]
                name = "pkg"
                version = "0.0.0"
            "#,
        )
        .file("pkg/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `[ROOT]/foo/pkg` is listed in default-members but is not a member
for workspace at `[ROOT]/foo/Cargo.toml`.

"#]])
        .run();
}

#[cargo_test]
fn workspace_default_members_and_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["pkg", "other"]
                default-members = ["pkg"]
                exclude = ["pkg"]
            "#,
        )
        .file(
            "pkg/Cargo.toml",
            r#"
                [package]
                name = "pkg"
                version = "0.0.0"
            "#,
        )
        .file("pkg/src/lib.rs", "")
        .file(
            "other/Cargo.toml",
            r#"
                [package]
                name = "other"
                version = "0.0.0"
            "#,
        )
        .file("other/src/lib.rs", "")
        .build();

    // pkg is in members, default-members, and exclude.
    // Since it's in members, it's still a workspace member (member wins over exclude).
    p.cargo("locate-project --workspace")
        .cwd("pkg")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_member_with_own_workspace_invalid_default_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["pkg"]
            "#,
        )
        .file(
            "pkg/Cargo.toml",
            r#"
                [package]
                name = "pkg"
                version = "0.0.0"

                [workspace]
                default-members = ["nonexistent"]
            "#,
        )
        .file("pkg/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("pkg")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `[ROOT]/foo/pkg/nonexistent` is listed in default-members but is not a member
for workspace at `[ROOT]/foo/pkg/Cargo.toml`.

"#]])
        .run();
}

#[cargo_test]
fn workspace_default_member_and_exclude_but_not_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["pkg-a"]
                default-members = ["pkg-b"]
                exclude = ["pkg-b"]
            "#,
        )
        .file(
            "pkg-a/Cargo.toml",
            r#"
                [package]
                name = "pkg-a"
                version = "0.0.0"
            "#,
        )
        .file("pkg-a/src/lib.rs", "")
        .file(
            "pkg-b/Cargo.toml",
            r#"
                [package]
                name = "pkg-b"
                version = "0.0.0"
            "#,
        )
        .file("pkg-b/src/lib.rs", "")
        .build();

    // Should error because pkg-b is in default-members but not in members
    // The exclude doesn't help since it's not in members either
    p.cargo("locate-project --workspace")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `[ROOT]/foo/pkg-b` is listed in default-members but is not a member
for workspace at `[ROOT]/foo/Cargo.toml`.

"#]])
        .run();
}

#[cargo_test]
fn workspace_only_in_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["other"]
                exclude = ["pkg"]
            "#,
        )
        .file(
            "other/Cargo.toml",
            r#"
                [package]
                name = "other"
                version = "0.0.0"
            "#,
        )
        .file("other/src/lib.rs", "")
        .file(
            "pkg/Cargo.toml",
            r#"
                [package]
                name = "pkg"
                version = "0.0.0"
            "#,
        )
        .file("pkg/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("pkg")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/pkg/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_only_exclude_no_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                exclude = ["pkg"]
            "#,
        )
        .file(
            "pkg/Cargo.toml",
            r#"
                [package]
                name = "pkg"
                version = "0.0.0"
            "#,
        )
        .file("pkg/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("pkg")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/pkg/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_glob_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crates/*"]
            "#,
        )
        .file(
            "crates/foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
            "#,
        )
        .file("crates/foo/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("crates/foo")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_glob_members_parent_path() {
    let p = project()
        .file(
            "workspace/Cargo.toml",
            r#"
                [workspace]
                members = ["../crates/*"]
            "#,
        )
        .file(
            "crates/foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                workspace = "../../workspace"
            "#,
        )
        .file("crates/foo/src/lib.rs", "")
        .file(
            "crates/bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.0"
                workspace = "../../workspace"
            "#,
        )
        .file("crates/bar/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("crates/foo")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/workspace/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project --workspace")
        .cwd("crates/bar")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/workspace/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_path_dependency_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "root"
                version = "0.0.0"

                [workspace]

                [dependencies]
                path-dep = { path = "path-dep" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "path-dep/Cargo.toml",
            r#"
                [package]
                name = "path-dep"
                version = "0.0.0"
            "#,
        )
        .file("path-dep/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("path-dep")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_nested_subdirectory_not_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["crate-a"]
            "#,
        )
        .file(
            "crate-a/Cargo.toml",
            r#"
                [package]
                name = "crate-a"
                version = "0.0.0"
            "#,
        )
        .file("crate-a/src/lib.rs", "")
        .file(
            "crate-a/subcrate/Cargo.toml",
            r#"
                [package]
                name = "subcrate"
                version = "0.0.0"
            "#,
        )
        .file("crate-a/subcrate/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("crate-a/subcrate")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] current package believes it's in a workspace when it's not:
current:   [ROOT]/foo/crate-a/subcrate/Cargo.toml
workspace: [ROOT]/foo/Cargo.toml

this may be fixable by adding `crate-a/subcrate` to the `workspace.members` array of the manifest located at: [ROOT]/foo/Cargo.toml
Alternatively, to keep it out of the workspace, add the package to the `workspace.exclude` array, or add an empty `[workspace]` table to the package's manifest.

"#]])
        .run();
}

#[cargo_test]
fn nested_independent_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["member"]
            "#,
        )
        .file(
            "member/Cargo.toml",
            r#"
                [package]
                name = "member"
                version = "0.0.0"
            "#,
        )
        .file("member/src/lib.rs", "")
        .file(
            "nested-ws/Cargo.toml",
            r#"
                [package]
                name = "nested-ws"
                version = "0.0.0"

                [workspace]
            "#,
        )
        .file("nested-ws/src/main.rs", "fn main() {}")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("nested-ws/src")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/nested-ws/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}
