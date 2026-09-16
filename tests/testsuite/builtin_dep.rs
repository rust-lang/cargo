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

thread [..] panicked at [..]
not yet implemented: SourceKind::Builtin
[NOTE] run with `RUST_BACKTRACE=1` environment variable to display a backtrace

"#]])
        .run();
}
