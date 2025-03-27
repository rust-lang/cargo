//! Tests for `[features]` table.

use crate::prelude::*;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, project};

#[cargo_test]
fn feature_activates_missing_feature() {
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
fn feature_activates_typoed_feature() {
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
                jaz = []
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
fn empty_feature_name() {
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
                "" = []
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] feature name cannot be empty
 --> Cargo.toml:9:17
  |
9 |                 "" = []
  |                 ^^

"#]])
        .run();
}

#[cargo_test]
fn same_name() {
    // Feature with the same name as a dependency.
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
                baz = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("tree -f")
        .arg("{p} [{f}]")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version

"#]])
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo) []
└── bar v1.0.0 ([ROOT]/foo/bar) []

"#]])
        .run();

    p.cargo("tree --features bar -f")
        .arg("{p} [{f}]")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo) [bar,baz]
└── bar v1.0.0 ([ROOT]/foo/bar) []

"#]])
        .run();
}

#[cargo_test]
fn feature_activates_required_dependency() {
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

                [dependencies.baz]
                path = "foo"
            "#,
        )
        .file("src/main.rs", "")
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
fn dependency_activates_missing_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                features = ["bar"]
            "#,
        )
        .file("src/main.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to select a version for `bar`.
    ... required by package `foo v0.0.1 ([ROOT]/foo)`
versions that meet the requirements `*` are: 0.0.1

package `foo` depends on `bar` with feature `bar` but `bar` does not have that feature.


failed to select a version for `bar` which could resolve this conflict

"#]])
        .run();

    p.change_file("Cargo.toml", &basic_manifest("foo", "0.0.1"));

    p.cargo("check --features test")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `foo v0.0.1 ([ROOT]/foo)` does not have the feature `test`

"#]])
        .run();
}

#[cargo_test]
fn dependency_activates_typoed_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                features = ["bar"]
            "#,
        )
        .file("src/main.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                baz = []
"#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to select a version for `bar`.
    ... required by package `foo v0.0.1 ([ROOT]/foo)`
versions that meet the requirements `*` are: 0.0.1

package `foo` depends on `bar` with feature `bar` but `bar` does not have that feature.
 package `bar` does have feature `baz`


failed to select a version for `bar` which could resolve this conflict

"#]])
        .run();
}

#[cargo_test]
fn optional_dev_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dev-dependencies.bar]
                path = "bar"
                optional = true
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dev-dependencies are not allowed to be optional: `bar`

"#]])
        .run();
}

#[cargo_test]
fn feature_activates_missing_dep_feature() {
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
                foo = ["bar/baz"]
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check --features foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] feature `foo` includes `bar/baz`, but `bar` is not a dependency
 --> Cargo.toml:9:23
  |
9 |                 foo = ["bar/baz"]
  |                       ^^^^^^^^^^^
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

"#]])
        .run();
}

#[cargo_test]
fn feature_activates_feature_inside_feature() {
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
                foo = ["bar/baz"]
                bar = []
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check --features foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] feature `foo` includes `bar/baz`, but `bar` is not a dependency
 --> Cargo.toml:9:23
  |
9 |                 foo = ["bar/baz"]
  |                       ^^^^^^^^^^^
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

"#]])
        .run();
}

#[cargo_test]
fn dependency_activates_dep_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                features = ["foo/bar"]
            "#,
        )
        .file("src/main.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check --features foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `foo/bar` in dependency `bar` is not allowed to contain slashes
  If you want to enable features [..]

"#]])
        .run();
}

#[cargo_test]
fn cli_activates_required_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check --features bar")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ERROR] package `foo v0.0.1 ([ROOT]/foo)` does not have feature `bar`

