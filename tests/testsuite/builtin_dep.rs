use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn feature_gate_accepted() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["builtin-dependencies"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"
                "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  unknown Cargo.toml feature `builtin-dependencies`

  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html for more information.

"#]])
        .run();
}
