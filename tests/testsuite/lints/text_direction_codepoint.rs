use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

const BIDI_MANIFEST: &str = "
[package]
name = \"foo\"
version = \"0.0.1\"
edition = \"2015\"
description = \"a \u{202e}description\u{202a} here\"  # this is a \u{202b}tricky\u{202c} comment
homepage = \"a \u{202e}homepage\u{202a} there\"  # this is a \u{202b}tricky\u{202c} comment
repository = \"a \u{202e}repository\u{202a} everywhere\"  # this is a \u{202b}tricky\u{202c} comment
";

#[cargo_test]
fn bidi_comments_warn() {
    let manifest = format!(
        "
{BIDI_MANIFEST}

[lints.cargo]
text_direction_codepoint_in_comment = \"warn\"
text_direction_codepoint_in_literal = \"allow\"
"
    );

    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unicode codepoint changing visible direction of text present in comment
 --> Cargo.toml:7:51
  |
7 | description = "a �description� here"  # this is a �tricky� comment
  |                                       ------------^------^--------
  |                                       |           |      |
  |                                       |           |      "/u{202c}"
  |                                       |           "/u{202b}"
  |                                       this comment contains an invisible unicode text flow control codepoint
  |
  = [NOTE] `cargo::text_direction_codepoint_in_comment` is set to `warn` in `[lints]`
[WARNING] unicode codepoint changing visible direction of text present in comment
 --> Cargo.toml:8:46
  |
8 | homepage = "a �homepage� there"  # this is a �tricky� comment
  |                                  ------------^------^--------
  |                                  |           |      |
  |                                  |           |      "/u{202c}"
  |                                  |           "/u{202b}"
  |                                  this comment contains an invisible unicode text flow control codepoint
[WARNING] unicode codepoint changing visible direction of text present in comment
 --> Cargo.toml:9:55
  |
9 | repository = "a �repository� everywhere"  # this is a �tricky� comment
  |                                           ------------^------^--------
  |                                           |           |      |
  |                                           |           |      "/u{202c}"
  |                                           |           "/u{202b}"
  |                                           this comment contains an invisible unicode text flow control codepoint
[WARNING] `foo` (manifest) generated 3 warnings
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bidi_literals_warn() {
    let manifest = format!(
        "
{BIDI_MANIFEST}

[lints.cargo]
text_direction_codepoint_in_comment = \"allow\"
text_direction_codepoint_in_literal = \"warn\"
"
    );

    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unicode codepoint changing visible direction of text present in literal
 --> Cargo.toml:7:18
  |
7 | description = "a �description� here"  # this is a �tricky� comment
  |               ---^-----------^------
  |               |  |           |
  |               |  |           "/u{202a}"
  |               |  "/u{202e}"
  |               this literal contains an invisible unicode text flow control codepoint
  |
  = [NOTE] `cargo::text_direction_codepoint_in_literal` is set to `warn` in `[lints]`
[HELP] if you want to keep them but make them visible in your source code, you can escape them
  |
7 - description = "a �description� here"  # this is a �tricky� comment
7 + description = "a /u{202E}description/u{202A} here"  # this is a �tricky� comment
  |
[WARNING] unicode codepoint changing visible direction of text present in literal
 --> Cargo.toml:8:15
  |
8 | homepage = "a �homepage� there"  # this is a �tricky� comment
  |            ---^--------^-------
  |            |  |        |
  |            |  |        "/u{202a}"
  |            |  "/u{202e}"
  |            this literal contains an invisible unicode text flow control codepoint
  |
[HELP] if you want to keep them but make them visible in your source code, you can escape them
  |
8 - homepage = "a �homepage� there"  # this is a �tricky� comment
8 + homepage = "a /u{202E}homepage/u{202A} there"  # this is a �tricky� comment
  |
[WARNING] unicode codepoint changing visible direction of text present in literal
 --> Cargo.toml:9:17
  |
9 | repository = "a �repository� everywhere"  # this is a �tricky� comment
  |              ---^----------^------------
  |              |  |          |
  |              |  |          "/u{202a}"
  |              |  "/u{202e}"
  |              this literal contains an invisible unicode text flow control codepoint
  |
[HELP] if you want to keep them but make them visible in your source code, you can escape them
  |
9 - repository = "a �repository� everywhere"  # this is a �tricky� comment
9 + repository = "a /u{202E}repository/u{202A} everywhere"  # this is a �tricky� comment
  |
[WARNING] `foo` (manifest) generated 3 warnings
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bidi_real_workspace() {
    let workspace_manifest = format!(
        "
[workspace]
members = [\"bar\"]

{BIDI_MANIFEST}

[lints.cargo]
text_direction_codepoint_in_comment = \"warn\"
text_direction_codepoint_in_literal = \"warn\"
"
    );

    let member_manifest = format!(
        "
[package]
name = \"bar\"
version = \"0.0.1\"
edition = \"2015\"
"
    );

    let p = project()
        .file("Cargo.toml", &workspace_manifest)
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &member_manifest)
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unicode codepoint changing visible direction of text present in comment
  --> Cargo.toml:10:51
   |
10 | description = "a �description� here"  # this is a �tricky� comment
   |                                       ------------^------^--------
   |                                       |           |      |
   |                                       |           |      "/u{202c}"
   |                                       |           "/u{202b}"
   |                                       this comment contains an invisible unicode text flow control codepoint
   |
   = [NOTE] `cargo::text_direction_codepoint_in_comment` is set to `warn` in `[lints]`
[WARNING] unicode codepoint changing visible direction of text present in comment
  --> Cargo.toml:11:46
   |
11 | homepage = "a �homepage� there"  # this is a �tricky� comment
   |                                  ------------^------^--------
   |                                  |           |      |
   |                                  |           |      "/u{202c}"
   |                                  |           "/u{202b}"
   |                                  this comment contains an invisible unicode text flow control codepoint
[WARNING] unicode codepoint changing visible direction of text present in comment
  --> Cargo.toml:12:55
   |
12 | repository = "a �repository� everywhere"  # this is a �tricky� comment
   |                                           ------------^------^--------
   |                                           |           |      |
   |                                           |           |      "/u{202c}"
   |                                           |           "/u{202b}"
   |                                           this comment contains an invisible unicode text flow control codepoint
[WARNING] unicode codepoint changing visible direction of text present in literal
  --> Cargo.toml:10:18
   |
10 | description = "a �description� here"  # this is a �tricky� comment
   |               ---^-----------^------
   |               |  |           |
   |               |  |           "/u{202a}"
   |               |  "/u{202e}"
   |               this literal contains an invisible unicode text flow control codepoint
   |
   = [NOTE] `cargo::text_direction_codepoint_in_literal` is set to `warn` in `[lints]`
[HELP] if you want to keep them but make them visible in your source code, you can escape them
   |
10 - description = "a �description� here"  # this is a �tricky� comment
10 + description = "a /u{202E}description/u{202A} here"  # this is a �tricky� comment
   |
[WARNING] unicode codepoint changing visible direction of text present in literal
  --> Cargo.toml:11:15
   |
11 | homepage = "a �homepage� there"  # this is a �tricky� comment
   |            ---^--------^-------
   |            |  |        |
   |            |  |        "/u{202a}"
   |            |  "/u{202e}"
   |            this literal contains an invisible unicode text flow control codepoint
   |
[HELP] if you want to keep them but make them visible in your source code, you can escape them
   |
11 - homepage = "a �homepage� there"  # this is a �tricky� comment
11 + homepage = "a /u{202E}homepage/u{202A} there"  # this is a �tricky� comment
   |
[WARNING] unicode codepoint changing visible direction of text present in literal
  --> Cargo.toml:12:17
   |
12 | repository = "a �repository� everywhere"  # this is a �tricky� comment
   |              ---^----------^------------
   |              |  |          |
   |              |  |          "/u{202a}"
   |              |  "/u{202e}"
   |              this literal contains an invisible unicode text flow control codepoint
   |
[HELP] if you want to keep them but make them visible in your source code, you can escape them
   |
12 - repository = "a �repository� everywhere"  # this is a �tricky� comment
12 + repository = "a /u{202E}repository/u{202A} everywhere"  # this is a �tricky� comment
   |
[WARNING] `foo` (manifest) generated 6 warnings
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bidi_virtual_workspace() {
    let workspace_manifest = format!(
        "
[workspace]
members = [\"bar\"]

[workspace.package]
description = \"a \u{202e}description\u{202a} here\"  # this is a \u{202b}tricky\u{202c} comment
homepage = \"a \u{202e}homepage\u{202a} there\"  # this is a \u{202b}tricky\u{202c} comment
repository = \"a \u{202e}repository\u{202a} everywhere\"  # this is a \u{202b}tricky\u{202c} comment

[workspace.lints.cargo]
text_direction_codepoint_in_comment = \"warn\"
text_direction_codepoint_in_literal = \"warn\"
"
    );

    let member_manifest = format!(
        "
[package]
name = \"bar\"
version = \"0.0.1\"
edition = \"2015\"
description.workspace = true
homepage.workspace = true
repository.workspace = true

[lints]
workspace = true
"
    );

    let p = project()
        .file("Cargo.toml", &workspace_manifest)
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &member_manifest)
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unicode codepoint changing visible direction of text present in comment
 --> Cargo.toml:6:51
  |
6 | description = "a �description� here"  # this is a �tricky� comment
  |                                       ------------^------^--------
  |                                       |           |      |
  |                                       |           |      "/u{202c}"
  |                                       |           "/u{202b}"
  |                                       this comment contains an invisible unicode text flow control codepoint
  |
  = [NOTE] `cargo::text_direction_codepoint_in_comment` is set to `warn` in `[lints]`
[WARNING] unicode codepoint changing visible direction of text present in comment
 --> Cargo.toml:7:46
  |
7 | homepage = "a �homepage� there"  # this is a �tricky� comment
  |                                  ------------^------^--------
  |                                  |           |      |
  |                                  |           |      "/u{202c}"
  |                                  |           "/u{202b}"
  |                                  this comment contains an invisible unicode text flow control codepoint
[WARNING] unicode codepoint changing visible direction of text present in comment
 --> Cargo.toml:8:55
  |
8 | repository = "a �repository� everywhere"  # this is a �tricky� comment
  |                                           ------------^------^--------
  |                                           |           |      |
  |                                           |           |      "/u{202c}"
  |                                           |           "/u{202b}"
  |                                           this comment contains an invisible unicode text flow control codepoint
[WARNING] unicode codepoint changing visible direction of text present in literal
 --> Cargo.toml:6:18
  |
6 | description = "a �description� here"  # this is a �tricky� comment
  |               ---^-----------^------
  |               |  |           |
  |               |  |           "/u{202a}"
  |               |  "/u{202e}"
  |               this literal contains an invisible unicode text flow control codepoint
  |
  = [NOTE] `cargo::text_direction_codepoint_in_literal` is set to `warn` in `[lints]`
[HELP] if you want to keep them but make them visible in your source code, you can escape them
  |
6 - description = "a �description� here"  # this is a �tricky� comment
6 + description = "a /u{202E}description/u{202A} here"  # this is a �tricky� comment
  |
[WARNING] unicode codepoint changing visible direction of text present in literal
 --> Cargo.toml:7:15
  |
7 | homepage = "a �homepage� there"  # this is a �tricky� comment
  |            ---^--------^-------
  |            |  |        |
  |            |  |        "/u{202a}"
  |            |  "/u{202e}"
  |            this literal contains an invisible unicode text flow control codepoint
  |
[HELP] if you want to keep them but make them visible in your source code, you can escape them
  |
7 - homepage = "a �homepage� there"  # this is a �tricky� comment
7 + homepage = "a /u{202E}homepage/u{202A} there"  # this is a �tricky� comment
  |
[WARNING] unicode codepoint changing visible direction of text present in literal
 --> Cargo.toml:8:17
  |
8 | repository = "a �repository� everywhere"  # this is a �tricky� comment
  |              ---^----------^------------
  |              |  |          |
  |              |  |          "/u{202a}"
  |              |  "/u{202e}"
  |              this literal contains an invisible unicode text flow control codepoint
  |
[HELP] if you want to keep them but make them visible in your source code, you can escape them
  |
8 - repository = "a �repository� everywhere"  # this is a �tricky� comment
8 + repository = "a /u{202E}repository/u{202A} everywhere"  # this is a �tricky� comment
  |
[WARNING] workspace (manifest) generated 6 warnings
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
