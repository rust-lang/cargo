use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn fires_on_tree_url() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
repository = "https://github.com/rust-lang/cargo/repo/tree/main/crates/foo"

[lints.cargo]
uninherited_repository = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] `package.repository` in a workspace member should be inherited from `[workspace.package]`
 --> Cargo.toml:8:1
  |
8 | repository = "https://github.com/rust-lang/cargo/repo/tree/main/crates/foo"
  | [..]
  |
  = [NOTE] `cargo::uninherited_repository` is set to `warn` in `[lints]`
[HELP] consider moving `repository` to `[workspace.package]` and inheriting it
  |
8 - repository = "https://github.com/rust-lang/cargo/repo/tree/main/crates/foo"
8 + repository.workspace = true
  |
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fires_on_blob_url() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
repository = "https://github.com/rust-lang/cargo/repo/blob/main/README.md"

[lints.cargo]
uninherited_repository = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] `package.repository` in a workspace member should be inherited from `[workspace.package]`
 --> Cargo.toml:8:1
  |
8 | repository = "https://github.com/rust-lang/cargo/repo/blob/main/README.md"
  | [..]
  |
  = [NOTE] `cargo::uninherited_repository` is set to `warn` in `[lints]`
[HELP] consider moving `repository` to `[workspace.package]` and inheriting it
  |
8 - repository = "https://github.com/rust-lang/cargo/repo/blob/main/README.md"
8 + repository.workspace = true
  |
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fires_on_root_url() {
    // Even a technically correct root URL should be inherited in a workspace.
    // The lint encourages inheritance regardless of the URL content.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
repository = "https://github.com/rust-lang/cargo/repo"

[lints.cargo]
uninherited_repository = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] `package.repository` in a workspace member should be inherited from `[workspace.package]`
 --> Cargo.toml:8:1
  |
8 | repository = "https://github.com/rust-lang/cargo/repo"
  | [..]
  |
  = [NOTE] `cargo::uninherited_repository` is set to `warn` in `[lints]`
[HELP] consider moving `repository` to `[workspace.package]` and inheriting it
  |
8 - repository = "https://github.com/rust-lang/cargo/repo"
8 + repository.workspace = true
  |
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn no_fire_without_workspace() {
    // Single-crate project (no explicit [workspace]) lint must not fire
    // because there is no [workspace.package] to inherit from.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
repository = "https://github.com/rust-lang/cargo/repo/tree/main/crates/foo"

[lints.cargo]
uninherited_repository = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn no_fire_when_inherited() {
    // Already using workspace inheritance lint must not fire.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace.package]
repository = "https://github.com/rust-lang/cargo/repo"

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
repository.workspace = true

[lints.cargo]
uninherited_repository = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fires_in_workspace_member() {
    // A member crate with an explicit repository should trigger the lint on
    // its own manifest, not the workspace root.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/foo"]
"#,
        )
        .file(
            "crates/foo/Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
repository = "https://github.com/rust-lang/cargo/repo/tree/main/crates/foo"

[lints.cargo]
uninherited_repository = "warn"
"#,
        )
        .file("crates/foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] `package.repository` in a workspace member should be inherited from `[workspace.package]`
 --> crates/foo/Cargo.toml:6:1
  |
6 | repository = "https://github.com/rust-lang/cargo/repo/tree/main/crates/foo"
  | [..]
  |
  = [NOTE] `cargo::uninherited_repository` is set to `warn` in `[lints]`
[HELP] consider moving `repository` to `[workspace.package]` and inheriting it
  |
6 - repository = "https://github.com/rust-lang/cargo/repo/tree/main/crates/foo"
6 + repository.workspace = true
  |
[CHECKING] foo v0.0.1 ([ROOT]/foo/crates/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn suppressed_by_allow() {
    // Setting the lint to "allow" must silence it even when the field is explicit.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
repository = "https://github.com/rust-lang/cargo/repo/tree/main/crates/foo"

[lints.cargo]
uninherited_repository = "allow"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
