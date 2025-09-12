//! Tests for namespaced features.

use crate::prelude::*;
use cargo_test_support::registry::{Dependency, Package, RegistryBuilder};
use cargo_test_support::str;
use cargo_test_support::{project, publish};

use super::features2::switch_to_resolver_2;

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
                edition = "2015"

                [dependencies]
                bar = {version="1.0", features=["feat"]}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] baz v1.0.0
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
                authors = []

                [features]
                bar = ["baz"]
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `bar` includes `baz` which is neither a dependency nor another feature

  [HELP] a feature with a similar name exists: `bar`

"#]])
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
                edition = "2015"

                [features]
                bar = ["dep:baz"]
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `bar` includes `dep:baz`, but `baz` is not listed as a dependency

"#]])
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
                edition = "2015"

                [features]
                bar = ["dep:baz"]

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `bar` includes `dep:baz`, but `baz` is not an optional dependency
  A non-optional dependency of the same name is defined; consider adding `optional = true` to its definition.

"#]])
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
                edition = "2015"

                [features]
                bar = ["baz"]

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check --features baz")
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 (registry `dummy-registry`)
[CHECKING] baz v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"

                [features]
                baz = []

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  optional dependency `baz` is not included in any feature
  Make sure that `dep:baz` is included in one of features in the [features] table.

"#]])
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
                edition = "2015"

                [features]
                baz = []

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();
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
                edition = "2015"

                [features]
                bar = ["baz"]

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `bar` includes `baz`, but `baz` is not an optional dependency
  A non-optional dependency of the same name is defined; consider adding `optional = true` to its definition.

"#]])
        .run();
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
                edition = "2015"

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

    p.cargo("run")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data("")
        .run();

    p.cargo("run --features baz")
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 (registry `dummy-registry`)
[COMPILING] baz v0.1.0
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
baz

"#]])
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
                edition = "2015"

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
                    #[allow(unexpected_cfgs)]
                    if cfg!(feature = "lazy_static") { println!("lazy_static"); }
                }
            "#,
        )
        .build();

    p.cargo("run")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data("")
        .run();

    p.cargo("run --features regex")
        .with_stderr_data(
            str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] regex v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] lazy_static v1.0.0 (registry `dummy-registry`)
