//! Tests for the `--message-format` flag for `cargo package`.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"
                license = "MIT"
                description = "foo"
                documentation = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --list --message-format json")
        .masquerade_as_nightly_cargo(&["package --message-format"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `--message-format` flag is unstable, pass `-Z unstable-options` to enable it
See https://github.com/rust-lang/cargo/issues/15353 for more information about the `--message-format` flag.

"#]])
        .run();
}

#[cargo_test]
fn requires_list() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"
                license = "MIT"
                description = "foo"
                documentation = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --message-format json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --message-format"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the following required arguments were not provided:
  --list

Usage: cargo[EXE] package --list --message-format <FMT> -Z <FLAG>

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn human() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2015"
            license = "MIT"
            description = "foo"
            documentation = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --list --message-format human -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --message-format"])
        .with_stderr_data(str![""])
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/lib.rs

"#]])
        .run();
}

#[cargo_test]
fn single_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2015"
            license = "MIT"
            description = "foo"
            documentation = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --list --message-format json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --message-format"])
        .with_stderr_data(str![""])
        .with_stdout_data(
            str![[r#"
[
  {
    "files": {
      "Cargo.lock": {
        "kind": "generate"
      },
      "Cargo.toml": {
        "kind": "generate",
        "path": "[ROOT]/foo/Cargo.toml"
      },
      "Cargo.toml.orig": {
        "kind": "copy",
        "path": "[ROOT]/foo/Cargo.toml"
      },
      "src/lib.rs": {
        "kind": "copy",
        "path": "[ROOT]/foo/src/lib.rs"
      }
    },
    "id": "path+[ROOTURL]/foo#0.0.0"
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();

    // has existing lockfile
    p.cargo("generate-lockfile").run();
    p.cargo("package --list --message-format json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --message-format"])
        .with_stderr_data(str![""])
        .with_stdout_data(
            str![[r#"
[
  {
    "files": {
      "Cargo.lock": {
        "kind": "generate",
        "path": "[ROOT]/foo/Cargo.lock"
      },
      "Cargo.toml": {
        "kind": "generate",
        "path": "[ROOT]/foo/Cargo.toml"
      },
      "Cargo.toml.orig": {
        "kind": "copy",
        "path": "[ROOT]/foo/Cargo.toml"
      },
      "src/lib.rs": {
        "kind": "copy",
        "path": "[ROOT]/foo/src/lib.rs"
      }
    },
    "id": "path+[ROOTURL]/foo#0.0.0"
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}

#[cargo_test]
fn workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["gondor", "rohan"]
            "#,
        )
        .file(
            "gondor/Cargo.toml",
            r#"
                [package]
                name = "gondor"
                edition = "2015"
                license = "MIT"
                description = "foo"
                documentation = "foo"
            "#,
        )
        .file("gondor/src/lib.rs", "")
        .file(
            "rohan/Cargo.toml",
            r#"
                [package]
                name = "rohan"
                edition = "2015"
                license = "MIT"
                description = "foo"
                documentation = "foo"
            "#,
        )
        .file("rohan/src/lib.rs", "")
        .build();

    p.cargo("package --list --message-format json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --message-format"])
        .with_stderr_data(str![""])
        .with_stdout_data(
            str![[r#"
[
  {
    "files": {
      "Cargo.lock": {
        "kind": "generate"
      },
      "Cargo.toml": {
        "kind": "generate",
        "path": "[ROOT]/foo/gondor/Cargo.toml"
      },
      "Cargo.toml.orig": {
        "kind": "copy",
        "path": "[ROOT]/foo/gondor/Cargo.toml"
      },
      "src/lib.rs": {
        "kind": "copy",
        "path": "[ROOT]/foo/gondor/src/lib.rs"
      }
    },
    "id": "path+[ROOTURL]/foo/gondor#0.0.0"
  },
  {
    "files": {
      "Cargo.lock": {
        "kind": "generate"
      },
      "Cargo.toml": {
        "kind": "generate",
        "path": "[ROOT]/foo/rohan/Cargo.toml"
      },
      "Cargo.toml.orig": {
        "kind": "copy",
        "path": "[ROOT]/foo/rohan/Cargo.toml"
      },
      "src/lib.rs": {
        "kind": "copy",
        "path": "[ROOT]/foo/rohan/src/lib.rs"
      }
    },
    "id": "path+[ROOTURL]/foo/rohan#0.0.0"
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();

    // has existing lockfile
    p.cargo("generate-lockfile").run();
    p.cargo("package --list --message-format json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --message-format"])
        .with_stderr_data(str![""])
        .with_stdout_data(
            str![[r#"
[
  {
    "files": {
      "Cargo.lock": {
        "kind": "generate",
        "path": "[ROOT]/foo/Cargo.lock"
      },
      "Cargo.toml": {
        "kind": "generate",
        "path": "[ROOT]/foo/gondor/Cargo.toml"
      },
      "Cargo.toml.orig": {
        "kind": "copy",
        "path": "[ROOT]/foo/gondor/Cargo.toml"
      },
      "src/lib.rs": {
        "kind": "copy",
        "path": "[ROOT]/foo/gondor/src/lib.rs"
      }
    },
    "id": "path+[ROOTURL]/foo/gondor#0.0.0"
  },
  {
    "files": {
      "Cargo.lock": {
        "kind": "generate",
        "path": "[ROOT]/foo/Cargo.lock"
      },
      "Cargo.toml": {
        "kind": "generate",
        "path": "[ROOT]/foo/rohan/Cargo.toml"
      },
      "Cargo.toml.orig": {
        "kind": "copy",
        "path": "[ROOT]/foo/rohan/Cargo.toml"
      },
      "src/lib.rs": {
        "kind": "copy",
        "path": "[ROOT]/foo/rohan/src/lib.rs"
      }
    },
    "id": "path+[ROOTURL]/foo/rohan#0.0.0"
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}
