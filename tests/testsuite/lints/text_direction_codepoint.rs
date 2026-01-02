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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:6:18
  |
6 | description = "A �test package"
  |                  ^ `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [NOTE] these kinds of unicode codepoints change the way text flows in applications/editors that support them, but can cause confusion because they change the order of characters on the screen
  = [HELP] if their presence wasn't intentional, you can remove them
[ERROR] encountered 1 error while running lints

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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:6:18
  |
6 | description = "A �test� package"
  |                  ^    ^ `/u{202D}` (LEFT-TO-RIGHT OVERRIDE)
  |                  |
  |                  `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [NOTE] these kinds of unicode codepoints change the way text flows in applications/editors that support them, but can cause confusion because they change the order of characters on the screen
  = [HELP] if their presence wasn't intentional, you can remove them
[ERROR] encountered 1 error while running lints

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
[WARNING] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:6:18
  |
6 | description = "A �test package"
  |                  ^ `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `warn` in `[lints]`
  = [NOTE] these kinds of unicode codepoints change the way text flows in applications/editors that support them, but can cause confusion because they change the order of characters on the screen
  = [HELP] if their presence wasn't intentional, you can remove them
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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unicode codepoint changing visible direction of text present in manifest
 --> Cargo.toml:6:14
  |
6 | info = "test �info"
  |              ^ `/u{202E}` (RIGHT-TO-LEFT OVERRIDE)
  |
  = [NOTE] `cargo::text_direction_codepoint` is set to `deny` by default
  = [NOTE] these kinds of unicode codepoints change the way text flows in applications/editors that support them, but can cause confusion because they change the order of characters on the screen
  = [HELP] if their presence wasn't intentional, you can remove them
[ERROR] encountered 1 error while running lints

"#]])
        .run();
}
