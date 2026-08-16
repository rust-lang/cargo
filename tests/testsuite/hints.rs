//! Tests for hints.

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{Execs, project, str};

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
[WARNING] foo@0.0.1: ignoring unsupported value type (string) for 'hints.mostly-unused', which expects a boolean
[CHECKING] bar v1.0.0
[RUNNING] `rustc --crate-name bar [..]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stderr_does_not_contain("-Zhint-mostly-unused")
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
[WARNING] foo@0.0.1: ignoring 'hints.mostly-unused', pass `-Zprofile-hint-mostly-unused` to enable it
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
    p.cargo("check -Zprofile-hint-mostly-unused -v")
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
    p.cargo("check -Zprofile-hint-mostly-unused -v")
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

fn with_opt_level(cargo: &mut Execs, crate_name: &str, level: &str) {
    let crate_marker = format!("[RUNNING] `rustc --crate-name {crate_name}");
    if level == "0" {
        // Cargo omits `-C opt-level` entirely at level 0.
        cargo.with_stderr_line_without(&[crate_marker], &["-C opt-level".to_owned()]);
    } else {
        cargo.with_stderr_line_without(&[crate_marker, format!("-C opt-level={level}")], &[]);
    }
}

#[cargo_test]
fn min_opt_level_with_numeric_profiles() {
    Package::new("dep", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "1.0.0"
            edition = "2024"

            [hints]
            min-opt-level = 2
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
            edition = "2024"

            [dependencies]
            dep = "1.0"

            [profile.low]
            inherits = "dev"
            opt-level = 1

            [profile.high]
            inherits = "dev"
            opt-level = 3
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut cargo = p.cargo("check -v");
    with_opt_level(&mut cargo, "dep", "0");
    with_opt_level(&mut cargo, "foo", "0");
    cargo.run();

    let mut cargo = p.cargo("check -v --profile low");
    with_opt_level(&mut cargo, "dep", "1");
    with_opt_level(&mut cargo, "foo", "1");
    cargo.run();

    let mut cargo = p.cargo("check -v --profile high");
    with_opt_level(&mut cargo, "dep", "3");
    with_opt_level(&mut cargo, "foo", "3");
    cargo.run();

    let mut cargo = p.cargo("check -v --release");
    with_opt_level(&mut cargo, "dep", "3");
    with_opt_level(&mut cargo, "foo", "3");
    cargo.run();
}

#[cargo_test]
fn min_opt_level_on_root_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2024"

            [hints]
            min-opt-level = 3
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut cargo = p.cargo("check -v");
    with_opt_level(&mut cargo, "foo", "0");
    cargo.run();
}

