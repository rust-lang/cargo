//! Tests for namespaced features.

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::{self, Dependency, Package};
use cargo_test_support::{paths, process, project, publish, rustc_host};
use std::fs;

#[cargo_test]
fn gated() {
    // Need namespaced-features to use `dep:` syntax.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [features]
                foo = ["dep:bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  namespaced features with the `dep:` prefix are only allowed on the nightly channel \
  and requires the `-Z namespaced-features` flag on the command-line
",
        )
        .run();
}

#[cargo_test]
fn dependency_gate_ignored() {
    // Dependencies with `dep:` features are ignored in the registry if not on nightly.
    Package::new("baz", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat", &["dep:baz"])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] no matching package named `bar` found
location searched: registry `https://github.com/rust-lang/crates.io-index`
required by package `foo v0.1.0 ([..]/foo)`
",
        )
        .run();

    // Publish a version without namespaced features, it should ignore 1.0.0
    // an use this instead.
    Package::new("bar", "1.0.1")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat", &["baz"])
        .publish();
    p.cargo("check")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar [..]
[CHECKING] bar v1.0.1
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn dependency_with_crate_syntax() {
    // Registry dependency uses dep: syntax.
    Package::new("baz", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat", &["dep:baz"])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = {version="1.0", features=["feat"]}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] [..]
[DOWNLOADED] [..]
[CHECKING] baz v1.0.0
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn namespaced_invalid_feature() {
    // Specifies a feature that doesn't exist.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [features]
                bar = ["baz"]
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  feature `bar` includes `baz` which is neither a dependency nor another feature
",
        )
        .run();
}

#[cargo_test]
fn namespaced_invalid_dependency() {
    // Specifies a dep:name that doesn't exist.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [features]
                bar = ["dep:baz"]
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  feature `bar` includes `dep:baz`, but `baz` is not listed as a dependency
",
        )
        .run();
}

#[cargo_test]
fn namespaced_non_optional_dependency() {
    // Specifies a dep:name for a dependency that is not optional.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [features]
                bar = ["dep:baz"]

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  feature `bar` includes `dep:baz`, but `baz` is not an optional dependency
  A non-optional dependency of the same name is defined; consider adding `optional = true` to its definition.
",
        )
        .run();
}

#[cargo_test]
fn namespaced_implicit_feature() {
    // Backwards-compatible with old syntax.
    Package::new("baz", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [features]
                bar = ["baz"]

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[CHECKING] foo v0.0.1 [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("check -Z namespaced-features --features baz")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 [..]
[CHECKING] baz v0.1.0
[CHECKING] foo v0.0.1 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn namespaced_shadowed_dep() {
    // An optional dependency is not listed in the features table, and its
    // implicit feature is overridden.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [features]
                baz = []

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  optional dependency `baz` is not included in any feature
  Make sure that `dep:baz` is included in one of features in the [features] table.
",
        )
        .run();
}

#[cargo_test]
fn namespaced_shadowed_non_optional() {
    // Able to specify a feature with the same name as a required dependency.
    Package::new("baz", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [features]
                baz = []

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn namespaced_implicit_non_optional() {
    // Includes a non-optional dependency in [features] table.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [features]
                bar = ["baz"]

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Z namespaced-features").masquerade_as_nightly_cargo().with_status(101).with_stderr(
        "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  feature `bar` includes `baz`, but `baz` is not an optional dependency
  A non-optional dependency of the same name is defined; consider adding `optional = true` to its definition.
",
    ).run();
}

#[cargo_test]
fn namespaced_same_name() {
    // Explicitly listing an optional dependency in the [features] table.
    Package::new("baz", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [features]
                baz = ["dep:baz"]

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    if cfg!(feature="baz") { println!("baz"); }
                }
            "#,
        )
        .build();

    p.cargo("run -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[COMPILING] foo v0.0.1 [..]
[FINISHED] [..]
[RUNNING] [..]
",
        )
        .with_stdout("")
        .run();

    p.cargo("run -Z namespaced-features --features baz")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 [..]
[COMPILING] baz v0.1.0
[COMPILING] foo v0.0.1 [..]
[FINISHED] [..]
[RUNNING] [..]
",
        )
        .with_stdout("baz")
        .run();
}

#[cargo_test]
fn no_implicit_feature() {
    // Using `dep:` will not create an implicit feature.
    Package::new("regex", "1.0.0").publish();
    Package::new("lazy_static", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                regex = { version = "1.0", optional = true }
                lazy_static = { version = "1.0", optional = true }

                [features]
                regex = ["dep:regex", "dep:lazy_static"]
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    if cfg!(feature = "regex") { println!("regex"); }
                    if cfg!(feature = "lazy_static") { println!("lazy_static"); }
                }
            "#,
        )
        .build();

    p.cargo("run -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
[RUNNING] `target/debug/foo[EXE]`
",
        )
        .with_stdout("")
        .run();

    p.cargo("run -Z namespaced-features --features regex")
        .masquerade_as_nightly_cargo()
        .with_stderr_unordered(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] regex v1.0.0 [..]
[DOWNLOADED] lazy_static v1.0.0 [..]
[COMPILING] regex v1.0.0
[COMPILING] lazy_static v1.0.0
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
[RUNNING] `target/debug/foo[EXE]`
",
        )
        .with_stdout("regex")
        .run();

    p.cargo("run -Z namespaced-features --features lazy_static")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[ERROR] Package `foo v0.1.0 [..]` does not have feature `lazy_static`. \
