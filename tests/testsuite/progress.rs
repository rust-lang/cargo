//! Tests for progress bar.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

#[cargo_test]
fn bad_progress_config_unknown_when() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [term]
            progress = { when = 'unknown' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: could not load config key `term.progress.when`

Caused by:
  unknown variant `unknown`, expected one of `auto`, `never`, `always`

"#]])
        .run();
}

#[cargo_test]
fn bad_progress_config_missing_width() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [term]
            progress = { when = 'always' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] "always" progress requires a `width` key

"#]])
        .run();
}

#[cargo_test]
fn default_progress_is_auto() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [term]
            progress = { width = 1000 }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn always_shows_progress() {
    const N: usize = 3;
    let mut deps = String::new();
    for i in 1..=N {
        Package::new(&format!("dep{}", i), "1.0.0").publish();
        deps.push_str(&format!("dep{} = \"1.0\"\n", i));
    }

    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [term]
            progress = { when = 'always', width = 100 }
            "#,
        )
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                {}
                "#,
                deps
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[DOWNLOADING] [..] crate [..]
[DOWNLOADED] 3 crates ([..]) in [..]s
[BUILDING] [..] [..]/4: [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn never_progress() {
    const N: usize = 3;
    let mut deps = String::new();
    for i in 1..=N {
        Package::new(&format!("dep{}", i), "1.0.0").publish();
        deps.push_str(&format!("dep{} = \"1.0\"\n", i));
    }

    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [term]
            progress = { when = 'never' }
            "#,
        )
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                {}
                "#,
                deps
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_does_not_contain("[DOWNLOADING] [..] crates [..]")
        .with_stderr_does_not_contain("[..][DOWNLOADED] 3 crates ([..]) in [..]")
        .with_stderr_does_not_contain("[BUILDING] [..] [..]/4: [..]")
        .run();
}

#[cargo_test]
fn plain_string_when_doesnt_work() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [term]
            progress = "never"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid configuration for key `term.progress`
expected a table, but found a string for `term.progress` in [ROOT]/foo/.cargo/config.toml

"#]])
        .run();
}
