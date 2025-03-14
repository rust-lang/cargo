//! Tests for the `cargo package --list=json` format

use cargo_test_support::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;
use cargo_test_support::symlink_supported;

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

    p.cargo("package --list=json")
        .masquerade_as_nightly_cargo(&["package --list=json"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `--list=<FMT>` flag is unstable, pass `-Z unstable-options` to enable it
See https://github.com/rust-lang/cargo/issues/11666 for more information about the `--list=<FMT>` flag.

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

    p.cargo("package --list=json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --list=json"])
        .with_stderr_data(str![""])
        .with_stdout_data(
            str![[r#"
{
  "path+[ROOTURL]/foo#0.0.0": {
    "Cargo.lock": {
      "source": "generated"
    },
    "Cargo.toml": {
      "source": "generated"
    },
    "Cargo.toml.orig": {
      "original_path": "[ROOT]/foo/Cargo.toml",
      "source": "copied"
    },
    "src/lib.rs": {
      "original_path": "[ROOT]/foo/src/lib.rs",
      "source": "existing"
    }
  }
}
"#]]
            .is_json(),
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

    p.cargo("package --list=json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --list=json"])
        .with_stderr_data(str![""])
        .with_stdout_data(
            str![[r#"
{
  "path+[ROOTURL]/foo/gondor#0.0.0": {
    "Cargo.lock": {
      "source": "generated"
    },
    "Cargo.toml": {
      "source": "generated"
    },
    "Cargo.toml.orig": {
      "original_path": "[ROOT]/foo/gondor/Cargo.toml",
      "source": "copied"
    },
    "src/lib.rs": {
      "original_path": "[ROOT]/foo/gondor/src/lib.rs",
      "source": "existing"
    }
  },
  "path+[ROOTURL]/foo/rohan#0.0.0": {
    "Cargo.lock": {
      "source": "generated"
    },
    "Cargo.toml": {
      "source": "generated"
    },
    "Cargo.toml.orig": {
      "original_path": "[ROOT]/foo/rohan/Cargo.toml",
      "source": "copied"
    },
    "src/lib.rs": {
      "original_path": "[ROOT]/foo/rohan/src/lib.rs",
      "source": "existing"
    }
  }
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn show_copied_files() {
    if !symlink_supported() {
        return;
    }
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["gondor"]
            "#,
        )
        .file("lib.rs", "")
        .file("LICENSE", "")
        .file("README.md", "")
        .file(
            "gondor/Cargo.toml",
            r#"
                [package]
                name = "gondor"
                edition = "2015"
                description = "foo"
                documentation = "foo"
                license-file = "../LICENSE"
            "#,
        )
        .file("gondor/main.rs", "fn main() {}")
        .symlink("lib.rs", "gondor/src/lib.rs")
        .symlink("README.md", "gondor/README.md")
        .file("original-dir/file", "")
        .symlink_dir("original-dir", "gondor/symlink-dir")
        .build();

    p.cargo("package --list=json -Zunstable-options")
        .masquerade_as_nightly_cargo(&["package --list=json"])
        .with_stderr_data(str![""])
        .with_stdout_data(
            str![[r#"
{
  "path+[ROOTURL]/foo/gondor#0.0.0": {
    "Cargo.lock": {
      "source": "generated"
    },
    "Cargo.toml": {
      "source": "generated"
    },
    "Cargo.toml.orig": {
      "original_path": "[ROOT]/foo/gondor/Cargo.toml",
      "source": "copied"
    },
    "LICENSE": {
      "original_path": "[ROOT]/foo/LICENSE",
      "source": "copied"
    },
    "README.md": {
      "original_path": "[ROOT]/foo/README.md",
      "source": "copied"
    },
    "main.rs": {
      "original_path": "[ROOT]/foo/gondor/main.rs",
      "source": "existing"
    },
    "src/lib.rs": {
      "original_path": "[ROOT]/foo/lib.rs",
      "source": "copied"
    },
    "symlink-dir/file": {
      "original_path": "[ROOT]/foo/original-dir/file",
      "source": "copied"
    }
  }
}
"#]]
            .is_json(),
        )
        .run();
}