It has an optional dependency with that name, but that dependency uses the \"dep:\" \
syntax in the features table, so it does not have an implicit feature with that name.
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn crate_feature_explicit() {
    // dep:name/feature syntax shouldn't set implicit feature.
    Package::new("bar", "1.0.0")
        .file(
            "src/lib.rs",
            r#"
                #[cfg(not(feature="feat"))]
                compile_error!{"feat missing"}
            "#,
        )
        .feature("feat", &[])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = {version = "1.0", optional=true}

                [features]
                f1 = ["dep:bar/feat"]
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[cfg(not(feature="f1"))]
                compile_error!{"f1 missing"}

                #[cfg(feature="bar")]
                compile_error!{"bar should not be set"}
            "#,
        )
        .build();

    p.cargo("check -Z namespaced-features --features f1")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 [..]
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn crate_syntax_bad_name() {
    // "dep:bar" = []
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version="1.0", optional=true }

                [features]
                "dep:bar" = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Z namespaced-features --features dep:bar")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at [..]/foo/Cargo.toml`

Caused by:
  feature named `dep:bar` is not allowed to start with `dep:`
",
        )
        .run();
}

#[cargo_test]
fn crate_syntax_in_dep() {
    // features = ["dep:baz"]
    Package::new("baz", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", features = ["dep:baz"] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] feature value `dep:baz` is not allowed to use explicit `dep:` syntax
",
        )
        .run();
}

#[cargo_test]
fn crate_syntax_cli() {
    // --features dep:bar
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", optional=true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Z namespaced-features --features dep:bar")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] feature value `dep:bar` is not allowed to use explicit `dep:` syntax
",
        )
        .run();
}

#[cargo_test]
fn crate_required_features() {
    // required-features = ["dep:bar"]
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", optional=true }

                [[bin]]
                name = "foo"
                required-features = ["dep:bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] invalid feature `dep:bar` in required-features of target `foo`: \
`dep:` prefixed feature values are not allowed in required-features
",
        )
        .run();
}

