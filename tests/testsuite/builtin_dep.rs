use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn builtin_dep_accepted() {
    let p = project()
        .file("src/lib.rs", "use core;")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                core = { builtin = true }
                "#,
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["builtin-dependencies"])
        .with_status(101)
                .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dependency (core) specified without providing a local path, Git repository, version, or workspace dependency to use

"#]])
        .run();
}
