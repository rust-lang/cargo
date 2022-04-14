//! Tests for --timings.

use cargo_test_support::project;
use cargo_test_support::registry::Package;

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
        .with_stderr_unordered(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 [..]
[COMPILING] dep v0.1.0
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
      Timing report saved to [..]/foo/target/cargo-timings/cargo-timing-[..].html
",
        )
        .run();

    p.cargo("clean").run();

    p.cargo("test --timings").run();

    p.cargo("clean").run();

    p.cargo("check --timings").run();

    p.cargo("clean").run();

    p.cargo("doc --timings").run();
}

const JSON_OUTPUT: &str = r#"
{
    "reason": "timing-info",
    "package_id": "dep 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
    "target": {
        "kind": [
            "lib"
        ],
        "crate_types": [
            "lib"
        ],
        "name": "dep",
        "src_path": "[..]/dep-0.1.0/src/lib.rs",
        "edition": "2015",
        "doc": true,
        "doctest": true,
        "test": true
    },
    "mode": "build",
    "duration": "{...}",
    "rmeta_time": "{...}"
}"#;

#[cargo_test]
fn timings_works_with_config() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            ".cargo/config",
            r#"
            [build]
            timings = ['json','html']
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("build --all-targets -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_stderr_unordered(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 [..]
[COMPILING] dep v0.1.0
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
      Timing report saved to [..]/foo/target/cargo-timings/cargo-timing-[..].html
",
        )
        .with_json_contains_unordered(JSON_OUTPUT)
        .run();
}

#[cargo_test]
fn timings_option_override_the_config() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            ".cargo/config",
            r#"
            [build]
            timings = ['json','html']
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("build --all-targets --timings=json -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_stderr_unordered(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 [..]
[COMPILING] dep v0.1.0
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .with_json_contains_unordered(JSON_OUTPUT)
        .run();
}

#[cargo_test]
fn invalid_timings_config() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            ".cargo/config",
            r#"
            [build]
            timings = ['json1','html1']
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("build --all-targets")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid timings output configuration: `json1`
",
        )
        .run();
}
