use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn bidi_in_description_allowed() {
    // BiDi in description is allowed (legitimate RTL language use case)
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
fn bidi_in_comment_denied() {
    // BiDi in comments is denied - can be used to hide malicious content
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
# This is a \u{202E}tricky comment
";
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:6:13
  |
6 | # This is a �tricky comment
  |             ^ `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [NOTE] these kinds of unicode codepoints change the way text flows on screen, but can cause confusion because they change the order of characters
  = [HELP] if their presence wasn't intentional, you can remove them, or use their escape sequence (e.g., /u{202E}) in double-quoted strings
[ERROR] encountered 1 error while running lints

"#]])
        .run();
}

#[cargo_test]
fn bidi_in_non_description_value_denied() {
    // BiDi in regular values (not description/metadata) is denied
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
authors = [\"Test \u{202E}Author\"]
";
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:6:18
  |
6 | authors = ["Test �Author"]
  |                  ^ `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [NOTE] these kinds of unicode codepoints change the way text flows on screen, but can cause confusion because they change the order of characters
  = [HELP] if their presence wasn't intentional, you can remove them, or use their escape sequence (e.g., /u{202E}) in double-quoted strings
[ERROR] encountered 1 error while running lints

"#]])
        .run();
}

#[cargo_test]
fn multiple_bidi_same_line() {
    // Multiple BiDi codepoints on the same line in a denied location
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
authors = [\"A \u{202E}test\u{202D} author\"]
";
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:6:15
  |
6 | authors = ["A �test� author"]
  |               ^    ^ `/u{202D}` (LEFT-TO-RIGHT OVERRIDE)
  |               |
  |               `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [NOTE] these kinds of unicode codepoints change the way text flows on screen, but can cause confusion because they change the order of characters
  = [HELP] if their presence wasn't intentional, you can remove them, or use their escape sequence (e.g., /u{202E}) in double-quoted strings
[ERROR] encountered 1 error while running lints

"#]])
        .run();
}

#[cargo_test]
fn allow_lint() {
    // Test that the lint can be allowed even for denied locations
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
authors = [\"Test \u{202E}Author\"]

[lints.cargo]
text_direction_codepoint = \"allow\"
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
fn warn_lint() {
    // Test that the lint can be set to warn
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
authors = [\"Test \u{202E}Author\"]

[lints.cargo]
text_direction_codepoint = \"warn\"
";
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:6:18
  |
6 | authors = ["Test �Author"]
  |                  ^ `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `warn` in `[lints]`
  = [NOTE] these kinds of unicode codepoints change the way text flows on screen, but can cause confusion because they change the order of characters
  = [HELP] if their presence wasn't intentional, you can remove them, or use their escape sequence (e.g., /u{202E}) in double-quoted strings
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
fn workspace_metadata_allowed() {
    // BiDi in workspace.metadata is allowed (legitimate use case)
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

#[cargo_test]
fn package_metadata_allowed() {
    // BiDi in package.metadata values is allowed
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"

[package.metadata]
info = \"test \u{202E}info\"
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
fn workspace_inherited_allow() {
    // Workspace-level lint configuration with member package
    let manifest = "
[workspace]
members = [\"foo\"]

[workspace.lints.cargo]
text_direction_codepoint = \"allow\"
";
    let foo_manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
authors = [\"Test \u{202E}Author\"]

[lints]
workspace = true
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
fn bidi_in_key_denied() {
    // BiDi in keys (inside quoted key) is denied - security concern
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"

[package.metadata]
\"\u{202E}evil\" = \"value\"
";
    let p = project()
        .file("Cargo.toml", manifest)
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:8:2
  |
8 | "�evil" = "value"
  |  ^ `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [NOTE] these kinds of unicode codepoints change the way text flows on screen, but can cause confusion because they change the order of characters
  = [HELP] if their presence wasn't intentional, you can remove them, or use their escape sequence (e.g., /u{202E}) in double-quoted strings
[ERROR] encountered 1 error while running lints

"#]])
        .run();
}
