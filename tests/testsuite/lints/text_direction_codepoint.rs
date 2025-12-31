use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn detects_bidi_in_description() {
    // Create a manifest with a RIGHT-TO-LEFT OVERRIDE (U+202E) in the description
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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest: `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
 --> Cargo.toml:6:18
  |
6 | description = "A �test package"
  |                  ^ this invisible unicode codepoint changes text flow direction
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [HELP] if their presence wasn't intentional, you can remove them
[ERROR] encountered 1 error while running lints

"#]])
        .run();
}

#[cargo_test]
fn detects_multiple_bidi() {
    // Create a manifest with multiple BiDi codepoints
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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest: `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
 --> Cargo.toml:6:18
  |
6 | description = "A �test� package"
  |                  ^ this invisible unicode codepoint changes text flow direction
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [HELP] if their presence wasn't intentional, you can remove them
[ERROR] unicode codepoint changing visible direction of text present in manifest: `/u{202D}` (LEFT-TO-RIGHT OVERRIDE)
 --> Cargo.toml:6:23
  |
6 | description = "A �test� package"
  |                       ^ this invisible unicode codepoint changes text flow direction
  |
  = [HELP] if their presence wasn't intentional, you can remove them
[ERROR] encountered 2 errors while running lints

"#]])
        .run();
}

#[cargo_test]
fn allow_lint() {
    // Test that the lint can be allowed
    let manifest = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
description = \"A \u{202E}test package\"

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
description = \"A \u{202E}test package\"

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
[WARNING] unicode codepoint changing visible direction of text present in manifest: `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
 --> Cargo.toml:6:18
  |
6 | description = "A �test package"
  |                  ^ this invisible unicode codepoint changes text flow direction
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `warn` in `[lints]`
  = [HELP] if their presence wasn't intentional, you can remove them
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn no_bidi_clean() {
    // Test that clean manifests pass
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
fn workspace_inherited_allow() {
    // Test workspace-level lint configuration with member package
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
description = \"A \u{202E}test package\"

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
fn virtual_workspace_bidi() {
    // Test that BiDi in virtual workspace manifest is detected
    // Note: description isn't valid for [workspace], so we use a comment-like string in resolver
    // Actually, virtual workspaces have limited fields, so let's put BiDi in a metadata field
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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest: `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
 --> Cargo.toml:6:14
  |
6 | info = "test �info"
  |              ^ this invisible unicode codepoint changes text flow direction
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [HELP] if their presence wasn't intentional, you can remove them
[ERROR] encountered 1 error while running lints

"#]])
        .run();
}