#[cargo_test]
fn json_exposed() {
    // Checks that the implicit dep: values are exposed in JSON.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", optional=true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("metadata -Z namespaced-features --no-deps")
        .masquerade_as_nightly_cargo()
        .with_json(
            r#"
                {
                  "packages": [
                    {
                      "name": "foo",
                      "version": "0.1.0",
                      "id": "foo 0.1.0 [..]",
                      "license": null,
                      "license_file": null,
                      "description": null,
                      "homepage": null,
                      "documentation": null,
                      "source": null,
                      "dependencies": "{...}",
                      "targets": "{...}",
                      "features": {
                        "bar": ["dep:bar"]
                      },
                      "manifest_path": "[..]foo/Cargo.toml",
                      "metadata": null,
                      "publish": null,
                      "authors": [],
                      "categories": [],
                      "keywords": [],
                      "readme": null,
                      "repository": null,
                      "edition": "2015",
                      "links": null
                    }
                  ],
                  "workspace_members": "{...}",
                  "resolve": null,
                  "target_directory": "[..]foo/target",
                  "version": 1,
                  "workspace_root": "[..]foo",
                  "metadata": null
                }
            "#,
        )
        .run();
}

#[cargo_test]
fn crate_feature_with_explicit() {
    // crate_name/feat_name syntax where crate_name already has a feature defined.
    // NOTE: I don't know if this is actually ideal behavior.
    Package::new("bar", "1.0.0")
        .feature("bar_feat", &[])
        .file(
            "src/lib.rs",
            r#"
                #[cfg(not(feature="bar_feat"))]
                compile_error!("bar_feat is not enabled");
            "#,
        )
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version="1.0", optional = true }

                [features]
                f1 = ["bar/bar_feat"]
                bar = ["dep:bar", "f2"]
                f2 = []
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[cfg(not(feature="bar"))]
                compile_error!("bar should be enabled");

                #[cfg(not(feature="f2"))]
                compile_error!("f2 should be enabled");
            "#,
        )
        .build();

    p.cargo("check -Z namespaced-features --features f1")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 [..]
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn optional_explicit_without_crate() {
    // "feat" syntax when there is no implicit "feat" feature because it is
    // explicitly listed elsewhere.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [features]
                feat1 = ["dep:bar"]
                feat2 = ["bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at [..]

Caused by:
  feature `feat2` includes `bar`, but `bar` is an optional dependency without an implicit feature
  Use `dep:bar` to enable the dependency.
",
        )
        .run();
}

