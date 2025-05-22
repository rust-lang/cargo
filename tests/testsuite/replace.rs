//! Tests for `[replace]` table source replacement.

use cargo_test_support::git;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, project, str};

#[cargo_test]
fn override_simple() {
    Package::new("bar", "0.1.0").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}' }}
                "#,
                bar.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v0.1.0 ([ROOTURL]/override#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn override_with_features() {
    Package::new("bar", "0.1.0").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}', features = ["some_feature"] }}
                "#,
                bar.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 2 packages to latest compatible versions
[WARNING] replacement for `bar` uses the features mechanism. default-features and features will not take effect because the replacement dependency does not support this mechanism
[CHECKING] bar v0.1.0 ([ROOTURL]/override#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn override_with_setting_default_features() {
    Package::new("bar", "0.1.0").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}', default-features = false, features = ["none_default_feature"] }}
                "#,
                bar.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 2 packages to latest compatible versions
[WARNING] replacement for `bar` uses the features mechanism. default-features and features will not take effect because the replacement dependency does not support this mechanism
[CHECKING] bar v0.1.0 ([ROOTURL]/override#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn missing_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"

                [replace]
                bar = { git = 'https://example.com' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").with_status(101).with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  replacements must specify a version to replace, but `https://github.com/rust-lang/crates.io-index#bar` does not

"#]]).run();
}

#[cargo_test]
fn invalid_semver_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"

                [replace]
                "bar:*" = { git = 'https://example.com' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  replacements must specify a valid semver version to replace, but `bar:*` does not
...
"#]])
        .run();
}

#[cargo_test]
fn different_version() {
    Package::new("bar", "0.2.0").publish();
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"

                [replace]
                "bar:0.1.0" = "0.2.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").with_status(101).with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  replacements cannot specify a version requirement, but found one for `https://github.com/rust-lang/crates.io-index#bar@0.1.0`

"#]]).run();
}

#[cargo_test]
fn transitive() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.2.0")
        .dep("bar", "0.1.0")
        .file("src/lib.rs", "extern crate bar; fn baz() { bar::bar(); }")
        .publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    baz = "0.2.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}' }}
                "#,
                foo.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.2.0 (registry `dummy-registry`)
