//! Tests for --timings.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

#[cargo_test]
fn timings_works() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            dep = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("build --all-targets --timings")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[COMPILING] dep v0.1.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..].html
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean").run();

    p.cargo("test --timings").run();

    p.cargo("clean").run();

    p.cargo("check --timings").run();

    p.cargo("clean").run();

    p.cargo("doc --timings").run();
}