#[cargo_test]
fn min_opt_level_with_size_profiles() {
    Package::new("dep", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "1.0.0"
            edition = "2024"

            [hints]
            min-opt-level = 2
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
            edition = "2024"

            [dependencies]
            dep = "1.0"

            [profile.small]
            inherits = "dev"
            opt-level = "s"

            [profile.tiny]
            inherits = "dev"
            opt-level = "z"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut cargo = p.cargo("check -v --profile small");
    with_opt_level(&mut cargo, "dep", "s");
    cargo.run();

    let mut cargo = p.cargo("check -v --profile tiny");
    with_opt_level(&mut cargo, "dep", "z");
    cargo.run();
}

#[cargo_test]
fn min_opt_level_with_package_overrides() {
    Package::new("dep", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "1.0.0"
            edition = "2024"

            [hints]
            min-opt-level = 2
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
            edition = "2024"

            [dependencies]
            dep = "1.0"

            [profile.wildcard]
            inherits = "dev"

            [profile.wildcard.package."*"]
            opt-level = 1

            [profile.specific]
            inherits = "dev"

            [profile.specific.package.dep]
            opt-level = 0
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut cargo = p.cargo("check -v --profile wildcard");
    with_opt_level(&mut cargo, "dep", "1");
    cargo.run();

    let mut cargo = p.cargo("check -v --profile specific");
    with_opt_level(&mut cargo, "dep", "0");
    cargo.run();
}

#[cargo_test]
fn min_opt_level_with_transitive_dependency() {
    Package::new("leaf", "1.0.0").publish();
    Package::new("dep", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "1.0.0"
            edition = "2024"

            [dependencies]
            leaf = "1.0"

            [hints]
            min-opt-level = 2
            "#,
        )
        .file("src/lib.rs", "")
        .dep("leaf", "1.0")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2024"

            [dependencies]
            dep = "1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut cargo = p.cargo("check -v");
    with_opt_level(&mut cargo, "dep", "0");
    with_opt_level(&mut cargo, "leaf", "0");
    cargo.run();
}

#[cargo_test]
fn min_opt_level_with_build_dependencies() {
    Package::new("dep", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "1.0.0"
            edition = "2024"

            [hints]
            min-opt-level = 2
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
            edition = "2024"

            [build-dependencies]
            dep = "1.0"

            [profile.overridden]
            inherits = "dev"

            [profile.overridden.build-override]
            opt-level = 0
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut cargo = p.cargo("check -v");
    with_opt_level(&mut cargo, "dep", "0");
    cargo.run();

    let mut cargo = p.cargo("check -v --profile overridden");
    with_opt_level(&mut cargo, "dep", "0");
    cargo.run();
}

#[cargo_test]
fn min_opt_level_with_wrong_type() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2024"

            [hints]
            min-opt-level = "s"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut cargo = p.cargo("check -v");
    with_opt_level(&mut cargo, "foo", "0");
    cargo
        .with_stderr_data(str![[r#"
[WARNING] Cargo.toml: unused manifest key: hints.min-opt-level
[WARNING] `foo` (manifest) generated 1 warning
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn min_opt_level_with_out_of_range_values() {
    for (path, name, level) in [("negative", "negative", "-1"), ("high", "high", "4")] {
        let p = project()
            .at(path)
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "{name}"
                    version = "0.0.1"
                    edition = "2024"

                    [hints]
                    min-opt-level = {level}
                    "#,
                ),
            )
            .file("src/main.rs", "fn main() {}")
            .build();

        let mut cargo = p.cargo("check -v");
        with_opt_level(&mut cargo, name, "0");
        cargo
            .with_stderr_data(str![[r#"
[WARNING] Cargo.toml: unused manifest key: hints.min-opt-level
[WARNING] [..] (manifest) generated 1 warning
[CHECKING] [..] v0.0.1 ([ROOT]/[..])
[RUNNING] `rustc --crate-name [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
            .run();
    }
}

#[cargo_test]
fn min_opt_level_registry_dependency_warnings_are_suppressed() {
    for (path, name, level, expected_opt_level) in [
        ("positive", "positive", "2", "0"),
        ("wrong-type", "wrong_type", r#""s""#, "0"),
        ("out-of-range", "out_of_range", "4", "0"),
    ] {
        Package::new(name, "1.0.0")
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "{name}"
                    version = "1.0.0"
                    edition = "2024"

                    [hints]
                    min-opt-level = {level}
                    "#,
                ),
            )
            .file("src/lib.rs", "")
            .publish();
        let p = project()
            .at(path)
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2024"

                    [dependencies]
                    {name} = "1.0"
                    "#,
                ),
            )
            .file("src/main.rs", "fn main() {}")
            .build();

        let mut cargo = p.cargo("check -v");
        with_opt_level(&mut cargo, name, expected_opt_level);
        cargo
            .with_stderr_does_not_contain("[WARNING] [..]hints.min-opt-level[..]")
            .run();
    }
}

#[cargo_test]
fn min_opt_level_without_feature_gate() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2024"

            [hints]
            min-opt-level = 2
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut cargo = p.cargo("check -v");
    with_opt_level(&mut cargo, "foo", "0");
    cargo
        .with_stderr_data(str![[r#"
[WARNING] Cargo.toml: unused manifest key: hints.min-opt-level
[WARNING] `foo` (manifest) generated 1 warning
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] Cargo.toml: unused manifest key: hints.min-opt-level
[WARNING] `foo` (manifest) generated 1 warning
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn min_opt_level_warning_is_emitted_once_per_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2024"

            [hints]
            min-opt-level = 2
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] Cargo.toml: unused manifest key: hints.min-opt-level
[WARNING] `foo` (manifest) generated 1 warning
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn min_opt_level_on_local_dependencies_without_feature_gate() {
    // Keep the local hinting packages in a dependency chain so their output
    // order is stable. They must remain path dependencies because hint gate
    // warnings are suppressed for non-local units, which would make the
    // silent-zero case vacuous.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2024"

            [dependencies]
            bar = { path = "bar" }
            "#,
        )
        .file("src/main.rs", "fn main() { bar::bar(); }")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "1.0.0"
            edition = "2024"

            [dependencies]
            zero = { path = "../zero" }

            [hints]
            min-opt-level = 3
            "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file(
            "zero/Cargo.toml",
            r#"
            [package]
            name = "zero"
            version = "1.0.0"
            edition = "2024"

            [hints]
            min-opt-level = 0
            "#,
        )
        .file("zero/src/lib.rs", "")
        .build();

    let mut cargo = p.cargo("check -v");
    with_opt_level(&mut cargo, "bar", "0");
    with_opt_level(&mut cargo, "zero", "0");
    cargo
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to highest Rust [..] compatible versions
[CHECKING] zero v1.0.0 ([ROOT]/foo/zero)
[RUNNING] `rustc --crate-name zero [..]`
[CHECKING] bar v1.0.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar [..]`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
