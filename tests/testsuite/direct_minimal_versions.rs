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

    p.cargo("generate-lockfile -Zdirect-minimal-versions")
        .masquerade_as_nightly_cargo(&["direct-minimal-versions"])
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

    p.cargo("generate-lockfile -Zdirect-minimal-versions")
        .masquerade_as_nightly_cargo(&["direct-minimal-versions"])
        .with_status(101)
        .with_stderr(
            r#"[UPDATING] [..]
[ERROR] failed to select a version for `dep`.
    ... required by package `foo v0.0.1 ([CWD])`
versions that meet the requirements `^1.1` are: 1.1.0

all possible versions conflict with previously selected packages.

  previously selected package `dep v1.0.0`
    ... which satisfies dependency `dep = "^1.0"` of package `foo v0.0.1 ([CWD])`

failed to select a version for `dep` which could resolve this conflict
"#,
        )
        .run();
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

    p.cargo("generate-lockfile -Zdirect-minimal-versions")
        .masquerade_as_nightly_cargo(&["direct-minimal-versions"])
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

    p.cargo("generate-lockfile -Zdirect-minimal-versions")
        .masquerade_as_nightly_cargo(&["direct-minimal-versions"])
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
        "indirect minimal version cannot be present"
    );
    assert!(
        !lock.contains("2.1.0"),
        "indirect minimal version cannot be present"
    );
    assert!(
        lock.contains("2.2.0"),
        "indirect maximal version must be present"
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

    p.cargo("generate-lockfile -Zdirect-minimal-versions")
        .masquerade_as_nightly_cargo(&["direct-minimal-versions"])
        .with_status(101)
        .with_stderr(
            r#"[UPDATING] [..]
[ERROR] failed to select a version for `indirect`.
    ... required by package `direct v1.0.0`
    ... which satisfies dependency `direct = "^1.0"` of package `foo v0.0.1 ([CWD])`
versions that meet the requirements `^2.1` are: 2.2.0, 2.1.0

all possible versions conflict with previously selected packages.

  previously selected package `indirect v2.0.0`
    ... which satisfies dependency `indirect = "^2.0"` of package `foo v0.0.1 ([CWD])`

failed to select a version for `indirect` which could resolve this conflict
"#,
        )
        .run();
}
