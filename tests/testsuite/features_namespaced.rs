//! Tests for namespaced features.

use cargo_test_support::project;

#[cargo_test]
fn namespaced_invalid_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["namespaced-features"]

                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                namespaced-features = true

                [features]
                bar = ["baz"]
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `baz` which is not defined as a feature
",
        )
        .run();
}

#[cargo_test]
fn namespaced_invalid_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["namespaced-features"]

                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                namespaced-features = true

                [features]
                bar = ["crate:baz"]
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `crate:baz` which is not a known dependency
",
        )
        .run();
}

#[cargo_test]
fn namespaced_non_optional_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["namespaced-features"]

                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                namespaced-features = true

                [features]
                bar = ["crate:baz"]

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `crate:baz` which is not an optional dependency.
  Consider adding `optional = true` to the dependency
",
        )
        .run();
}

#[cargo_test]
fn namespaced_implicit_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["namespaced-features"]

                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                namespaced-features = true

                [features]
                bar = ["baz"]

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().run();
}

#[cargo_test]
fn namespaced_shadowed_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["namespaced-features"]

                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                namespaced-features = true

                [features]
                baz = []

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().with_status(101).with_stderr(
        "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `baz` includes the optional dependency of the same name, but this is left implicit in the features included by this feature.
  Consider adding `crate:baz` to this feature's requirements.
",
    )
        .run();
}

#[cargo_test]
fn namespaced_shadowed_non_optional() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["namespaced-features"]

                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                namespaced-features = true

                [features]
                baz = []

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().with_status(101).with_stderr(
        "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `baz` includes the dependency of the same name, but this is left implicit in the features included by this feature.
  Additionally, the dependency must be marked as optional to be included in the feature definition.
  Consider adding `crate:baz` to this feature's requirements and marking the dependency as `optional = true`
",
    )
        .run();
}

#[cargo_test]
fn namespaced_implicit_non_optional() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["namespaced-features"]

                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                namespaced-features = true

                [features]
                bar = ["baz"]

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().with_status(101).with_stderr(
        "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `baz` which is not defined as a feature.
  A non-optional dependency of the same name is defined; consider adding `optional = true` to its definition
",
    ).run();
}

#[cargo_test]
fn namespaced_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["namespaced-features"]

                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                namespaced-features = true

                [features]
                baz = ["crate:baz"]

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().run();
}
