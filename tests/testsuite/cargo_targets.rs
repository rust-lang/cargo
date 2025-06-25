//! Tests specifically related to target handling (lib, bins, examples, tests, benches).

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn warn_unmatched_target_filters() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "0.1.0"
        edition = "2015"

        [lib]
        test = false
        bench = false
        "#,
        )
        .file("src/lib.rs", r#"fn main() {}"#)
        .build();

    p.cargo("check --tests --bins --examples --benches")
        .with_stderr_data(str![[r#"
[WARNING] target filters `bins`, `tests`, `examples`, `benches` specified, but no targets matched; this is a no-op
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn reserved_windows_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [[bin]]
            name = "con"
            path = "src/main.rs"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    if cfg!(windows) {
        p.cargo("check")
            .with_stderr_data(str![[r#"
[WARNING] binary target `con` is a reserved Windows filename, this target will not work on Windows platforms
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
            .run();
    } else {
        p.cargo("check")
            .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
            .run();
    }
}
