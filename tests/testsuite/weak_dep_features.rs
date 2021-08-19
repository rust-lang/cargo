//! Tests for weak-dep-features.

use super::features2::switch_to_resolver_2;
use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{project, publish};
use std::fmt::Write;

// Helper to create lib.rs files that check features.
fn require(enabled_features: &[&str], disabled_features: &[&str]) -> String {
    let mut s = String::new();
    for feature in enabled_features {
        writeln!(s, "#[cfg(not(feature=\"{feature}\"))] compile_error!(\"expected feature {feature} to be enabled\");",
            feature=feature).unwrap();
    }
    for feature in disabled_features {
        writeln!(s, "#[cfg(feature=\"{feature}\")] compile_error!(\"did not expect feature {feature} to be enabled\");",
            feature=feature).unwrap();
    }
    s
}

#[cargo_test]
fn gated() {
    // Need -Z weak-dep-features to enable.
    Package::new("bar", "1.0.0").feature("feat", &[]).publish();
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
                f1 = ["bar?/feat"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  optional dependency features with `?` syntax are only allowed on the nightly \
  channel and requires the `-Z weak-dep-features` flag on the command line
  Feature `f1` had feature value `bar?/feat`.
",
        )
        .run();
}

#[cargo_test]
fn dependency_gate_ignored() {
    // Dependencies with ? features in the registry are ignored in the
    // registry if not on nightly.
    Package::new("baz", "1.0.0").feature("feat", &[]).publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat", &["baz?/feat"])
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
location searched: registry `crates-io`
required by package `foo v0.1.0 ([..]/foo)`
",
        )
        .run();

    // Publish a version without the ? feature, it should ignore 1.0.0
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
fn simple() {
    Package::new("bar", "1.0.0")
        .feature("feat", &[])
        .file("src/lib.rs", &require(&["feat"], &[]))
        .publish();
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
                f1 = ["bar?/feat"]
            "#,
        )
        .file("src/lib.rs", &require(&["f1"], &[]))
        .build();

    // It's a bit unfortunate that this has to download `bar`, but avoiding
    // that is extremely difficult.
    p.cargo("check -Z weak-dep-features --features f1")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 [..]
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("check -Z weak-dep-features --features f1,bar")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn deferred() {
    // A complex chain that requires deferring enabling the feature due to
    // another dependency getting enabled.
    Package::new("bar", "1.0.0")
        .feature("feat", &[])
        .file("src/lib.rs", &require(&["feat"], &[]))
        .publish();
    Package::new("dep", "1.0.0")
        .add_dep(Dependency::new("bar", "1.0").optional(true))
        .feature("feat", &["bar?/feat"])
        .publish();
    Package::new("bar_activator", "1.0.0")
        .feature_dep("dep", "1.0", &["bar"])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                dep = { version = "1.0", features = ["feat"] }
                bar_activator = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Z weak-dep-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 [..]
[DOWNLOADED] bar_activator v1.0.0 [..]
[DOWNLOADED] bar v1.0.0 [..]
[CHECKING] bar v1.0.0
[CHECKING] dep v1.0.0
[CHECKING] bar_activator v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn not_optional_dep() {
    // Attempt to use dep_name?/feat where dep_name is not optional.
    Package::new("dep", "1.0.0").feature("feat", &[]).publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                dep = "1.0"

                [features]
                feat = ["dep?/feat"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Z weak-dep-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr("\
error: failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `feat` includes `dep?/feat` with a `?`, but `dep` is not an optional dependency
  A non-optional dependency of the same name is defined; consider removing the `?` or changing the dependency to be optional
")
        .run();
}

#[cargo_test]
fn optional_cli_syntax() {
    // --features bar?/feat
    Package::new("bar", "1.0.0")
        .feature("feat", &[])
        .file("src/lib.rs", &require(&["feat"], &[]))
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", optional = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Does not build bar.
    p.cargo("check --features bar?/feat -Z weak-dep-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 [..]
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    // Builds bar.
    p.cargo("check --features bar?/feat,bar -Z weak-dep-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    eprintln!("check V2 resolver");
    switch_to_resolver_2(&p);
    p.build_dir().rm_rf();
    // Does not build bar.
    p.cargo("check --features bar?/feat -Z weak-dep-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    // Builds bar.
    p.cargo("check --features bar?/feat,bar -Z weak-dep-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn required_features() {
    // required-features doesn't allow ?
    Package::new("bar", "1.0.0").feature("feat", &[]).publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [[bin]]
                name = "foo"
                required-features = ["bar?/feat"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Z weak-dep-features")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] invalid feature `bar?/feat` in required-features of target `foo`: \
optional dependency with `?` is not allowed in required-features
",
        )
        .run();
}

#[cargo_test]
fn weak_with_host_decouple() {
    // -Z weak-opt-features with new resolver
    //
    // foo v0.1.0
    // └── common v1.0.0
    //     └── bar v1.0.0        <-- does not have `feat` enabled
    // [build-dependencies]
    // └── bar_activator v1.0.0
    //     └── common v1.0.0
    //         └── bar v1.0.0    <-- does have `feat` enabled
    Package::new("bar", "1.0.0")
        .feature("feat", &[])
        .file(
            "src/lib.rs",
            r#"
                pub fn feat() -> bool {
                    cfg!(feature = "feat")
                }
            "#,
        )
        .publish();

    Package::new("common", "1.0.0")
        .add_dep(Dependency::new("bar", "1.0").optional(true))
        .feature("feat", &["bar?/feat"])
        .file(
            "src/lib.rs",
            r#"
                #[cfg(feature = "bar")]
                pub fn feat() -> bool { bar::feat() }
                #[cfg(not(feature = "bar"))]
                pub fn feat() -> bool { false }
            "#,
        )
        .publish();

    Package::new("bar_activator", "1.0.0")
        .feature_dep("common", "1.0", &["bar", "feat"])
        .file(
            "src/lib.rs",
            r#"
                pub fn feat() -> bool {
                    common::feat()
                }
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
                resolver = "2"

                [dependencies]
                common = { version = "1.0", features = ["feat"] }

                [build-dependencies]
                bar_activator = "1.0"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    assert!(!common::feat());
                }
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    assert!(bar_activator::feat());
                }
            "#,
        )
        .build();

    p.cargo("run -Z weak-dep-features")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] [..]
[DOWNLOADED] [..]
[DOWNLOADED] [..]
[COMPILING] bar v1.0.0
[COMPILING] common v1.0.0
[COMPILING] bar_activator v1.0.0
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
[RUNNING] `target/debug/foo[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn weak_namespaced() {
    // Behavior with a dep: dependency.
    Package::new("bar", "1.0.0")
        .feature("feat", &[])
        .file("src/lib.rs", &require(&["feat"], &[]))
        .publish();
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
                f1 = ["bar?/feat"]
                f2 = ["dep:bar"]
            "#,
        )
        .file("src/lib.rs", &require(&["f1"], &["f2", "bar"]))
        .build();

    p.cargo("check -Z weak-dep-features -Z namespaced-features --features f1")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 [..]
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("tree -Z weak-dep-features -Z namespaced-features -f")
        .arg("{p} feats:{f}")
        .masquerade_as_nightly_cargo()
        .with_stdout("foo v0.1.0 ([ROOT]/foo) feats:")
        .run();

    p.cargo("tree -Z weak-dep-features -Z namespaced-features --features f1 -f")
        .arg("{p} feats:{f}")
        .masquerade_as_nightly_cargo()
        .with_stdout("foo v0.1.0 ([ROOT]/foo) feats:f1")
        .run();

    p.cargo("tree -Z weak-dep-features -Z namespaced-features --features f1,f2 -f")
        .arg("{p} feats:{f}")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.1.0 ([ROOT]/foo) feats:f1,f2
└── bar v1.0.0 feats:feat
",
        )
        .run();

    // "bar" remains not-a-feature
    p.change_file("src/lib.rs", &require(&["f1", "f2"], &["bar"]));

    p.cargo("check -Z weak-dep-features -Z namespaced-features --features f1,f2")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn tree() {
    Package::new("bar", "1.0.0")
        .feature("feat", &[])
        .file("src/lib.rs", &require(&["feat"], &[]))
        .publish();
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
                f1 = ["bar?/feat"]
            "#,
        )
        .file("src/lib.rs", &require(&["f1"], &[]))
        .build();

    p.cargo("tree -Z weak-dep-features --features f1")
        .masquerade_as_nightly_cargo()
        .with_stdout("foo v0.1.0 ([ROOT]/foo)")
        .run();

    p.cargo("tree -Z weak-dep-features --features f1,bar")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0
",
        )
        .run();

    p.cargo("tree -Z weak-dep-features --features f1,bar -e features")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.1.0 ([ROOT]/foo)
└── bar feature \"default\"
    └── bar v1.0.0
",
        )
        .run();

    p.cargo("tree -Z weak-dep-features --features f1,bar -e features -i bar")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
bar v1.0.0
├── bar feature \"default\"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature \"bar\" (command-line)
│       ├── foo feature \"default\" (command-line)
│       └── foo feature \"f1\" (command-line)
└── bar feature \"feat\"
    └── foo feature \"f1\" (command-line)
",
        )
        .run();

    p.cargo("tree -Z weak-dep-features -e features --features bar?/feat")
        .masquerade_as_nightly_cargo()
        .with_stdout("foo v0.1.0 ([ROOT]/foo)")
        .run();

    // This is a little strange in that it produces no output.
    // Maybe `cargo tree` should print a note about why?
    p.cargo("tree -Z weak-dep-features -e features -i bar --features bar?/feat")
        .masquerade_as_nightly_cargo()
        .with_stdout("")
        .run();

    p.cargo("tree -Z weak-dep-features -e features -i bar --features bar?/feat,bar")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
bar v1.0.0
├── bar feature \"default\"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature \"bar\" (command-line)
│       └── foo feature \"default\" (command-line)
└── bar feature \"feat\" (command-line)
",
        )
        .run();
}

#[cargo_test]
fn publish() {
    // Publish behavior with /? syntax.
    Package::new("bar", "1.0.0").feature("feat", &[]).publish();
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
                feat2 = ["bar?/feat"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --token sekrit -Z weak-dep-features")
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
            "feat2": ["bar?/feat"]
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
feat2 = ["bar?/feat"]
"#,
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}
