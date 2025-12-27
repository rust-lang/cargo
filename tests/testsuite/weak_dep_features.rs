//! Tests for weak-dep-features.

use std::fmt::Write;

use crate::prelude::*;
use cargo_test_support::registry::{Dependency, Package, RegistryBuilder};
use cargo_test_support::str;
use cargo_test_support::{project, publish};

use super::features2::switch_to_resolver_2;

// Helper to create lib.rs files that check features.
fn require(enabled_features: &[&str], disabled_features: &[&str]) -> String {
    let mut s = String::new();
    writeln!(s, "#![allow(unexpected_cfgs)]").unwrap();
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
                edition = "2015"

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
    p.cargo("check --features f1")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check --features f1,bar")
        .with_stderr_data(str![[r#"
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"

                [dependencies]
                dep = { version = "1.0", features = ["feat"] }
                bar_activator = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar_activator v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[CHECKING] dep v1.0.0
[CHECKING] bar_activator v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn deferred_v5() {
    // A modified version of the deferred test from above to enable
    // an entire dependency in a deferred way.
    Package::new("bar", "1.0.0")
        .feature("feat", &["feat_dep"])
        .add_dep(Dependency::new("feat_dep", "1.0").optional(true))
        .file("src/lib.rs", "extern crate feat_dep;")
        .publish();
    Package::new("dep", "1.0.0")
        .add_dep(Dependency::new("bar", "1.0").optional(true))
        .feature("feat", &["bar?/feat"])
        .publish();
    Package::new("bar_activator", "1.0.0")
        .feature_dep("dep", "1.0", &["bar"])
        .publish();
    Package::new("feat_dep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                dep = { version = "1.0", features = ["feat"] }
                bar_activator = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 4 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] feat_dep v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar_activator v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] feat_dep v1.0.0
[CHECKING] bar v1.0.0
[CHECKING] dep v1.0.0
[CHECKING] bar_activator v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    let lockfile = p.read_lockfile();

    assert!(
        lockfile.contains(r#"version = 4"#),
        "lockfile version is not 4!\n{lockfile}",
    );
    // Previous behavior: feat_dep is inside lockfile.
    assert!(
        lockfile.contains(r#"name = "feat_dep""#),
        "feat_dep not found\n{lockfile}",
    );
    // Update to new lockfile version
    let new_lockfile = lockfile.replace("version = 4", "version = 5");
    p.change_file("Cargo.lock", &new_lockfile);

    // We should still compile feat_dep
    p.cargo("check -Znext-lockfile-bump")
        .masquerade_as_nightly_cargo(&["weak_namespaced_check"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(
        lockfile.contains(r#"version = 5"#),
        "lockfile version is not 5!\n{lockfile}",
    );
    // New behavior: feat_dep is still there.
    assert!(
        lockfile.contains(r#"name = "feat_dep""#),
        "feat_dep not found\n{lockfile}",
    );
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
                edition = "2015"

                [dependencies]
                dep = "1.0"

                [features]
                feat = ["dep?/feat"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `feat` includes `dep?/feat` with a `?`, but `dep` is not an optional dependency
  A non-optional dependency of the same name is defined; consider removing the `?` or changing the dependency to be optional

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Does not build bar.
    p.cargo("check --features bar?/feat")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Builds bar.
    p.cargo("check --features bar?/feat,bar")
        .with_stderr_data(str![[r#"
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    eprintln!("check V2 resolver");
    switch_to_resolver_2(&p);
    p.build_dir().rm_rf();
    // Does not build bar.
    p.cargo("check --features bar?/feat")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Builds bar.
    p.cargo("check --features bar?/feat,bar")
        .with_stderr_data(str![[r#"
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [[bin]]
                name = "foo"
                required-features = ["bar?/feat"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[ERROR] invalid feature `bar?/feat` in required-features of target `foo`: optional dependency with `?` is not allowed in required-features

"#]])
        .run();
}

#[cargo_test]
fn weak_with_host_decouple() {
    // weak-dep-features with new resolver
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
                edition = "2015"
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

    p.cargo("run")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] common v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar_activator v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[COMPILING] bar v1.0.0
[COMPILING] common v1.0.0
[COMPILING] bar_activator v1.0.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [features]
                f1 = ["bar?/feat"]
                f2 = ["dep:bar"]
            "#,
        )
        .file("src/lib.rs", &require(&["f1"], &["f2", "bar"]))
        .build();

    p.cargo("check --features f1")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("tree -f")
        .arg("{p} feats:{f}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) feats:

"#]])
        .run();

    p.cargo("tree --features f1 -f")
        .arg("{p} feats:{f}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) feats:f1

"#]])
        .run();

    p.cargo("tree --features f1,f2 -f")
        .arg("{p} feats:{f}")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo) feats:f1,f2
└── bar v1.0.0 feats:feat

"#]])
        .run();

    // "bar" remains not-a-feature
    p.change_file("src/lib.rs", &require(&["f1", "f2"], &["bar"]));

    p.cargo("check --features f1,f2")
        .with_stderr_data(str![[r#"
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn weak_namespaced_v5() {
    // Behavior with a dep: dependency.
    Package::new("maybe_enabled", "1.0.0")
        .feature("feat", &[])
        .file("src/lib.rs", &require(&["feat"], &[]))
        .publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("maybe_enabled", "1.0").optional(true))
        .feature("f1", &["maybe_enabled?/feat"])
        .feature("f2", &["dep:maybe_enabled"])
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
                bar = { version = "1.0", features = ["f2", "f1"] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] maybe_enabled v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] maybe_enabled v1.0.0
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();

    assert!(
        lockfile.contains(r#"version = 4"#),
        "lockfile version is not 4!\n{lockfile}",
    );
    // Previous behavior: maybe_enabled is inside lockfile.
    assert!(
        lockfile.contains(r#"name = "maybe_enabled""#),
        "maybe_enabled not found\n{lockfile}",
    );
    // Update to new lockfile version
    let new_lockfile = lockfile.replace("version = 4", "version = 5");
    p.change_file("Cargo.lock", &new_lockfile);

    // We should still compile maybe_enabled
    p.cargo("check -Znext-lockfile-bump")
        .masquerade_as_nightly_cargo(&["weak_namespaced_check"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(
        lockfile.contains(r#"version = 5"#),
        "lockfile version is not 5!\n{lockfile}",
    );
    // New behavior: maybe_enabled is still there.
    assert!(
        lockfile.contains(r#"name = "maybe_enabled""#),
        "maybe_enabled not found\n{lockfile}",
    );
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [features]
                f1 = ["bar?/feat"]
            "#,
        )
        .file("src/lib.rs", &require(&["f1"], &[]))
        .build();

    p.cargo("tree --features f1")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    p.cargo("tree --features f1,bar")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0

"#]])
        .run();

    p.cargo("tree --features f1,bar -e features")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar feature "default"
    └── bar v1.0.0

"#]])
        .run();

    p.cargo("tree --features f1,bar -e features -i bar")
        .with_stdout_data(str![[r#"
bar v1.0.0
├── bar feature "default"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature "bar" (command-line)
│       ├── foo feature "default" (command-line)
│       └── foo feature "f1" (command-line)
└── bar feature "feat"
    └── foo feature "f1" (command-line)

"#]])
        .run();

    p.cargo("tree -e features --features bar?/feat")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)

"#]])
        .run();

    // This is a little strange in that it produces no output.
    // Maybe `cargo tree` should print a note about why?
    p.cargo("tree -e features -i bar --features bar?/feat")
        .with_stdout_data("")
        .run();

    p.cargo("tree -e features -i bar --features bar?/feat,bar")
        .with_stdout_data(str![[r#"
bar v1.0.0
├── bar feature "default"
│   └── foo v0.1.0 ([ROOT]/foo)
│       ├── foo feature "bar" (command-line)
│       └── foo feature "default" (command-line)
└── bar feature "feat" (command-line)

"#]])
        .run();
}

#[cargo_test]
fn publish() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    // Publish behavior with /? syntax.
    Package::new("bar", "1.0.0").feature("feat", &[]).publish();
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
                feat2 = ["bar?/feat"]
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
feat2 = ["bar?/feat"]

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
fn disabled_weak_direct_dep() {
    // Issue #10801
    // A weak direct dependency should be included in Cargo.lock,
    // even if disabled, and even if on lockfile version 5.
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
                edition = "2015"

                [dependencies]
                bar = { version = "1.0", optional = true }

                [features]
                f1 = ["bar?/feat"]
            "#,
        )
        .file("src/lib.rs", &require(&["f1"], &[]))
        .build();

    p.cargo("generate-lockfile").run();

    let lockfile = p.read_lockfile();
    assert!(
        lockfile.contains(r#"version = 4"#),
        "lockfile version is not 4!\n{lockfile}",
    );
    // Previous behavior: bar is inside lockfile.
    assert!(
        lockfile.contains(r#"name = "bar""#),
        "bar not found\n{lockfile}",
    );

    // Update to new lockfile version
    let new_lockfile = lockfile.replace("version = 4", "version = 5");
    p.change_file("Cargo.lock", &new_lockfile);

    p.cargo("check -Znext-lockfile-bump --features f1")
        .masquerade_as_nightly_cargo(&["weak_namespaced_check"])
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(
        lockfile.contains(r#"version = 5"#),
        "lockfile version is not 5!\n{lockfile}",
    );
    // New behavior: bar is still there because it is a direct (optional) dependency.
    assert!(
        lockfile.contains(r#"name = "bar""#),
        "bar not found\n{lockfile}",
    );

    p.cargo("check -Znext-lockfile-bump --features f1,bar")
        .masquerade_as_nightly_cargo(&["weak_namespaced_check"])
        .with_stderr_data(str![[r#"
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn disabled_weak_optional_deps() {
    // Issue #10801
    // A weak dependency of a dependency should not be included in Cargo.lock,
    // at least on lockfile version 5.
    Package::new("bar", "1.0.0")
        .feature("feat", &[])
        .file("src/lib.rs", &require(&["feat"], &[]))
        .publish();
    Package::new("dep", "1.0.0")
        .add_dep(Dependency::new("bar", "1.0").optional(true))
        .feature("feat", &["bar?/feat"])
        .file("src/lib.rs", "")
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
                dep = { version = "1.0", features = ["feat"] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    let lockfile = p.read_lockfile();

    assert!(
        lockfile.contains(r#"version = 4"#),
        "lockfile version is not 4!\n{lockfile}",
    );
    // Previous behavior: bar is inside lockfile.
    assert!(
        lockfile.contains(r#"name = "bar""#),
        "bar not found\n{lockfile}",
    );

    // Update to new lockfile version
    let new_lockfile = lockfile.replace("version = 4", "version = 5");
    p.change_file("Cargo.lock", &new_lockfile);

    // Note how we are not downloading bar here
    p.cargo("check -Znext-lockfile-bump")
        .masquerade_as_nightly_cargo(&["weak_namespaced_check"])
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(
        lockfile.contains(r#"version = 5"#),
        "lockfile version is not 5!\n{lockfile}",
    );
    // New behavior: bar is gone.
    assert!(
        !lockfile.contains(r#"name = "bar""#),
        "bar inside lockfile!\n{lockfile}",
    );

    // Note how we are not downloading bar here
    p.cargo("check -Znext-lockfile-bump --features dep/bar")
        .masquerade_as_nightly_cargo(&["weak_namespaced_check"])
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[CHECKING] dep v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(
        lockfile.contains(r#"version = 5"#),
        "lockfile version is not 5!\n{lockfile}",
    );
    // bar is still not there, even if dep/bar is enabled on the command line.
    // This might be unintuitive, but it matches what happens on lock version 3
    // if there was no optional feat = bar?/feat feature in bar.
    assert!(
        !lockfile.contains(r#"name = "bar""#),
        "bar inside lockfile!\n{lockfile}",
    );
}