#[cargo_test]
fn tree() {
    Package::new("baz", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat1", &["dep:baz"])
        .feature("feat2", &[])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", features = ["feat1"], optional=true }

                [features]
                a = ["bar/feat2"]
                b = ["dep:bar/feat2"]
                bar = ["dep:bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e features -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout("foo v0.1.0 ([ROOT]/foo)")
        .run();

    p.cargo("tree -e features --features a -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.1.0 ([ROOT]/foo)
├── bar feature \"default\"
│   └── bar v1.0.0
│       └── baz feature \"default\"
│           └── baz v1.0.0
└── bar feature \"feat1\"
    └── bar v1.0.0 (*)
",
        )
        .run();

    p.cargo("tree -e features --features a -i bar -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
bar v1.0.0
├── bar feature \"default\"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature \"a\" (command-line)
│       ├── foo feature \"bar\"
│       │   └── foo feature \"a\" (command-line)
│       └── foo feature \"default\" (command-line)
├── bar feature \"feat1\"
│   └── foo v0.1.0 ([ROOT]/foo) (*)
└── bar feature \"feat2\"
    └── foo feature \"a\" (command-line)
",
        )
        .run();

    p.cargo("tree -e features --features b -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.1.0 ([ROOT]/foo)
├── bar feature \"default\"
│   └── bar v1.0.0
│       └── baz feature \"default\"
│           └── baz v1.0.0
└── bar feature \"feat1\"
    └── bar v1.0.0 (*)
",
        )
        .run();

    p.cargo("tree -e features --features b -i bar -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
bar v1.0.0
├── bar feature \"default\"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature \"b\" (command-line)
│       └── foo feature \"default\" (command-line)
├── bar feature \"feat1\"
│   └── foo v0.1.0 ([ROOT]/foo) (*)
└── bar feature \"feat2\"
    └── foo feature \"b\" (command-line)
",
        )
        .run();

    p.cargo("tree -e features --features bar -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.1.0 ([ROOT]/foo)
├── bar feature \"default\"
│   └── bar v1.0.0
│       └── baz feature \"default\"
│           └── baz v1.0.0
└── bar feature \"feat1\"
    └── bar v1.0.0 (*)
",
        )
        .run();

    p.cargo("tree -e features --features bar -i bar -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
bar v1.0.0
├── bar feature \"default\"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature \"bar\" (command-line)
│       └── foo feature \"default\" (command-line)
└── bar feature \"feat1\"
    └── foo v0.1.0 ([ROOT]/foo) (*)
",
        )
        .run();
}

#[cargo_test]
fn publish_no_implicit() {
    // Does not include implicit features or dep: syntax on publish.
    Package::new("opt-dep1", "1.0.0").publish();
    Package::new("opt-dep2", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                description = "foo"
                license = "MIT"
                homepage = "https://example.com/"

                [dependencies]
                opt-dep1 = { version = "1.0", optional = true }
                opt-dep2 = { version = "1.0", optional = true }

                [features]
                feat = ["opt-dep1"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --token sekrit")
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.1.0 [..]
[UPLOADING] foo v0.1.0 [..]
",
        )
        .run();

    publish::validate_upload_with_contents(
        "v1",
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [
            {
              "default_features": true,
              "features": [],
              "kind": "normal",
              "name": "opt-dep1",
              "optional": true,
              "registry": "https://github.com/rust-lang/crates.io-index",
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [],
              "kind": "normal",
              "name": "opt-dep2",
              "optional": true,
              "registry": "https://github.com/rust-lang/crates.io-index",
              "target": null,
              "version_req": "^1.0"
            }
          ],
          "description": "foo",
          "documentation": null,
          "features": {
            "feat": ["opt-dep1"]
          },
          "homepage": "https://example.com/",
          "keywords": [],
          "license": "MIT",
          "license_file": null,
          "links": null,
          "name": "foo",
          "readme": null,
          "readme_file": null,
          "repository": null,
          "vers": "0.1.0"
          }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[(
            "Cargo.toml",
            r#"[..]
[package]
name = "foo"
version = "0.1.0"
description = "foo"
homepage = "https://example.com/"
license = "MIT"
[dependencies.opt-dep1]
version = "1.0"
optional = true

[dependencies.opt-dep2]
version = "1.0"
optional = true

[features]
feat = ["opt-dep1"]
"#,
        )],
    );
}

#[cargo_test]
fn publish() {
    // Publish uploads `features2` in JSON.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                description = "foo"
                license = "MIT"
                homepage = "https://example.com/"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [features]
                feat1 = []
                feat2 = ["dep:bar"]
                feat3 = ["feat2"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    registry::api_path().join("api/v2/crates").mkdir_p();

    p.cargo("publish --token sekrit -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.1.0 [..]
[VERIFYING] foo v0.1.0 [..]
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
[UPLOADING] foo v0.1.0 [..]
",
        )
        .run();

    publish::validate_upload_with_contents(
        "v2",
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [
            {
              "default_features": true,
              "features": [],
              "kind": "normal",
              "name": "bar",
              "optional": true,
              "registry": "https://github.com/rust-lang/crates.io-index",
              "target": null,
              "version_req": "^1.0"
            }
          ],
          "description": "foo",
          "documentation": null,
          "features": {
            "feat1": [],
            "feat2": [],
            "feat3": ["feat2"]
          },
          "features2": {
            "feat2": ["dep:bar"]
          },
          "homepage": "https://example.com/",
          "keywords": [],
          "license": "MIT",
          "license_file": null,
          "links": null,
          "name": "foo",
          "readme": null,
          "readme_file": null,
          "repository": null,
          "vers": "0.1.0"
          }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[(
            "Cargo.toml",
            r#"[..]
[package]
name = "foo"
version = "0.1.0"
description = "foo"
homepage = "https://example.com/"
license = "MIT"
[dependencies.bar]
version = "1.0"
optional = true

[features]
feat1 = []
feat2 = ["dep:bar"]
feat3 = ["feat2"]
"#,
        )],
    );
}

#[cargo_test]
fn old_registry_publish_error() {
    // What happens if a registry does not support the v2 api.
    let server = registry::RegistryBuilder::new().build_api_server(&|headers| {
        assert_eq!(headers[0], "PUT /api/v2/crates/new HTTP/1.1");
        (404, &"")
    });

    Package::new("bar", "1.0.0").alternative(true).publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                description = "foo"
                license = "MIT"
                homepage = "https://example.com/"

                [dependencies]
                bar = { version = "1.0", optional = true, registry = "alternative" }

                [features]
                feat = ["dep:bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --registry alternative -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr("\
[UPDATING] [..]
[PACKAGING] foo v0.1.0 [..]
[VERIFYING] foo v0.1.0 [..]
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
[UPLOADING] foo v0.1.0 [..]
[ERROR] This package uses new feature syntax that is not supported by the registry at http://127.0.0.1:[..]
")
        .run();

    server.join().unwrap();
}

// This is a test for exercising the behavior of older versions of cargo. You
// will need rustup installed. This will iterate over the installed
// toolchains, and run some tests over each one, producing a report at the
// end.
//
// This is ignored because it is intended to be run on a developer system with
// a bunch of toolchains installed. As of this writing, I have tested 1.0 to
// 1.51. Run this with:
//
//    cargo test --test testsuite -- old_cargos --nocapture --ignored
#[ignore]
#[cargo_test]
fn old_cargos() {
    if std::process::Command::new("rustup").output().is_err() {
        eprintln!("old_cargos ignored, rustup not installed");
        return;
    }
    Package::new("new-baz-dep", "1.0.0").publish();

    Package::new("baz", "1.0.0").publish();
    Package::new("baz", "1.0.1")
        .add_dep(Dependency::new("new-baz-dep", "1.0").optional(true))
        .feature("new-feat", &["dep:new-baz-dep"])
        .publish();

    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat", &["baz"])
        .publish();
    let bar_cksum = Package::new("bar", "1.0.1")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat", &["dep:baz"])
        .publish();
    Package::new("bar", "1.0.2")
        .add_dep(Dependency::new("baz", "1.0").enable_features(&["new-feat"]))
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Collect a sorted list of all installed toolchains for this host.
    let host = rustc_host();
    // I tend to have lots of toolchains installed, but I don't want to test
    // all of them (like dated nightlies, or toolchains for non-host targets).
    let valid_names = &[
        format!("stable-{}", host),
        format!("beta-{}", host),
        format!("nightly-{}", host),
    ];
    let output = cargo::util::process("rustup")
        .args(&["toolchain", "list"])
        .exec_with_output()
        .expect("rustup should be installed");
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let mut toolchains: Vec<_> = stdout
        .lines()
        .map(|line| {
            // Some lines say things like (default), just get the version.
            line.split_whitespace().next().expect("non-empty line")
        })
        .filter(|line| {
            line.ends_with(&host)
                && (line.starts_with("1.") || valid_names.iter().any(|name| name == line))
        })
        .map(|line| {
            let output = cargo::util::process("rustc")
                .args(&[format!("+{}", line).as_str(), "-V"])
                .exec_with_output()
                .expect("rustc installed");
            let version = std::str::from_utf8(&output.stdout).unwrap();
            let parts: Vec<_> = version.split_whitespace().collect();
            assert_eq!(parts[0], "rustc");
            assert!(parts[1].starts_with("1."));

            (
                semver::Version::parse(parts[1]).expect("valid version"),
                line,
            )
        })
        .collect();

    toolchains.sort_by(|a, b| a.0.cmp(&b.0));

    let config_path = paths::home().join(".cargo/config");
    let lock_path = p.root().join("Cargo.lock");

    // Results collected for printing a final report.
    let mut results: Vec<Vec<String>> = Vec::new();

    for (version, toolchain) in toolchains {
        let mut toolchain_result = vec![toolchain.to_string()];
        if version < semver::Version::new(1, 12, 0) {
            fs::write(
                &config_path,
                format!(
                    r#"
                        [registry]
                        index = "{}"
                    "#,
                    registry::registry_url()
                ),
            )
            .unwrap();
        } else {
            fs::write(
                &config_path,
                format!(
                    "
                        [source.crates-io]
                        registry = 'https://wut'  # only needed by 1.12
                        replace-with = 'dummy-registry'

                        [source.dummy-registry]
                        registry = '{}'
                    ",
                    registry::registry_url()
                ),
            )
            .unwrap();
        }

        let run_cargo = || -> String {
            match process("cargo")
                .args(&[format!("+{}", toolchain).as_str(), "build"])
                .cwd(p.root())
                .exec_with_output()
            {
                Ok(_output) => {
                    eprintln!("{} ok", toolchain);
                    let output = process("cargo")
                        .args(&[format!("+{}", toolchain).as_str(), "pkgid", "bar"])
                        .cwd(p.root())
                        .exec_with_output()
                        .expect("pkgid should succeed");
                    let stdout = std::str::from_utf8(&output.stdout).unwrap();
                    let version = stdout
                        .trim()
                        .rsplitn(2, ':')
                        .next()
                        .expect("version after colon");
                    format!("success bar={}", version)
                }
                Err(e) => {
                    eprintln!("{} err {}", toolchain, e);
                    "failed".to_string()
                }
            }
        };

        lock_path.rm_rf();
        p.build_dir().rm_rf();

        toolchain_result.push(run_cargo());
        if version < semver::Version::new(1, 12, 0) {
            p.change_file(
                "Cargo.lock",
                &format!(
                    r#"
                        [root]
                        name = "foo"
                        version = "0.1.0"
                        dependencies = [
                         "bar 1.0.1 (registry+{url})",
                        ]

                        [[package]]
                        name = "bar"
                        version = "1.0.1"
                        source = "registry+{url}"
                    "#,
                    url = registry::registry_url()
                ),
            );
        } else {
            p.change_file(
                "Cargo.lock",
                &format!(
                    r#"
                        [root]
                        name = "foo"
                        version = "0.1.0"
                        dependencies = [
                         "bar 1.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                        ]

                        [[package]]
                        name = "bar"
                        version = "1.0.1"
                        source = "registry+https://github.com/rust-lang/crates.io-index"

                        [metadata]
                        "checksum bar 1.0.1 (registry+https://github.com/rust-lang/crates.io-index)" = "{}"
                    "#,
                    bar_cksum
                ),
            );
        }
        toolchain_result.push(run_cargo());
        results.push(toolchain_result);
    }

    // Generate a report.
    let headers = vec!["Version", "Unlocked", "Locked"];
    let init: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    let col_widths = results.iter().fold(init, |acc, row| {
        acc.iter().zip(row).map(|(a, b)| *a.max(&b.len())).collect()
    });
    // Print headers
    let spaced: Vec<_> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            format!(
                " {}{}",
                h,
                " ".repeat(col_widths[i].saturating_sub(h.len()))
            )
        })
        .collect();
    eprintln!("{}", spaced.join(" |"));
    let lines: Vec<_> = col_widths.iter().map(|w| "-".repeat(*w + 2)).collect();
    eprintln!("{}", lines.join("|"));
    // Print columns.
    for row in results {
        let rs: Vec<_> = row
            .iter()
            .enumerate()
            .map(|(i, c)| {
                format!(
                    " {}{} ",
                    c,
                    " ".repeat(col_widths[i].saturating_sub(c.len()))
                )
            })
            .collect();
        eprintln!("{}", rs.join("|"));
    }
}