[COMPILING] regex v1.0.0
[COMPILING] lazy_static v1.0.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]]
            .unordered(),
        )
        .with_stdout_data(str![[r#"
regex

"#]])
        .run();

    p.cargo("run --features lazy_static")
        .with_stderr_data(str![[r#"
[ERROR] package `foo v0.1.0 ([ROOT]/foo)` does not have feature `lazy_static`

[HELP] an optional dependency with that name exists, but the `features` table includes it with the "dep:" syntax so it does not have an implicit feature with that name
Dependency `lazy_static` would be enabled by these features:
	- `regex`

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version="1.0", optional=true }

                [features]
                "dep:bar" = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check --features dep:bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] feature named `dep:bar` is not allowed to start with `dep:`
  --> Cargo.toml:11:17
   |
11 |                 "dep:bar" = []
   |                 ^^^^^^^^^

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", features = ["dep:baz"] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `dep:baz` in dependency `bar` is not allowed to use explicit `dep:` syntax
  If you want to enable an optional dependency, specify the name of the optional dependency without the `dep:` prefix, or specify a feature from the dependency's `[features]` table that enables the optional dependency.

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional=true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check --features dep:bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] feature `dep:bar` is not allowed to use explicit `dep:` syntax

"#]])
        .run();

    switch_to_resolver_2(&p);
    p.cargo("check --features dep:bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] feature `dep:bar` is not allowed to use explicit `dep:` syntax

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional=true }

                [[bin]]
                name = "foo"
                required-features = ["dep:bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[ERROR] invalid feature `dep:bar` in required-features of target `foo`: `dep:` prefixed feature values are not allowed in required-features

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional=true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("metadata --no-deps")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": "{...}",
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {
        "bar": [
          "dep:bar"
        ]
      },
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": "{...}",
      "version": "0.1.0"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}

"#]]
            .is_json(),
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
                edition = "2015"

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

    p.cargo("check --features f1")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [features]
                feat1 = ["dep:bar"]
                feat2 = ["bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `feat2` includes `bar`, but `bar` is an optional dependency without an implicit feature
  Use `dep:bar` to enable the dependency.

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", features = ["feat1"], optional=true }

                [features]
                a = ["bar/feat2"]
                bar = ["dep:bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e features")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    p.cargo("tree -e features --features a")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── bar feature "default"
│   └── bar v1.0.0
│       └── baz feature "default"
│           └── baz v1.0.0
└── bar feature "feat1"
    └── bar v1.0.0 (*)

"#]])
        .run();

    p.cargo("tree -e features --features a -i bar")
        .with_stdout_data(str![[r#"
bar v1.0.0
├── bar feature "default"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature "a" (command-line)
│       ├── foo feature "bar"
│       │   └── foo feature "a" (command-line)
│       └── foo feature "default" (command-line)
├── bar feature "feat1"
│   └── foo v0.1.0 ([ROOT]/foo) (*)
└── bar feature "feat2"
    └── foo feature "a" (command-line)

"#]])
        .run();

    p.cargo("tree -e features --features bar")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
├── bar feature "default"
│   └── bar v1.0.0
│       └── baz feature "default"
│           └── baz v1.0.0
└── bar feature "feat1"
    └── bar v1.0.0 (*)

"#]])
        .run();

    p.cargo("tree -e features --features bar -i bar")
        .with_stdout_data(str![[r#"
bar v1.0.0
├── bar feature "default"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature "bar" (command-line)
│       └── foo feature "default" (command-line)
└── bar feature "feat1"
    └── foo v0.1.0 ([ROOT]/foo) (*)

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional=true }

                [features]
                a = ["dep:bar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e features")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    p.cargo("tree -e features --all-features")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar feature "default"
    └── bar v1.0.0

"#]])
        .run();

    p.cargo("tree -e features -i bar --all-features")
        .with_stdout_data(str![[r#"
bar v1.0.0
└── bar feature "default"
    └── foo v0.1.0 ([ROOT]/foo)
        ├── foo feature "a" (command-line)
        └── foo feature "default" (command-line)

"#]])
        .run();
}

#[cargo_test]
fn publish_no_implicit() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

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
                edition = "2015"
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

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.1.0 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.1.0 ([ROOT]/foo)
[UPLOADED] foo v0.1.0 to registry `crates-io`
[NOTE] waiting for foo v0.1.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.1.0 at registry `crates-io`

"#]])
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
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [],
              "kind": "normal",
              "name": "opt-dep2",
              "optional": true,
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
          "rust_version": null,
          "vers": "0.1.0"
          }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.1.0"
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
homepage = "https://example.com/"
readme = false
license = "MIT"

[features]
feat = ["opt-dep1"]

[lib]
name = "foo"
path = "src/lib.rs"

[dependencies.opt-dep1]
version = "1.0"
optional = true

[dependencies.opt-dep2]
version = "1.0"
optional = true

"##]],
        )],
    );
}

#[cargo_test]
fn publish() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    // Publish behavior with explicit dep: syntax.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
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

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.1.0 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.1.0 ([ROOT]/foo)
[COMPILING] foo v0.1.0 ([ROOT]/foo/target/package/foo-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.1.0 ([ROOT]/foo)
[UPLOADED] foo v0.1.0 to registry `crates-io`
[NOTE] waiting for foo v0.1.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.1.0 at registry `crates-io`

"#]])
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
          "rust_version": null,
          "vers": "0.1.0"
          }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.1.0"
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
homepage = "https://example.com/"
readme = false
license = "MIT"

[features]
feat1 = []
feat2 = ["dep:bar"]
feat3 = ["feat2"]

[lib]
name = "foo"
path = "src/lib.rs"

[dependencies.bar]
version = "1.0"
optional = true

"##]],
        )],
    );
}

#[cargo_test]
fn namespaced_feature_together() {
    // Check for an error when `dep:` is used with `/`
    Package::new("bar", "1.0.0")
        .feature("bar-feat", &[])
        .publish();

    // Non-optional shouldn't have extra err.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"

                [features]
                f1 = ["dep:bar/bar-feat"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `f1` includes `dep:bar/bar-feat` with both `dep:` and `/`
  To fix this, remove the `dep:` prefix.

"#]])
        .run();

    // Weak dependency shouldn't have extra err.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar = {version = "1.0", optional = true }

            [features]
            f1 = ["dep:bar?/bar-feat"]
        "#,
    );
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `f1` includes `dep:bar?/bar-feat` with both `dep:` and `/`
  To fix this, remove the `dep:` prefix.

"#]])
        .run();

    // If dep: is already specified, shouldn't have extra err.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar = {version = "1.0", optional = true }

            [features]
            f1 = ["dep:bar", "dep:bar/bar-feat"]
        "#,
    );
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `f1` includes `dep:bar/bar-feat` with both `dep:` and `/`
  To fix this, remove the `dep:` prefix.

"#]])
        .run();

    // Only when the other 3 cases aren't true should it give some extra help.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar = {version = "1.0", optional = true }

            [features]
            f1 = ["dep:bar/bar-feat"]
        "#,
    );
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `f1` includes `dep:bar/bar-feat` with both `dep:` and `/`
  To fix this, remove the `dep:` prefix.
  If the intent is to avoid creating an implicit feature `bar` for an optional dependency, then consider replacing this with two values:
      "dep:bar", "bar/bar-feat"

"#]])
        .run();
}

#[cargo_test]
fn dep_feature_when_hidden() {
    // Checks for behavior with dep:bar and bar/feat syntax when there is no
    // `bar` feature.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = { path = "bar", optional = true }

                [features]
                f1 = ["dep:bar"]
                f2 = ["bar/bar_feat"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"

                [features]
                bar_feat = []
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("tree -f")
        .arg("{p} features={f}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) features=

"#]])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version

"#]])
        .run();

    p.cargo("tree -F f1 -f")
        .arg("{p} features={f}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) features=f1
└── bar v0.1.0 ([ROOT]/foo/bar) features=

"#]])
        .with_stderr_data("")
        .run();

    p.cargo("tree -F f2 -f")
        .arg("{p} features={f}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) features=f2
└── bar v0.1.0 ([ROOT]/foo/bar) features=bar_feat

"#]])
        .with_stderr_data("")
        .run();

    p.cargo("tree --all-features -f")
        .arg("{p} features={f}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) features=f1,f2
└── bar v0.1.0 ([ROOT]/foo/bar) features=bar_feat

"#]])
        .with_stderr_data("")
        .run();
}
