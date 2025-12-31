use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn bidi_in_description() {
    // Manifest with a RIGHT-TO-LEFT OVERRIDE (U+202E) in the description
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
description = \"A \u{202E}test package\"
";
    let p = project()
        .file("Cargo.toml", manifest)
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
fn multiple_bidi() {
    // Manifest with multiple BiDi codepoints
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
description = \"A \u{202E}test\u{202D} package\"
";
    let p = project()
        .file("Cargo.toml", manifest)
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
fn no_bidi_clean() {
    // Clean manifest without BiDi
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
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
fn workspace_member_bidi() {
    // Workspace with BiDi in member package
    let manifest = "
[workspace]
members = [\"foo\"]
";
    let foo_manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
description = \"A \u{202E}test package\"
";
    let p = project()
        .file("Cargo.toml", manifest)
        .file("foo/Cargo.toml", foo_manifest)
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn virtual_workspace_bidi() {
    // Virtual workspace with BiDi in metadata
    let manifest = "
[workspace]
members = [\"foo\"]

[workspace.metadata]
info = \"test \u{202E}info\"
";
    let foo_manifest = r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
"#;
    let p = project()
        .file("Cargo.toml", manifest)
        .file("foo/Cargo.toml", foo_manifest)
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