[HELP] a depednency with that name exists but it is required dependency and only optional dependencies can be used as features.

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn dependency_activates_required_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                features = ["baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.baz]
                path = "baz"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file("bar/baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("bar/baz/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[ERROR] failed to select a version for `bar`.
    ... required by package `foo v0.0.1 ([ROOT]/foo)`
versions that meet the requirements `*` are: 0.0.1

package `foo` depends on `bar` with feature `baz` but `bar` does not have that feature.
 A required dependency with that name exists, but only optional dependencies can be used as features.


failed to select a version for `bar` which could resolve this conflict

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn no_transitive_dep_feature_requirement() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.derived]
                path = "derived"

                [features]
                default = ["derived/bar/qux"]
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern crate derived;
                fn main() { derived::test(); }
            "#,
        )
        .file(
            "derived/Cargo.toml",
            r#"
                [package]
                name = "derived"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("derived/src/lib.rs", "extern crate bar; pub use bar::test;")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                qux = []
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                #[cfg(feature = "qux")]
                pub fn test() { print!("test"); }
            "#,
        )
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  multiple slashes in feature `derived/bar/qux` (included by feature `default`) are not allowed

"#]])
        .run();
}

#[cargo_test]
fn no_feature_doesnt_build() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                optional = true
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[cfg(feature = "bar")]
                extern crate bar;
                #[cfg(feature = "bar")]
                fn main() { bar::bar(); println!("bar") }
                #[cfg(not(feature = "bar"))]
                fn main() {}
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.process(&p.bin("foo")).with_stdout_data("").run();

    let expected = if cfg!(target_os = "windows") && cfg!(target_env = "msvc") {
        str![[r#"
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar [..]`
[DIRTY] foo v0.0.1 ([ROOT]/foo): the list of features changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
    } else {
        str![[r#"
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar [..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
    };
    p.cargo("build --features bar -v")
        .with_stderr_data(expected)
        .run();
    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
bar

"#]])
        .run();
}

#[cargo_test]
fn default_feature_pulled_in() {
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
                default = ["bar"]

                [dependencies.bar]
                path = "bar"
                optional = true
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[cfg(feature = "bar")]
                extern crate bar;
                #[cfg(feature = "bar")]
                fn main() { bar::bar(); println!("bar") }
                #[cfg(not(feature = "bar"))]
                fn main() {}
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
bar

"#]])
        .run();

    let expected = if cfg!(target_os = "windows") && cfg!(target_env = "msvc") {
        str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the list of features changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
    } else {
        str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
    };
    p.cargo("build --no-default-features -v")
        .with_stderr_data(expected)
        .run();
    p.process(&p.bin("foo")).with_stdout_data("").run();
}

#[cargo_test]
fn cyclic_feature() {
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
                default = ["default"]
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cyclic feature dependency: feature `default` depends on itself

"#]])
        .run();
}

#[cargo_test]
fn cyclic_feature2() {
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
                foo = ["bar"]
                bar = ["foo"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn groups_on_groups_on_groups() {
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
                default = ["f1"]
                f1 = ["f2", "bar"]
                f2 = ["f3", "f4"]
                f3 = ["f5", "f6", "baz"]
                f4 = ["f5", "f7"]
                f5 = ["f6"]
                f6 = ["f7"]
                f7 = ["bar"]

                [dependencies.bar]
                path = "bar"
                optional = true

                [dependencies.baz]
                path = "baz"
                optional = true
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate bar;
                #[allow(unused_extern_crates)]
                extern crate baz;
                fn main() {}
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[CHECKING] baz v0.0.1 ([ROOT]/foo/baz)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn many_cli_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                optional = true

                [dependencies.baz]
                path = "baz"
                optional = true
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate bar;
                #[allow(unused_extern_crates)]
                extern crate baz;
                fn main() {}
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check --features")
        .arg("bar baz")
        .with_stderr_data(
            str![[r#"
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[CHECKING] baz v0.0.1 ([ROOT]/foo/baz)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn union_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.d1]
                path = "d1"
                features = ["f1"]
                [dependencies.d2]
                path = "d2"
                features = ["f2"]
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate d1;
                extern crate d2;
                fn main() {
                    d2::f1();
                    d2::f2();
                }
            "#,
        )
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                f1 = ["d2"]

                [dependencies.d2]
                path = "../d2"
                features = ["f1"]
                optional = true
            "#,
        )
        .file("d1/src/lib.rs", "")
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                f1 = []
                f2 = []
            "#,
        )
        .file(
            "d2/src/lib.rs",
            r#"
                #[cfg(feature = "f1")] pub fn f1() {}
                #[cfg(feature = "f2")] pub fn f2() {}
            "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[CHECKING] d2 v0.0.1 ([ROOT]/foo/d2)
[CHECKING] d1 v0.0.1 ([ROOT]/foo/d1)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn many_features_no_rebuilds() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name    = "b"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies.a]
                path = "a"
                features = ["fall"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name    = "a"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [features]
                ftest  = []
                ftest2 = []
                fall   = ["ftest", "ftest2"]
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[CHECKING] b v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.root().move_into_the_past();

    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[FRESH] a v0.1.0 ([ROOT]/foo/a)
[FRESH] b v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

// Tests that all cmd lines work with `--features ""`
#[cargo_test]
fn empty_features() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("check --features").arg("").run();
}

// Tests that all cmd lines work with `--features ""`
#[cargo_test]
fn transitive_features() {
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
                foo = ["bar/baz"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() { bar::baz(); }")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                baz = []
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"#[cfg(feature = "baz")] pub fn baz() {}"#,
        )
        .build();

    p.cargo("check --features foo").run();
}

#[cargo_test]
fn everything_in_the_lockfile() {
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
                f1 = ["d1/f1"]
                f2 = ["d2"]

                [dependencies.d1]
                path = "d1"
                [dependencies.d2]
                path = "d2"
                optional = true
                [dependencies.d3]
                path = "d3"
                optional = true
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                f1 = []
            "#,
        )
        .file("d1/src/lib.rs", "")
        .file("d2/Cargo.toml", &basic_manifest("d2", "0.0.2"))
        .file("d2/src/lib.rs", "")
        .file(
            "d3/Cargo.toml",
            r#"
                [package]
                name = "d3"
                version = "0.0.3"
                edition = "2015"
                authors = []

                [features]
                f3 = []
            "#,
        )
        .file("d3/src/lib.rs", "")
        .build();

    p.cargo("fetch").run();
    let lockfile = p.read_lockfile();
    assert!(
        lockfile.contains(r#"name = "d1""#),
        "d1 not found\n{}",
        lockfile
    );
    assert!(
        lockfile.contains(r#"name = "d2""#),
        "d2 not found\n{}",
        lockfile
    );
    assert!(
        lockfile.contains(r#"name = "d3""#),
        "d3 not found\n{}",
        lockfile
    );
}

#[cargo_test]
fn no_rebuild_when_frobbing_default_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "a" }
                b = { path = "b" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "../a", features = ["f1"], default-features = false }
            "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [features]
                default = ["f1"]
                f1 = []
            "#,
        )
        .file("a/src/lib.rs", "")
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
fn unions_work_with_no_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "a" }
                b = { path = "b" }
            "#,
        )
        .file("src/lib.rs", "extern crate a; pub fn foo() { a::a(); }")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "../a", features = [], default-features = false }
            "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [features]
                default = ["f1"]
                f1 = []
            "#,
        )
        .file("a/src/lib.rs", r#"#[cfg(feature = "f1")] pub fn a() {}"#)
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
fn optional_and_dev_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name    = "test"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = { path = "foo", optional = true }
                [dev-dependencies]
                foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] test v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn activating_feature_activates_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name    = "test"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = { path = "foo", optional = true }

                [features]
                a = ["foo/a"]
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate foo; pub fn bar() { foo::bar(); }",
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [features]
                a = []
            "#,
        )
        .file("foo/src/lib.rs", r#"#[cfg(feature = "a")] pub fn bar() {}"#)
        .build();

    p.cargo("check --features a -v").run();
}

#[cargo_test]
fn activating_feature_does_not_activate_transitive_dev_dependency() {
    let p = project()
        .no_manifest()
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.0"
                edition = "2021"

                [features]
                f = ["b/f"]

                [dependencies]
                b = { path = "../b" }
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.0.0"
                edition = "2021"

                [features]
                f = ["c/f"]

                [dev-dependencies]
                c = { path = "../c" }
            "#,
        )
        .file(
            "c/Cargo.toml",
            r#"
                [package]
                name = "c"
                version = "0.0.0"
                edition = "2021"

                [features]
                f = []
            "#,
        )
        .file("a/src/lib.rs", "")
        .file("b/src/lib.rs", "")
        .file("c/src/lib.rs", "compile_error!")
        .build();

    p.cargo("check --manifest-path a/Cargo.toml --features f")
        .run();
}

#[cargo_test]
fn dep_feature_in_cmd_line() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.derived]
                path = "derived"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern crate derived;
                fn main() { derived::test(); }
            "#,
        )
        .file(
            "derived/Cargo.toml",
            r#"
                [package]
                name = "derived"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"

                [features]
                default = []
                derived-feat = ["bar/some-feat"]
            "#,
        )
        .file("derived/src/lib.rs", "extern crate bar; pub use bar::test;")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                some-feat = []
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                #[cfg(feature = "some-feat")]
                pub fn test() { print!("test"); }
            "#,
        )
        .build();

    // The foo project requires that feature "some-feat" in "bar" is enabled.
    // Building without any features enabled should fail:
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
error[E0432]: unresolved import `bar::test`
...
"#]])
        .run();

    // We should be able to enable the feature "derived-feat", which enables "some-feat",
    // on the command line. The feature is enabled, thus building should be successful:
    p.cargo("check --features derived/derived-feat").run();

    // Trying to enable features of transitive dependencies is an error
    p.cargo("check --features bar/some-feat")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `foo v0.0.1 ([ROOT]/foo)` does not have a dependency named `bar`

"#]])
        .run();

    // Hierarchical feature specification should still be disallowed
    p.cargo("check --features derived/bar/some-feat")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] multiple slashes in feature `derived/bar/some-feat` is not allowed

"#]])
        .run();
}

#[cargo_test]
fn all_features_flag_enables_all_features() {
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
                foo = []
                bar = []

                [dependencies.baz]
                path = "baz"
                optional = true
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[cfg(feature = "foo")]
                pub fn foo() {}

                #[cfg(feature = "bar")]
                pub fn bar() {
                    extern crate baz;
                    baz::baz();
                }

                fn main() {
                    foo();
                    bar();
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check --all-features").run();
}

#[cargo_test]
fn many_cli_features_comma_delimited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                optional = true

                [dependencies.baz]
                path = "baz"
                optional = true
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate bar;
                #[allow(unused_extern_crates)]
                extern crate baz;
                fn main() {}
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("check --features bar,baz")
        .with_stderr_data(
            str![[r#"
[LOCKING] 2 packages to latest compatible versions
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[CHECKING] baz v0.0.1 ([ROOT]/foo/baz)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn many_cli_features_comma_and_space_delimited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
                optional = true

                [dependencies.baz]
                path = "baz"
                optional = true

                [dependencies.bam]
                path = "bam"
                optional = true

                [dependencies.bap]
                path = "bap"
                optional = true
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate bar;
                #[allow(unused_extern_crates)]
                extern crate baz;
                #[allow(unused_extern_crates)]
                extern crate bam;
                #[allow(unused_extern_crates)]
                extern crate bap;
                fn main() {}
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .file("bam/Cargo.toml", &basic_manifest("bam", "0.0.1"))
        .file("bam/src/lib.rs", "pub fn bam() {}")
        .file("bap/Cargo.toml", &basic_manifest("bap", "0.0.1"))
        .file("bap/src/lib.rs", "pub fn bap() {}")
        .build();

    p.cargo("check --features")
        .arg("bar,baz bam bap")
        .with_stderr_data(
            str![[r#"
[LOCKING] 4 packages to latest compatible versions
[CHECKING] bam v0.0.1 ([ROOT]/foo/bam)
[CHECKING] bap v0.0.1 ([ROOT]/foo/bap)
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[CHECKING] baz v0.0.1 ([ROOT]/foo/baz)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn only_dep_is_optional() {
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

                [features]
                foo = ['bar']

                [dependencies]
                bar = { version = "0.1", optional = true }

                [dev-dependencies]
                bar = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn all_features_all_crates() {
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

                [workspace]
                members = ['bar']
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                foo = []
            "#,
        )
        .file("bar/src/main.rs", "#[cfg(feature = \"foo\")] fn main() {}")
        .build();

    p.cargo("check --all-features --workspace").run();
}

#[cargo_test]
fn feature_off_dylib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]

                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lib]
                crate-type = ["dylib"]

                [features]
                f1 = []
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn hello() -> &'static str {
                    if cfg!(feature = "f1") {
                        "f1"
                    } else {
                        "no f1"
                    }
                }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                foo = { path = ".." }
            "#,
        )
        .file(
            "bar/src/main.rs",
            r#"
                extern crate foo;

                fn main() {
                    assert_eq!(foo::hello(), "no f1");
                }
            "#,
        )
        .build();

    // Build the dylib with `f1` feature.
    p.cargo("check --features f1").run();
    // Check that building without `f1` uses a dylib without `f1`.
    p.cargo("run -p bar").run();
}

#[cargo_test]
fn warn_if_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
               [package]
               name = "foo"
               version = "0.0.1"
               edition = "2015"
               authors = []

               [dependencies.bar]
               path = "bar"
               optional = true

               [features]
               default-features = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] `[features]` defines a feature named `default-features`
[NOTE] only a feature named `default` will be enabled by default
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn no_feature_for_non_optional_dep() {
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
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[cfg(not(feature = "bar"))]
                fn main() {
                }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                a = []
            "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("check --features bar/a").run();
}

#[cargo_test]
fn features_option_given_twice() {
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
                a = []
                b = []
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[cfg(all(feature = "a", feature = "b"))]
                fn main() {}
            "#,
        )
        .build();

    p.cargo("check --features a --features b").run();
}

#[cargo_test]
fn multi_multi_features() {
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
                a = []
                b = []
                c = []
            "#,
        )
        .file(
            "src/main.rs",
            r#"
               #[cfg(all(feature = "a", feature = "b", feature = "c"))]
               fn main() {}
            "#,
        )
        .build();

    p.cargo("check --features a --features").arg("b c").run();
}

#[cargo_test]
fn cli_parse_ok() {
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
                a = []
            "#,
        )
        .file(
            "src/main.rs",
            r#"
               #[cfg(feature = "a")]
               fn main() {
                    assert_eq!(std::env::args().nth(1).unwrap(), "b");
               }
            "#,
        )
        .build();

    p.cargo("run --features a b").run();
}

#[cargo_test]
fn all_features_virtual_ws() {
    // What happens with `--all-features` in the root of a virtual workspace.
    // Some of this behavior is a little strange (member dependencies also
    // have all features enabled, one might expect `f4` to be disabled).
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                b = {path="../b", optional=true}

                [features]
                default = ["f1"]
                f1 = []
                f2 = []
            "#,
        )
        .file(
            "a/src/main.rs",
            r#"
                fn main() {
                    if cfg!(feature="f1") {
                        println!("f1");
                    }
                    if cfg!(feature="f2") {
                        println!("f2");
                    }
                    #[cfg(feature="b")]
                    b::f();
                }
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2015"

                [features]
                default = ["f3"]
                f3 = []
                f4 = []
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
                pub fn f() {
                    if cfg!(feature="f3") {
                        println!("f3");
                    }
                    if cfg!(feature="f4") {
                        println!("f4");
                    }
                }
            "#,
        )
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
f1

"#]])
        .run();
    p.cargo("run --all-features")
        .with_stdout_data(str![[r#"
f1
f2
f3
f4

"#]])
        .run();
    // In `a`, it behaves differently. :(
    p.cargo("run --all-features")
        .cwd("a")
        .with_stdout_data(str![[r#"
f1
f2
f3

"#]])
        .run();
}

#[cargo_test]
fn slash_optional_enables() {
    // --features dep/feat will enable `dep` and set its feature.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
                edition = "2015"

            [dependencies]
            dep = {path="dep", optional=true}
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(not(feature="dep"))]
            compile_error!("dep not set");
            "#,
        )
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"

            [features]
            feat = []
            "#,
        )
        .file(
            "dep/src/lib.rs",
            r#"
            #[cfg(not(feature="feat"))]
            compile_error!("feat not set");
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] dep not set
...
"#]])
        .run();

    p.cargo("check --features dep/feat").run();
}

#[cargo_test]
fn registry_summary_order_doesnt_matter() {
    // Checks for an issue where the resolver depended on the order of entries
    // in the registry summary. If there was a non-optional dev-dependency
    // that appeared before an optional normal dependency, then the resolver
    // would not activate the optional dependency with a pkg/featname feature
    // syntax.
    Package::new("dep", "0.1.0")
        .feature("feat1", &[])
        .file(
            "src/lib.rs",
            r#"
                #[cfg(feature="feat1")]
                pub fn work() {
                    println!("it works");
                }
            "#,
        )
        .publish();
    Package::new("bar", "0.1.0")
        .feature("bar_feat", &["dep/feat1"])
        .add_dep(Dependency::new("dep", "0.1.0").dev())
        .add_dep(Dependency::new("dep", "0.1.0").optional(true))
        .file(
            "src/lib.rs",
            r#"
                // This will fail to compile without `dep` optional dep activated.
                extern crate dep;

                pub fn doit() {
                    dep::work();
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
                edition = "2018"

                [dependencies]
                bar = { version="0.1", features = ["bar_feat"] }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    bar::doit();
                }
            "#,
        )
        .build();

    p.cargo("run")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] bar v0.1.0 (registry `dummy-registry`)
[COMPILING] dep v0.1.0
[COMPILING] bar v0.1.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
it works

"#]])
        .run();
}

#[cargo_test]
fn nonexistent_required_features() {
    Package::new("required_dependency", "0.1.0")
        .feature("simple", &[])
        .publish();
    Package::new("optional_dependency", "0.2.0")
        .feature("optional", &[])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            [features]
            existing = []
            fancy = ["optional_dependency"]
            [dependencies]
            required_dependency = { version = "0.1", optional = false}
            optional_dependency = { version = "0.2", optional = true}
            [[example]]
            name = "ololo"
            required-features = ["not_present",
                                 "existing",
                                 "fancy",
                                 "required_dependency/not_existing",
                                 "required_dependency/simple",
                                 "optional_dependency/optional",
                                 "not_specified_dependency/some_feature"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/ololo.rs", "fn main() {}")
        .build();

    p.cargo("check --examples").with_stderr_data(str![[r#"
...
[WARNING] invalid feature `not_present` in required-features of target `ololo`: `not_present` is not present in [features] section
[WARNING] invalid feature `required_dependency/not_existing` in required-features of target `ololo`: feature `not_existing` does not exist in package `required_dependency v0.1.0`
[WARNING] invalid feature `not_specified_dependency/some_feature` in required-features of target `ololo`: dependency `not_specified_dependency` does not exist
...
"#]]).run();
}

#[cargo_test]
fn invalid_feature_names_error() {
    // Errors for more restricted feature syntax.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [features]
                # Invalid start character.
                "+foo" = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character `+` in feature name: `+foo`, the first character must be a Unicode XID start character or digit (most letters or `_` or `0` to `9`)
 --> Cargo.toml:9:17
  |
9 |                 "+foo" = []
  |                 ^^^^^^

"#]])
        .run();

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [features]
            # Invalid continue character.
            "a&b" = []
        "#,
    );

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character `&` in feature name: `a&b`, characters must be Unicode XID characters, '-', `+`, or `.` (numbers, `+`, `-`, `_`, `.`, or most letters)
 --> Cargo.toml:9:13
  |
9 |             "a&b" = []
  |             ^^^^^

"#]])
        .run();
}

#[cargo_test]
fn invalid_feature_name_slash_error() {
    // Errors for more restricted feature syntax.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [features]
                "foo/bar" = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character `/` in feature name: `foo/bar`, feature name is not allowed to contain slashes
 --> Cargo.toml:8:17
  |
8 |                 "foo/bar" = []
  |                 ^^^^^^^^^

"#]])
        .run();
}