[CHECKING] bar v0.1.0 ([ROOTURL]/override#[..])
[CHECKING] baz v0.2.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn persists_across_rebuilds() {
    Package::new("bar", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}' }}
                "#,
                foo.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v0.1.0 ([ROOTURL]/override#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn replace_registry_with_path() {
    Package::new("bar", "0.1.0").publish();

    let _ = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"

                [replace]
                "bar:0.1.0" = { path = "../bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v0.1.0 ([ROOT]/bar)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn use_a_spec_to_select() {
    Package::new("baz", "0.1.1")
        .file("src/lib.rs", "pub fn baz1() {}")
        .publish();
    Package::new("baz", "0.2.0").publish();
    Package::new("bar", "0.1.1")
        .dep("baz", "0.2")
        .file(
            "src/lib.rs",
            "extern crate baz; pub fn bar() { baz::baz3(); }",
        )
        .publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("baz", "0.2.0"))
        .file("src/lib.rs", "pub fn baz3() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1"
                    baz = "0.1"

                    [replace]
                    "baz:0.2.0" = {{ git = '{}' }}
                "#,
                foo.url()
            ),
        )
        .file(
            "src/lib.rs",
            "
            extern crate bar;
            extern crate baz;

            pub fn local() {
                baz::baz1();
                bar::bar();
            }
        ",
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 4 packages to latest compatible versions
[ADDING] baz v0.1.1 (available: v0.2.0)
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] bar v0.1.1 (registry `dummy-registry`)
[CHECKING] baz v0.2.0 ([ROOTURL]/override#[..])
[CHECKING] baz v0.1.1
[CHECKING] bar v0.1.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn override_adds_some_deps() {
    Package::new("baz", "0.1.1").publish();
    Package::new("bar", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                baz = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}' }}
                "#,
                foo.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.1 (registry `dummy-registry`)
[CHECKING] baz v0.1.1
[CHECKING] bar v0.1.0 ([ROOTURL]/override#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    Package::new("baz", "0.1.2").publish();
    p.cargo("update")
        .arg(&format!("{}#bar", foo.url()))
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/override`
[UPDATING] `dummy-registry` index
[LOCKING] 0 packages to latest compatible versions
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]])
        .run();
    p.cargo("update  https://github.com/rust-lang/crates.io-index#bar")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 0 packages to latest compatible versions
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn locked_means_locked_yes_no_seriously_i_mean_locked() {
    // this in theory exercises #2041
    Package::new("baz", "0.1.0").publish();
    Package::new("baz", "0.2.0").publish();
    Package::new("bar", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                baz = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1"
                    baz = "0.1"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}' }}
                "#,
                foo.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn override_wrong_name() {
    Package::new("baz", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    baz = "0.1"

                    [replace]
                    "baz:0.1.0" = {{ git = '{}' }}
                "#,
                foo.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[ERROR] failed to get `baz` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  no matching package for override `https://github.com/rust-lang/crates.io-index#baz@0.1.0` found
  location searched: [ROOTURL]/override
  version required: =0.1.0

"#]])
        .run();
}

#[cargo_test]
fn override_with_nothing() {
    Package::new("bar", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}' }}
                "#,
                foo.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update [ROOTURL]/override

Caused by:
  Could not find Cargo.toml in `[ROOT]/home/.cargo/git/checkouts/override-[HASH]/[..]`

"#]])
        .run();
}

#[cargo_test]
fn override_wrong_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [replace]
                "bar:0.1.0" = { git = 'https://example.com', version = '0.2.0' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").with_status(101).with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  replacements cannot specify a version requirement, but found one for `https://github.com/rust-lang/crates.io-index#bar@0.1.0`

"#]]).run();
}

#[cargo_test]
fn multiple_specs() {
    Package::new("bar", "0.1.0").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{0}' }}

                    [replace."https://github.com/rust-lang/crates.io-index#bar:0.1.0"]
                    git = '{0}'
                "#,
                bar.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  overlapping replacement specifications found:

    * https://github.com/rust-lang/crates.io-index#bar@0.1.0
    * https://github.com/rust-lang/crates.io-index#bar@0.1.0

  both specifications match: bar v0.1.0

"#]])
        .run();
}

#[cargo_test]
fn test_override_dep() {
    Package::new("bar", "0.1.0").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{0}' }}
                "#,
                bar.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("test -p bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 2 packages to latest compatible versions
[ERROR] There are multiple `bar` packages in your project, and the specification `bar` is ambiguous.
Please re-run this command with one of the following specifications:
  registry+https://github.com/rust-lang/crates.io-index#bar@0.1.0
  git+[ROOTURL]/override#bar@0.1.0

"#]])
        .run();
}

#[cargo_test]
fn update() {
    Package::new("bar", "0.1.0").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{0}' }}
                "#,
                bar.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("update")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 0 packages to latest compatible versions

"#]])
        .run();
}

// foo -> near -> far
// near is overridden with itself
#[cargo_test]
fn no_override_self() {
    let deps = git::repo(&paths::root().join("override"))
        .file("far/Cargo.toml", &basic_manifest("far", "0.1.0"))
        .file("far/src/lib.rs", "")
        .file(
            "near/Cargo.toml",
            r#"
                [package]
                name = "near"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                far = { path = "../far" }
            "#,
        )
        .file("near/src/lib.rs", "#![no_std] pub extern crate far;")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    near = {{ git = '{0}' }}

                    [replace]
                    "near:0.1.0" = {{ git = '{0}' }}
                "#,
                deps.url()
            ),
        )
        .file("src/lib.rs", "#![no_std] pub extern crate near;")
        .build();

    p.cargo("check --verbose").run();
}

#[cargo_test]
fn override_an_override() {
    Package::new("chrono", "0.2.0")
        .dep("serde", "< 0.9")
        .publish();
    Package::new("serde", "0.7.0")
        .file("src/lib.rs", "pub fn serde07() {}")
        .publish();
    Package::new("serde", "0.8.0")
        .file("src/lib.rs", "pub fn serde08() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                chrono = "0.2"
                serde = "0.8"

                [replace]
                "chrono:0.2.0" = { path = "chrono" }
                "serde:0.8.0" = { path = "serde" }
            "#,
        )
        .file(
            "Cargo.lock",
            r#"
                [[package]]
                name = "foo"
                version = "0.0.1"
                dependencies = [
                 "chrono 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
                 "serde 0.8.0 (registry+https://github.com/rust-lang/crates.io-index)",
                ]

                [[package]]
                name = "chrono"
                version = "0.2.0"
                source = "registry+https://github.com/rust-lang/crates.io-index"
                replace = "chrono 0.2.0"

                [[package]]
                name = "chrono"
                version = "0.2.0"
                dependencies = [
                 "serde 0.7.0 (registry+https://github.com/rust-lang/crates.io-index)",
                ]

                [[package]]
                name = "serde"
                version = "0.7.0"
                source = "registry+https://github.com/rust-lang/crates.io-index"

                [[package]]
                name = "serde"
                version = "0.8.0"
                source = "registry+https://github.com/rust-lang/crates.io-index"
                replace = "serde 0.8.0"

                [[package]]
                name = "serde"
                version = "0.8.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate chrono;
            extern crate serde;

            pub fn foo() {
                chrono::chrono();
                serde::serde08_override();
            }
        ",
        )
        .file(
            "chrono/Cargo.toml",
            r#"
                [package]
                name = "chrono"
                version = "0.2.0"
                edition = "2015"
                authors = []

                [dependencies]
                serde = "< 0.9"
            "#,
        )
        .file(
            "chrono/src/lib.rs",
            "
            extern crate serde;
            pub fn chrono() {
                serde::serde07();
            }
        ",
        )
        .file("serde/Cargo.toml", &basic_manifest("serde", "0.8.0"))
        .file("serde/src/lib.rs", "pub fn serde08_override() {}")
        .build();

    p.cargo("check -v").run();
}

#[cargo_test]
fn overriding_nonexistent_no_spurious() {
    Package::new("bar", "0.1.0").dep("baz", "0.1").publish();
    Package::new("baz", "0.1.0").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{url}' }}
                    "baz:0.1.0" = {{ git = '{url}' }}
                "#,
                url = bar.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] package replacement is not used: https://github.com/rust-lang/crates.io-index#baz@0.1.0
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
        .run();
}

#[cargo_test]
fn no_warnings_when_replace_is_used_in_another_workspace_member() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = [ "first_crate", "second_crate"]

            [replace]
            "bar:0.1.0" = { path = "local_bar" }
            "#,
        )
        .file(
            "first_crate/Cargo.toml",
            r#"
                [package]
                name = "first_crate"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("first_crate/src/lib.rs", "")
        .file(
            "second_crate/Cargo.toml",
            &basic_manifest("second_crate", "0.1.0"),
        )
        .file("second_crate/src/lib.rs", "")
        .file("local_bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("local_bar/src/lib.rs", "")
        .build();

    p.cargo("check")
        .cwd("first_crate")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v0.1.0 ([ROOT]/foo/local_bar)
[CHECKING] first_crate v0.1.0 ([ROOT]/foo/first_crate)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .cwd("second_crate")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[CHECKING] second_crate v0.1.0 ([ROOT]/foo/second_crate)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn replace_to_path_dep() {
    Package::new("bar", "0.1.0").dep("baz", "0.1").publish();
    Package::new("baz", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1.0"

                [replace]
                "bar:0.1.0" = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file(
            "bar/src/lib.rs",
            "extern crate baz; pub fn bar() { baz::baz(); }",
        )
        .file("bar/baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("bar/baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn override_with_default_feature() {
    Package::new("another", "0.1.0").publish();
    Package::new("another", "0.1.1").dep("bar", "0.1").publish();
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = { path = "bar", default-features = false }
                another = "0.1"
                another2 = { path = "another2" }

                [replace]
                'bar:0.1.0' = { path = "bar" }
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() { bar::bar(); }")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [features]
                default = []
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                #[cfg(feature = "default")]
                pub fn bar() {}
            "#,
        )
        .file(
            "another2/Cargo.toml",
            r#"
                [package]
                name = "another2"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = { version = "0.1", default-features = false }
            "#,
        )
        .file("another2/src/lib.rs", "")
        .build();

    p.cargo("run").run();
}

#[cargo_test]
fn override_plus_dep() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1"

                [replace]
                'bar:0.1.0' = { path = "bar" }
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
                authors = []

                [dependencies]
                foo = { path = ".." }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] cyclic package dependency: package `bar v0.1.0 ([ROOT]/foo/bar)` depends on itself. Cycle:
package `bar v0.1.0 ([ROOT]/foo/bar)`
    ... which satisfies dependency `bar = "^0.1"` of package `foo v0.0.1 ([ROOT]/foo)`
    ... which satisfies path dependency `foo` of package `bar v0.1.0 ([ROOT]/foo/bar)`

"#]])
        .run();
}

#[cargo_test]
fn override_generic_matching_other_versions() {
    Package::new("bar", "0.1.0+a").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}' }}
                "#,
                bar.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  replacement specification `https://github.com/rust-lang/crates.io-index#bar@0.1.0` matched 0.1.0+a and tried to override it with 0.1.0
  avoid matching unrelated packages by being more specific

"#]]).with_status(101).run();
}

#[cargo_test]
fn override_respects_spec_metadata() {
    Package::new("bar", "0.1.0+a").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0+a"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0+notTheBuild" = {{ git = '{}' }}
                "#,
                bar.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[WARNING] package replacement is not used: https://github.com/rust-lang/crates.io-index#bar@0.1.0+notTheBuild
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0+a (registry `dummy-registry`)
[CHECKING] bar v0.1.0+a
[CHECKING] foo v0.0.1 ([ROOT]/foo)
error[E0425]: cannot find function `bar`[..]
...
[ERROR] could not compile `foo` (lib) due to 1 previous error

"#]]).with_status(101).run();
}

#[cargo_test]
fn override_spec_metadata_is_optional() {
    Package::new("bar", "0.1.0+a").publish();

    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0+a"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    bar = "0.1.0"

                    [replace]
                    "bar:0.1.0" = {{ git = '{}' }}
                "#,
                bar.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/override`
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v0.1.0+a ([ROOTURL]/override#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
