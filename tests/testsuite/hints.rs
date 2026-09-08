//! Tests for hints.

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{project, str};

#[cargo_test]
fn empty_hints_no_warn() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [hints]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unknown_hints_warn() {
    Package::new("bar", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "1.0.0"
            edition = "2015"

            [hints]
            this-is-an-unknown-hint = true

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [dependencies]
            bar = "1.0"

            [hints]
            this-is-an-unknown-hint = true

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] Cargo.toml: unused manifest key: hints.this-is-an-unknown-hint
[WARNING] `foo` (manifest) generated 1 warning
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[RUNNING] `rustc --crate-name bar [..]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn hint_unknown_type_warn() {
    Package::new("bar", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "1.0.0"
            edition = "2015"

            [hints]
            mostly-unused = 1

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [dependencies]
            bar = "1.0"

            [hints]
            mostly-unused = "string"

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[WARNING] ignoring unsupported value type (string) for `hints.mostly-unused`
  --> Cargo.toml:11:29
   |
11 |             mostly-unused = "string"
   |                             ^^^^^^^^ expected a boolean
[CHECKING] bar v1.0.0
[RUNNING] `rustc --crate-name bar [..]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stderr_does_not_contain("-Zhint-mostly-unused")
        .run();

    p.cargo("check -vv")
        .with_stderr_data(str![[r#"
[WARNING] ignoring unsupported value type (integer) for `hints.mostly-unused`
 --> [ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.0/Cargo.toml:8:29
  |
8 |             mostly-unused = 1
  |                             ^ expected a boolean
[WARNING] ignoring unsupported value type (string) for `hints.mostly-unused`
  --> Cargo.toml:11:29
   |
11 |             mostly-unused = "string"
   |                             ^^^^^^^^ expected a boolean
[FRESH] bar v1.0.0
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn hints_mostly_unused_warn_without_gate() {
    Package::new("bar", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "1.0.0"
            edition = "2015"

            [hints]
            mostly-unused = true

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [dependencies]
            bar = "1.0"

            [hints]
            mostly-unused = true

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[WARNING] ignoring `hints.mostly-unused`
  --> Cargo.toml:11:13
   |
11 |             mostly-unused = true
   |             ^^^^^^^^^^^^^^^^^^^^
   |
   = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[CHECKING] bar v1.0.0
[RUNNING] `rustc --crate-name bar [..]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stderr_does_not_contain("-Zhint-mostly-unused")
        .run();

    p.cargo("check -vv")
        .with_stderr_data(str![[r#"
[WARNING] ignoring `hints.mostly-unused`
 --> [ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.0/Cargo.toml:8:13
  |
8 |             mostly-unused = true
  |             ^^^^^^^^^^^^^^^^^^^^
  |
  = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[WARNING] ignoring `hints.mostly-unused`
  --> Cargo.toml:11:13
   |
11 |             mostly-unused = true
   |             ^^^^^^^^^^^^^^^^^^^^
   |
   = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[FRESH] bar v1.0.0
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn mostly_unused_warns_for_each_source_with_multiple_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [hints]
            mostly-unused = true

            [profile.dev.build-override]
            hint-mostly-unused = true
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo; fn main() {}")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] ignoring `hints.mostly-unused`
 --> Cargo.toml:8:13
  |
8 |             mostly-unused = true
  |             ^^^^^^^^^^^^^^^^^^^^
  |
  = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[WARNING] ignoring `hint-mostly-unused` profile option for `foo@0.0.1`
  |
  = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] ignoring `hints.mostly-unused`
 --> Cargo.toml:8:13
  |
8 |             mostly-unused = true
  |             ^^^^^^^^^^^^^^^^^^^^
  |
  = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[WARNING] ignoring `hint-mostly-unused` profile option for `foo@0.0.1`
  |
  = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn mostly_unused_invalid_value_and_profile_warn_once_per_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [hints]
            mostly-unused = "string"

            [profile.dev]
            hint-mostly-unused = true
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo; fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] ignoring `hint-mostly-unused` profile option for `foo@0.0.1`
  |
  = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[WARNING] ignoring unsupported value type (string) for `hints.mostly-unused`
 --> Cargo.toml:8:29
  |
8 |             mostly-unused = "string"
  |                             ^^^^^^^^ expected a boolean
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn mostly_unused_package_hint_warns_with_mixed_profiles() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [dependencies]
            bar = { path = "bar" }

            [build-dependencies]
            bar = { path = "bar" }

            [profile.dev.build-override]
            hint-mostly-unused = false
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .file("build.rs", "extern crate bar; fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            edition = "2015"

            [hints]
            mostly-unused = true
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[WARNING] ignoring `hints.mostly-unused`
 --> bar/Cargo.toml:8:13
  |
8 |             mostly-unused = true
  |             ^^^^^^^^^^^^^^^^^^^^
  |
  = [HELP] pass `-Zprofile-hint-mostly-unused` to enable it
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zhint-mostly-unused is unstable")]
fn hints_mostly_unused_nightly() {
    Package::new("bar", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "1.0.0"
            edition = "2015"

            [hints]
            mostly-unused = true

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [dependencies]
            bar = "1.0"

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .arg("-Zprofile-hint-mostly-unused")
        .masquerade_as_nightly_cargo(&["profile-hint-mostly-unused"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[RUNNING] `rustc --crate-name bar [..] -Zhint-mostly-unused [..]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo [..] -Zhint-mostly-unused [..]",
        )
        .run();
}

#[cargo_test(nightly, reason = "-Zhint-mostly-unused is unstable")]
fn mostly_unused_profile_overrides_hints_nightly() {
    Package::new("bar", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "1.0.0"
            edition = "2015"

            [hints]
            mostly-unused = true

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [dependencies]
            bar = "1.0"

            [profile.dev.package.bar]
            hint-mostly-unused = false

            [lints.cargo]
            default = "allow"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .arg("-Zprofile-hint-mostly-unused")
        .masquerade_as_nightly_cargo(&["profile-hint-mostly-unused"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[RUNNING] `rustc --crate-name bar [..]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stderr_does_not_contain("-Zhint-mostly-unused")
        .run();
}

#[cargo_test(nightly, reason = "-Zhint-mostly-unused is unstable")]
fn mostly_unused_profile_overrides_hints_on_self_nightly() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [hints]
            mostly-unused = true

            [profile.dev]
            hint-mostly-unused = false
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stderr_does_not_contain("-Zhint-mostly-unused")
        .run();
}
