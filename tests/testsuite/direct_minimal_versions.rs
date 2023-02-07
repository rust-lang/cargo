//! Tests for minimal-version resolution.
//!
//! Note: Some tests are located in the resolver-tests package.

use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn simple() {
    Package::new("dep", "1.0.0").publish();
    Package::new("dep", "1.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"

                [dependencies]
                dep = "1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile -Zminimal-versions")
        .masquerade_as_nightly_cargo(&["minimal-versions"])
        .run();

    let lock = p.read_lockfile();

    assert!(
        lock.contains("1.0.0"),
        "dep minimal version must be present"
    );
    assert!(
        !lock.contains("1.1.0"),
        "dep maximimal version cannot be present"
    );
}

#[cargo_test]
fn mixed_dependencies() {
    Package::new("dep", "1.0.0").publish();
    Package::new("dep", "1.1.0").publish();
    Package::new("dep", "1.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"

                [dependencies]
                dep = "1.0"

                [dev-dependencies]
                dep = "1.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile -Zminimal-versions")
        .masquerade_as_nightly_cargo(&["minimal-versions"])
        .run();

    let lock = p.read_lockfile();

    assert!(
        !lock.contains("1.0.0"),
        "dep incompatible version cannot be present"
    );
    assert!(
        lock.contains("1.1.0"),
        "dep minimal version must be present"
    );
    assert!(
        !lock.contains("1.2.0"),
        "dep maximimal version cannot be present"
    );
}

#[cargo_test]
fn yanked() {
    Package::new("dep", "1.0.0").yanked(true).publish();
    Package::new("dep", "1.1.0").publish();
    Package::new("dep", "1.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"

                [dependencies]
                dep = "1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile -Zminimal-versions")
        .masquerade_as_nightly_cargo(&["minimal-versions"])
        .run();

    let lock = p.read_lockfile();

    assert!(
        lock.contains("1.1.0"),
        "dep minimal version must be present"
    );
    assert!(
        !lock.contains("1.0.0"),
        "yanked minimal version must be skipped"
    );
    assert!(
        !lock.contains("1.2.0"),
        "dep maximimal version cannot be present"
    );
}

#[cargo_test]
fn indirect() {
    Package::new("indirect", "2.0.0").publish();
    Package::new("indirect", "2.1.0").publish();
    Package::new("indirect", "2.2.0").publish();
    Package::new("direct", "1.0.0")
        .dep("indirect", "2.1")
        .publish();
    Package::new("direct", "1.1.0")
        .dep("indirect", "2.1")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"

                [dependencies]
                direct = "1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile -Zminimal-versions")
        .masquerade_as_nightly_cargo(&["minimal-versions"])
        .run();

    let lock = p.read_lockfile();

    assert!(
        lock.contains("1.0.0"),
        "direct minimal version must be present"
    );
    assert!(
        !lock.contains("1.1.0"),
        "direct maximimal version cannot be present"
    );
    assert!(
        !lock.contains("2.0.0"),
        "indirect unmatched version cannot be present"
    );
    assert!(
        lock.contains("2.1.0"),
        "indirect minimal version must be present"
    );
    assert!(
        !lock.contains("2.2.0"),
        "indirect maximimal version cannot be present"
    );
}

#[cargo_test]
fn indirect_conflict() {
    Package::new("indirect", "2.0.0").publish();
    Package::new("indirect", "2.1.0").publish();
    Package::new("indirect", "2.2.0").publish();
    Package::new("direct", "1.0.0")
        .dep("indirect", "2.1")
        .publish();
    Package::new("direct", "1.1.0")
        .dep("indirect", "2.1")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"

                [dependencies]
                direct = "1.0"
                indirect = "2.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile -Zminimal-versions")
        .masquerade_as_nightly_cargo(&["minimal-versions"])
        .with_stderr(
            "\
[UPDATING] [..]
",
        )
        .run();

    let lock = p.read_lockfile();

    assert!(
        lock.contains("1.0.0"),
        "direct minimal version must be present"
    );
    assert!(
        !lock.contains("1.1.0"),
        "direct maximimal version cannot be present"
    );
    assert!(
        !lock.contains("2.0.0"),
        "indirect unmatched version cannot be present"
    );
    assert!(
        lock.contains("2.1.0"),
        "indirect minimal version must be present"
    );
    assert!(
        !lock.contains("2.2.0"),
        "indirect maximimal version cannot be present"
    );
}
