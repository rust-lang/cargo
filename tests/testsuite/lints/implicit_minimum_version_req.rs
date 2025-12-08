//! Tests for the `implicit_minimum_version_req` lint.

use crate::prelude::*;

use cargo_test_support::basic_manifest;
use cargo_test_support::git;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

#[cargo_test]
fn major_only() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "1"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn major_minor() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "1.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1.0"
  |       ^^^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |           ++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fully_specified_should_not_warn() {
    Package::new("dep", "1.2.3").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "1.0.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.2.3 (registry `dummy-registry`)
[CHECKING] dep v1.2.3
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn detailed_dep_major_only() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = { version = "1" }

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:19
  |
7 | dep = { version = "1" }
  |                   ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = { version = "1.0.0" }
  |                     ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn greater_eq() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = ">=1.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = ">=1.0"
  |       ^^^^^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = ">=1.0.0"
  |             ++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn less_should_not_warn() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "<2.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn wildcard_should_not_warn() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "1.*"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn wildcard_minor_should_not_warn() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "1.0.*"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn greater_should_not_warn() {
    Package::new("dep", "1.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = ">1.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.1.0 (registry `dummy-registry`)
[CHECKING] dep v1.1.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn less_eq_should_not_warn() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "<=2.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn multiple_requirements() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = ">=1.0, <2.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = ">=1.0, <2.0"
  |       ^^^^^^^^^^^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = ">=1.0.0, <2.0"
  |             ++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn tilde_requirement_should_not_warn() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "~1.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn exact_requirement_should_not_warn() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "=1"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn path_dep_should_not_warn() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
bar = { path = "bar" }

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.1.0"
edition = "2021"
"#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn path_dep_with_registry_version() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
bar = { path = "bar", version = "0.1" }

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.1.0"
edition = "2021"
"#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:33
  |
7 | bar = { path = "bar", version = "0.1" }
  |                                 ^^^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | bar = { path = "bar", version = "0.1.0" }
  |                                     ++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn git_dep_should_not_warn() {
    let git_project = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
bar = {{ git = '{}' }}

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn git_dep_with_registry_version() {
    let git_project = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
bar = {{ git = '{}', version = "0.1" }}

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:[..]
  |
7 | bar = { git = '[ROOTURL]/bar', version = "0.1" }
  |                                      [..]^^^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | bar = { git = '[ROOTURL]/bar', version = "0.1.0" }
  |                                          [..]++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn dev_dep() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dev-dependencies]
dep = "1"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_dep() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[build-dependencies]
dep = "1.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1.0"
  |       ^^^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |           ++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[COMPILING] dep v1.0.0
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn target_dep() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

# Spaces are critical here to check Cargo tolerates them
[target.'cfg(      all(  )   )'.dependencies]
dep = "1"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:8:7
  |
8 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
8 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn target_dev_dep() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

# Spaces are critical here to check Cargo tolerates them
[target.'cfg(      all(  )   )'.dev-dependencies]
dep = "1"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:8:7
  |
8 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
8 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn multiple_implicit_deps() {
    Package::new("dep", "1.0.0").publish();
    Package::new("regex", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "1"
regex = "1.0"

[lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:8:9
  |
8 | regex = "1.0"
  |         ^^^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
8 | regex = "1.0.0"
  |             ++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] regex v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] regex v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn workspace_inherited() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["member"]
resolver = "2"

[workspace.dependencies]
dep = "1"

[workspace.lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file(
            "member/Cargo.toml",
            r#"
[package]
name = "member"
edition = "2021"

[dependencies]
dep.workspace = true

[lints]
workspace = true
"#,
        )
        .file("member/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] member v0.0.0 ([ROOT]/foo/member)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_workspace_dep() {
    // Should still warn for workspace dep
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["member"]
resolver = "2"

[workspace.dependencies]
dep = "1"

[workspace.lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file(
            "member/Cargo.toml",
            r#"
[package]
name = "member"
edition = "2021"
"#,
        )
        .file("member/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[CHECKING] member v0.0.0 ([ROOT]/foo/member)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_workspace_dep_and_package_implicit_req() {
    // Should warn package and workspace separately
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["member"]
resolver = "2"

[workspace.dependencies]
dep = "1"

[workspace.lints.cargo]
implicit_minimum_version_req = "warn"
"#,
        )
        .file(
            "member/Cargo.toml",
            r#"
[package]
name = "member"
edition = "2021"

[dependencies]
dep = "1.0"

[lints]
workspace = true
"#,
        )
        .file("member/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[WARNING] dependency version requirement without an explicit minimum version
 --> member/Cargo.toml:7:7
  |
7 | dep = "1.0"
  |       ^^^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |           ++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `warn` in `[lints]`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[CHECKING] member v0.0.0 ([ROOT]/foo/member)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn deny() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"

[dependencies]
dep = "1"

[lints.cargo]
implicit_minimum_version_req = "deny"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] dependency version requirement without an explicit minimum version
 --> Cargo.toml:7:7
  |
7 | dep = "1"
  |       ^^^ missing full version components
  |
[HELP] consider specifying full `major.minor.patch` version components
  |
7 | dep = "1.0.0"
  |         ++++
  = [NOTE] `cargo::implicit_minimum_version_req` is set to `deny` in `[lints]`
[ERROR] encountered 1 error while running lints

"#]])
        .run();
}
