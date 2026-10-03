use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn direct_repository_with_workspace_repository() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[workspace.lints.cargo]
unused_workspace_package_fields = "allow"

[workspace.package]
repository = "https://example.com/workspace"

[package]
name = "member"
version = "0.1.0"
edition = "2015"
repository = "https://example.com/workspace"

[lints.cargo]
default = { level = "allow", priority = -1 }
repository_not_inherited = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[WARNING] `package.repository` is not inherited from the workspace
  --> Cargo.toml:14:14
   |
14 | repository = "https://example.com/workspace"
   |              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::repository_not_inherited` is set to `warn` in `[lints]`
[HELP] consider using `repository.workspace = true` if the workspace repository applies
[WARNING] `member` (manifest) generated 1 warning

"#]])
        .run();
}

#[cargo_test]
fn direct_repository_in_explicit_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[package]
name = "member"
version = "0.1.0"
edition = "2015"
repository = "https://example.com/member"

[lints.cargo]
default = { level = "allow", priority = -1 }
repository_not_inherited = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[WARNING] `package.repository` is not inherited from the workspace
 --> Cargo.toml:8:14
  |
8 | repository = "https://example.com/member"
  |              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::repository_not_inherited` is set to `warn` in `[lints]`
[HELP] consider defining `workspace.package.repository` and inheriting it where appropriate
[WARNING] `member` (manifest) generated 1 warning

"#]])
        .run();
}

#[cargo_test]
fn inherited_repository() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[workspace.package]
repository = "https://example.com/workspace"

[package]
name = "member"
version = "0.1.0"
edition = "2015"
repository.workspace = true

[lints.cargo]
default = { level = "allow", priority = -1 }
repository_not_inherited = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("fetch").with_stderr_data(str![""]).run();
}

#[cargo_test]
fn implicit_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "member"
version = "0.1.0"
edition = "2015"
repository = "https://example.com/member"

[lints.cargo]
default = { level = "allow", priority = -1 }
repository_not_inherited = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("fetch").with_stderr_data(str![""]).run();
}

#[cargo_test]
fn no_package_repository() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]

[package]
name = "member"
version = "0.1.0"
edition = "2015"

[lints.cargo]
default = { level = "allow", priority = -1 }
repository_not_inherited = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("fetch").with_stderr_data(str![""]).run();
}

#[cargo_test]
fn member_of_virtual_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["member"]

[workspace.package]
repository = "https://example.com/workspace"

[workspace.lints.cargo]
unused_workspace_package_fields = "allow"
"#,
        )
        .file(
            "member/Cargo.toml",
            r#"
[package]
name = "member"
version = "0.1.0"
edition = "2015"
repository = "https://example.com/member"

[lints.cargo]
default = { level = "allow", priority = -1 }
repository_not_inherited = "warn"
"#,
        )
        .file("member/src/main.rs", "fn main() {}")
        .build();

    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[WARNING] `package.repository` is not inherited from the workspace
 --> member/Cargo.toml:6:14
  |
6 | repository = "https://example.com/member"
  |              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::repository_not_inherited` is set to `warn` in `[lints]`
[HELP] consider using `repository.workspace = true` if the workspace repository applies
[WARNING] `member` (manifest) generated 1 warning

"#]])
        .run();
}

#[cargo_test]
fn excluded_path_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
exclude = ["dep"]

[package]
name = "root"
version = "0.1.0"
edition = "2015"

[dependencies]
dep = { path = "dep" }
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
[package]
name = "dep"
version = "0.1.0"
edition = "2015"
repository = "https://example.com/dep"

[lints.cargo]
default = { level = "allow", priority = -1 }
repository_not_inherited = "warn"
"#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version

"#]])
        .run();
}
