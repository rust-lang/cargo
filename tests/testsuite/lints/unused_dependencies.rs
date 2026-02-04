use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::rustc_host;
use cargo_test_support::str;

#[cargo_test]
fn unused_dep_normal() {
    // The most basic case where there is an unused dependency
    Package::new("unused", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            unused = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:12:13
   |
12 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[CHECKING] unused v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_dep_build() {
    Package::new("unused", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [build-dependencies]
            unused = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "build.rs",
            r#"
            fn main() {}
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:12:13
   |
12 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[COMPILING] unused v0.1.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_dep_build_no_build_rs() {
    Package::new("unused", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [build-dependencies]
            unused = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:12:13
   |
12 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_dep_lib_bins() {
    // Make sure that dependency uses by both binaries and libraries
    // are being registered as used
    Package::new("unused", "0.1.0").publish();
    Package::new("lib_used", "0.1.0").publish();
    Package::new("bins_used", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            unused = "0.1.0"
            lib_used = "0.1.0"
            bins_used = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            use lib_used as _;
            "#,
        )
        .file(
            "src/bin/foo.rs",
            r#"
            use bins_used as _;
            fn main() {}
            "#,
        )
        .file(
            "src/bin/bar.rs",
            r#"
            use bins_used as _;
            fn main() {}
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:14:13
   |
14 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] lib_used v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] bins_used v0.1.0 (registry `dummy-registry`)
[CHECKING] bins_used v0.1.0
[CHECKING] unused v0.1.0
[CHECKING] lib_used v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    p.cargo("check -Zcargo-lints --lib")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:14:13
   |
14 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -Zcargo-lints --bins")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:14:13
   |
14 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -Zcargo-lints --bin foo")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:14:13
   |
14 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_dep_build_with_used_dep_normal() {
    // Check sharing of a dependency
    // between build and proper deps
    Package::new("unused_build", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [build-dependencies]
            unused_build = "0.1.0"

            [dependencies]
            unused_build = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "build.rs",
            r#"
            fn main() {}
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            use unused_build as _;
            fn main() {}
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:15:13
   |
15 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] unused_build v0.1.0 (registry `dummy-registry`)
[COMPILING] unused_build v0.1.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_dep_normal_but_implicit_used_dep_dev() {
    Package::new("used_dev", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            used_dev = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .file(
            "tests/foo.rs",
            r#"
            #[test]
            fn foo {
                use used_dev as _;
            }
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:12:13
   |
12 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] used_dev v0.1.0 (registry `dummy-registry`)
[CHECKING] used_dev v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_dep_normal_but_explicit_used_dep_dev() {
    Package::new("used_once", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            used_once = "0.1.0"

            [dev-dependencies]
            used_once = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .file(
            "tests/foo.rs",
            r#"
            #[test]
            fn foo {
                use used_once as _;
            }
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:15:13
   |
15 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] used_once v0.1.0 (registry `dummy-registry`)
[CHECKING] used_once v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_dep_dev_but_explicit_used_dep_normal() {
    Package::new("used_once", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            used_once = "0.1.0"

            [dev-dependencies]
            used_once = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                use used_once as _;
            }
            "#,
        )
        .file(
            "tests/foo.rs",
            r#"
            #[test]
            fn foo {
            }
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:15:13
   |
15 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] used_once v0.1.0 (registry `dummy-registry`)
[CHECKING] used_once v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn optional_dependency() {
    // The most basic case where there is an unused dependency
    Package::new("unused", "0.1.0").publish();
    Package::new("used", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            unused = { version = "0.1.0", optional = true }
            used = { version = "0.1.0", optional = true }

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #[cfg(feature = "used")]
            use used as _;

            fn main() {}
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:13:13
   |
13 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -Zcargo-lints -F used,unused")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:13:13
   |
13 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[DOWNLOADING] crates ...
[DOWNLOADED] used v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[CHECKING] unused v0.1.0
[CHECKING] used v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn unused_dep_renamed() {
    // Make sure that package renaming works
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.2.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            baz = { package = "bar", version = "0.1.0" }
            bar = { package = "baz", version = "0.2.0" }

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            use bar as _;
            fn main() {}
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:13:13
   |
13 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.2.0 (registry `dummy-registry`)
[DOWNLOADED] bar v0.1.0 (registry `dummy-registry`)
[CHECKING] baz v0.2.0
[CHECKING] bar v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn warning_replay() {
    // The most basic case where there is an unused dependency
    Package::new("unused", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            unused = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:12:13
   |
12 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[CHECKING] unused v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:12:13
   |
12 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unused_dep_target() {
    // The most basic case where there is an unused dependency
    Package::new("unused", "0.1.0").publish();
    Package::new("used", "0.1.0").publish();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [target.{host}.dependencies]
            unused = "0.1.0"
            used = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#
            ),
        )
        .file(
            "src/main.rs",
            r#"
            use used as _;
            fn main() {}
            "#,
        )
        .build();

    p.cargo(&format!("check -Zcargo-lints --target {host}"))
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:13:13
   |
13 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] used v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[CHECKING] unused v0.1.0
[CHECKING] used v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn unused_dev_deps() {
    // Test for unused dev dependencies
    Package::new("unit_used", "0.1.0").publish();
    Package::new("doctest_used", "0.1.0").publish();
    Package::new("test_used", "0.1.0").publish();
    Package::new("example_used", "0.1.0").publish();
    Package::new("bench_used", "0.1.0").publish();
    Package::new("unused", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dev-dependencies]
            unit_used = "0.1.0"
            doctest_used = "0.1.0"
            test_used = "0.1.0"
            example_used = "0.1.0"
            bench_used = "0.1.0"
            unused = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            /// ```
            /// use doctest_used as _;
            /// ```
            pub fn foo() {}

            #[test]
            fn test() {
                use unit_used as _;
            }
        "#,
        )
        .file(
            "tests/hello.rs",
            r#"
            use test_used as _;
            "#,
        )
        .file(
            "examples/hello.rs",
            r#"
            use example_used as _;
            fn main() {}
            "#,
        )
        .file(
            "benches/hello.rs",
            r#"
            use bench_used as _;
            fn main() {}
            "#,
        )
        .build();

    // doesn't check any tests, still no unused dev dep warnings
    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:17:13
   |
17 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[LOCKING] 6 packages to latest compatible versions
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // doesn't check doctests, still no unused dev dep warnings
    p.cargo("check -Zcargo-lints --all-targets")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:17:13
   |
17 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[DOWNLOADING] crates ...
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] unit_used v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] test_used v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] example_used v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] doctest_used v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] bench_used v0.1.0 (registry `dummy-registry`)
[CHECKING] bench_used v0.1.0
[CHECKING] unused v0.1.0
[CHECKING] doctest_used v0.1.0
[CHECKING] example_used v0.1.0
[CHECKING] unit_used v0.1.0
[CHECKING] test_used v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    // doesn't test doctests and benches and thus doesn't create unused dev dep warnings
    p.cargo("test -Zcargo-lints --no-run")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:17:13
   |
17 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[COMPILING] example_used v0.1.0
[COMPILING] unit_used v0.1.0
[COMPILING] unused v0.1.0
[COMPILING] test_used v0.1.0
[COMPILING] bench_used v0.1.0
[COMPILING] doctest_used v0.1.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[EXECUTABLE] tests/hello.rs (target/debug/deps/hello-[HASH][EXE])

"#]]
            .unordered(),
        )
        .run();

    // doesn't test doctests, still no unused dev dep warnings
    p.cargo("test -Zcargo-lints --no-run --all-targets")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:17:13
   |
17 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[EXECUTABLE] tests/hello.rs (target/debug/deps/hello-[HASH][EXE])
[EXECUTABLE] benches/hello.rs (target/debug/deps/hello-[HASH][EXE])
[EXECUTABLE] unittests examples/hello.rs (target/debug/examples/hello-[HASH][EXE])

"#]])
        .run();

    // tests everything including doctests, but not
    // the benches
    p.cargo("test -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> Cargo.toml:17:13
   |
17 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] tests/hello.rs (target/debug/deps/hello-[HASH][EXE])
[DOCTEST] foo

"#]])
        .run();
}

#[cargo_test]
fn package_selection() {
    // Make sure that workspaces are supported,
    // --all params, -p params, etc.
    Package::new("used_bar", "0.1.0").publish();
    Package::new("used_foo", "0.1.0").publish();
    Package::new("used_external", "0.1.0").publish();
    Package::new("unused", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["foo", "bar"]
            exclude = ["external"]
        "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            unused = "0.1.0"
            used_foo = "0.1.0"
            bar.path = "../bar"
            external.path = "../external"

            [lints.cargo]
            unused_dependencies = "warn"
            "#,
        )
        .file(
            "foo/src/lib.rs",
            r#"
            use used_foo as _;
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            unused = "0.1.0"
            used_bar = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
            use used_bar as _;
            "#,
        )
        .file(
            "external/Cargo.toml",
            r#"
            [package]
            name = "external"
            version = "0.1.0"
            authors = []
            edition = "2018"

            [dependencies]
            unused = "0.1.0"
            used_external = "0.1.0"

            [lints.cargo]
            unused_dependencies = "warn"
            "#,
        )
        .file(
            "external/src/lib.rs",
            r#"
            use used_external as _;
            "#,
        )
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> bar/Cargo.toml:13:13
   |
13 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[WARNING] unknown lint: `unused_dependencies`
   |
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
  --> foo/Cargo.toml:15:13
15 |             unused_dependencies = "warn"
[LOCKING] 5 packages to latest compatible versions
[DOWNLOADED] used_foo v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] used_external v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] used_bar v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] unused v0.1.0 (registry `dummy-registry`)
[CHECKING] unused v0.1.0
[CHECKING] used_bar v0.1.0
[CHECKING] used_external v0.1.0
[CHECKING] used_foo v0.1.0
[CHECKING] external v0.1.0 ([ROOT]/foo/external)
[CHECKING] foo v0.1.0 ([ROOT]/foo/foo)

"#]]
            .unordered(),
        )
        .run();

    p.cargo("check -Zcargo-lints -p foo")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> bar/Cargo.toml:13:13
   |
13 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[WARNING] unknown lint: `unused_dependencies`
   |
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
  --> foo/Cargo.toml:15:13
15 |             unused_dependencies = "warn"

"#]]
            .unordered(),
        )
        .run();

    p.cargo("check -Zcargo-lints -p bar")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(
            str![[r#"
[WARNING] unknown lint: `unused_dependencies`
  --> bar/Cargo.toml:13:13
   |
13 |             unused_dependencies = "warn"
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[WARNING] unknown lint: `unused_dependencies`
   |
   |             ^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
  --> foo/Cargo.toml:15:13
15 |             unused_dependencies = "warn"

"#]]
            .unordered(),
        )
        .run();
}
