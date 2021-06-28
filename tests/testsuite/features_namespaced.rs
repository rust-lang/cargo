//! Tests for namespaced features.

use super::features2::switch_to_resolver_2;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{project, publish, rustc_host};

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
        .with_stderr(&format!(
            "\
[UPDATING] [..]
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
[RUNNING] `target/{}/debug/foo[EXE]`
",
            rustc_host()
        ))
        .with_stdout("")
        .run();

    p.cargo("run -Z namespaced-features --features regex")
        .masquerade_as_nightly_cargo()
        .with_stderr_unordered(&format!(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] regex v1.0.0 [..]
[DOWNLOADED] lazy_static v1.0.0 [..]
[COMPILING] regex v1.0.0
[COMPILING] lazy_static v1.0.0
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
[RUNNING] `target/{}/debug/foo[EXE]`
",
            rustc_host()
        ))
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
error: failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  feature `dep:baz` in dependency `bar` is not allowed to use explicit `dep:` syntax
  If you want to enable [..]
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
[ERROR] feature `dep:bar` is not allowed to use explicit `dep:` syntax
",
        )
        .run();

    switch_to_resolver_2(&p);
    p.cargo("check -Z namespaced-features --features dep:bar")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] feature `dep:bar` is not allowed to use explicit `dep:` syntax
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
                      "default_run": null,
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
fn tree_no_implicit() {
    // tree without an implicit feature
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

                [features]
                a = ["dep:bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e features -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout("foo v0.1.0 ([ROOT]/foo)")
        .run();

    p.cargo("tree -e features --all-features -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.1.0 ([ROOT]/foo)
└── bar feature \"default\"
    └── bar v1.0.0
",
        )
        .run();

    p.cargo("tree -e features -i bar --all-features -Z namespaced-features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
bar v1.0.0
└── bar feature \"default\"
    └── foo v0.1.0 ([ROOT]/foo)
        └── foo feature \"a\" (command-line)
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
            &format!(
                r#"{}
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
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}

#[cargo_test]
fn publish() {
    // Publish behavior with explicit dep: syntax.
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
            "feat2": ["dep:bar"],
            "feat3": ["feat2"]
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
            &format!(
                r#"{}
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
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}
