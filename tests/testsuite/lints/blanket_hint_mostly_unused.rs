use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test(nightly, reason = "-Zhint-mostly-unused is unstable")]
fn named_profile_blanket() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"

[profile.dev]
hint-mostly-unused = true
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Zprofile-hint-mostly-unused -v")
        .masquerade_as_nightly_cargo(&["profile-hint-mostly-unused", "cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zhint-mostly-unused is unstable")]
fn profile_package_wildcard() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"

[profile.dev.package."*"]
hint-mostly-unused = true
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Zprofile-hint-mostly-unused -v")
        .masquerade_as_nightly_cargo(&["profile-hint-mostly-unused", "cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zhint-mostly-unused is unstable")]
fn profile_build_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"

[profile.dev.build-override]
hint-mostly-unused = true
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Zprofile-hint-mostly-unused -v")
        .masquerade_as_nightly_cargo(&["profile-hint-mostly-unused", "cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zhint-mostly-unused is unstable")]
fn workspace_profile_package_wildcard() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]

[profile.dev.package."*"]
hint-mostly-unused = true
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
"#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zprofile-hint-mostly-unused -v")
        .masquerade_as_nightly_cargo(&["profile-hint-mostly-unused", "cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
