//! Tests for feature selection on the command-line.

use std::fmt::Write;

use crate::prelude::*;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{basic_manifest, project, str};

use super::features2::switch_to_resolver_2;

#[cargo_test]
fn virtual_no_default_features() {
    // --no-default-features in root of virtual workspace.
    Package::new("dep1", "1.0.0").publish();
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
            edition = "2015"

            [dependencies]
            dep1 = {version = "1.0", optional = true}

            [features]
            default = ["dep1"]
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"
            edition = "2015"

            [features]
            default = ["f1"]
            f1 = []
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
            #[cfg(feature = "f1")]
            compile_error!{"expected f1 off"}
            "#,
        )
        .build();

    p.cargo("check --no-default-features")
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[CHECKING] b v0.1.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    p.cargo("check --features foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] none of the selected packages contains this feature: foo
selected packages: a, b
[HELP] there is a similarly named feature: f1

"#]])
        .run();

    p.cargo("check --features a/dep1,b/f1,b/f2,f2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] none of the selected packages contains these features: b/f2, f2
selected packages: a, b
[HELP] there is a similarly named feature: f1

"#]])
        .run();

    p.cargo("check --features a/dep,b/f1,b/f2,f2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] none of the selected packages contains these features: a/dep, b/f2, f2
selected packages: a, b
[HELP] there are similarly named features: a/dep1, f1

"#]])
        .run();

    p.cargo("check --features a/dep,a/dep1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] none of the selected packages contains this feature: a/dep
selected packages: a, b
[HELP] there is a similarly named feature: b/f1

"#]])
        .run();

    p.cargo("check -p b --features=dep1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'b' does not contain this feature: dep1
[HELP] package with the missing feature: a

"#]])
        .run();
}

#[cargo_test]
fn virtual_typo_member_feature() {
    project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2015"
            resolver = "2"

            [features]
            deny-warnings = []
            "#,
        )
        .file("src/lib.rs", "")
        .build()
        .cargo("check --features a/deny-warning")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the package 'a' does not contain this feature: a/deny-warning
[HELP] there is a similarly named feature: a/deny-warnings

"#]])
        .run();
}

#[cargo_test]
fn virtual_features() {
    // --features in root of virtual workspace.
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
            edition = "2015"

            [features]
            f1 = []
            "#,
        )
        .file(
            "a/src/lib.rs",
            r#"
            #[cfg(not(feature = "f1"))]
            compile_error!{"f1 is missing"}
            "#,
        )
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("check --features f1")
        .with_stderr_data(
            str![[r#"
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[CHECKING] b v0.1.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn virtual_with_specific() {
    // -p flags with --features in root of virtual.
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
            edition = "2015"

            [features]
            f1 = []
            f2 = []
            "#,
        )
        .file(
            "a/src/lib.rs",
            r#"
            #[cfg(not(feature = "f1"))]
            compile_error!{"f1 is missing"}
            #[cfg(not(feature = "f2"))]
            compile_error!{"f2 is missing"}
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
            f2 = []
            f3 = []
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
            #[cfg(not(feature = "f2"))]
            compile_error!{"f2 is missing"}
            #[cfg(not(feature = "f3"))]
            compile_error!{"f3 is missing"}
            "#,
        )
        .build();

    p.cargo("check -p a -p b --features f1,f2,f3")
        .with_stderr_data(
            str![[r#"
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[CHECKING] b v0.1.0 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn other_member_from_current() {
    // -p for another member while in the current directory.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar = { path="bar", features=["f3"] }

            [features]
            f1 = ["bar/f4"]
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
            f1 = []
            f2 = []
            f3 = []
            f4 = []
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "bar/src/main.rs",
            r#"
            fn main() {
                if cfg!(feature = "f1") {
                    print!("f1");
                }
                if cfg!(feature = "f2") {
                    print!("f2");
                }
                if cfg!(feature = "f3") {
                    print!("f3");
                }
                if cfg!(feature = "f4") {
                    print!("f4");
                }
                println!();
            }
            "#,
        )
        .build();

    // Old behavior.
    p.cargo("run -p bar --features f1")
        .with_stdout_data(str![[r#"
f3f4

"#]])
        .run();

    p.cargo("run -p bar --features f1,f2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `foo v0.1.0 ([ROOT]/foo)` does not have the feature `f2`

[HELP] a feature with a similar name exists: `f1`

"#]])
        .run();

    p.cargo("run -p bar --features bar/f1")
        .with_stdout_data(str![[r#"
f1f3

"#]])
        .run();

    // New behavior.
    switch_to_resolver_2(&p);
    p.cargo("run -p bar --features f1")
        .with_stdout_data(str![[r#"
f1

"#]])
        .run();

    p.cargo("run -p bar --features f1,f2")
        .with_stdout_data(str![[r#"
f1f2

"#]])
        .run();

    p.cargo("run -p bar --features bar/f1")
        .with_stdout_data(str![[r#"
f1

"#]])
        .run();
}

#[cargo_test]
fn feature_default_resolver() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2015"

            [features]
            test = []
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    if cfg!(feature = "test") {
                        println!("feature set");
                    }
                }
            "#,
        )
        .build();

    p.cargo("check --features testt")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `a v0.1.0 ([ROOT]/foo)` does not have the feature `testt`

[HELP] a feature with a similar name exists: `test`

"#]])
        .run();

    p.cargo("run --features test")
        .with_status(0)
        .with_stdout_data(str![[r#"
feature set

"#]])
        .run();

    p.cargo("run --features a/test")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `a v0.1.0 ([ROOT]/foo)` does not have a dependency named `a`

"#]])
        .run();
}

#[cargo_test]
fn command_line_optional_dep() {
    // Enabling a dependency used as a `dep:` errors helpfully
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2015"

            [features]
            foo = ["dep:bar"]

            [dependencies]
            bar = { version = "1.0.0", optional = true }
            "#,
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("check --features bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[ERROR] package `a v0.1.0 ([ROOT]/foo)` does not have feature `bar`

[HELP] an optional dependency with that name exists, but the `features` table includes it with the "dep:" syntax so it does not have an implicit feature with that name
Dependency `bar` would be enabled by these features:
	- `foo`

"#]])
        .run();
}

#[cargo_test]
fn command_line_optional_dep_three_options() {
    // Trying to enable an optional dependency used as a `dep:` errors helpfully, when there are three features which would enable the dependency
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2015"

            [features]
            f1 = ["dep:bar"]
            f2 = ["dep:bar"]
            f3 = ["dep:bar"]

            [dependencies]
            bar = { version = "1.0.0", optional = true }
            "#,
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("check --features bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[ERROR] package `a v0.1.0 ([ROOT]/foo)` does not have feature `bar`

[HELP] an optional dependency with that name exists, but the `features` table includes it with the "dep:" syntax so it does not have an implicit feature with that name
Dependency `bar` would be enabled by these features:
	- `f1`
	- `f2`
	- `f3`

"#]])
        .run();
}

#[cargo_test]
fn command_line_optional_dep_many_options() {
    // Trying to enable an optional dependency used as a `dep:` errors helpfully, when there are many features which would enable the dependency
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2015"

            [features]
            f1 = ["dep:bar"]
            f2 = ["dep:bar"]
            f3 = ["dep:bar"]
            f4 = ["dep:bar"]

            [dependencies]
            bar = { version = "1.0.0", optional = true }
            "#,
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("check --features bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[ERROR] package `a v0.1.0 ([ROOT]/foo)` does not have feature `bar`

[HELP] an optional dependency with that name exists, but the `features` table includes it with the "dep:" syntax so it does not have an implicit feature with that name
Dependency `bar` would be enabled by these features:
	- `f1`
	- `f2`
	- `f3`
	  ...

"#]])
        .run();
}

#[cargo_test]
fn command_line_optional_dep_many_paths() {
    // Trying to enable an optional dependency used as a `dep:` errors helpfully, when a features would enable the dependency in multiple ways
    Package::new("bar", "1.0.0")
        .feature("a", &[])
        .feature("b", &[])
        .feature("c", &[])
        .feature("d", &[])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2015"

            [features]
            f1 = ["dep:bar", "bar/a", "bar/b"] # Remove the implicit feature
            f2 = ["bar/b", "bar/c"] # Overlaps with previous
            f3 = ["bar/d"] # No overlap with previous

            [dependencies]
            bar = { version = "1.0.0", optional = true }
            "#,
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("check --features bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[ERROR] package `a v0.1.0 ([ROOT]/foo)` does not have feature `bar`

[HELP] an optional dependency with that name exists, but the `features` table includes it with the "dep:" syntax so it does not have an implicit feature with that name
Dependency `bar` would be enabled by these features:
	- `f1`
	- `f2`
	- `f3`

"#]])
        .run();
}

#[cargo_test]
fn virtual_member_slash() {
    // member slash feature syntax
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            b = {path="../b", optional=true}

            [features]
            default = ["f1"]
            f1 = []
            f2 = []
            "#,
        )
        .file(
            "a/src/lib.rs",
            r#"
            #[cfg(feature = "f1")]
            compile_error!{"f1 is set"}

            #[cfg(feature = "f2")]
            compile_error!{"f2 is set"}

            #[cfg(feature = "b")]
            compile_error!{"b is set"}
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
            bfeat = []
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
            #[cfg(feature = "bfeat")]
            compile_error!{"bfeat is set"}
            "#,
        )
        .build();

    p.cargo("check -p a")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] f1 is set
...
"#]])
        .with_stderr_does_not_contain("[..]f2 is set[..]")
        .with_stderr_does_not_contain("[..]b is set[..]")
        .run();

    p.cargo("check -p a --features a/f1")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] f1 is set
...
"#]])
        .with_stderr_does_not_contain("[..]f2 is set[..]")
        .with_stderr_does_not_contain("[..]b is set[..]")
        .run();

    p.cargo("check -p a --features a/f2")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] f1 is set
...
[ERROR] f2 is set
...
"#]])
        .with_stderr_does_not_contain("[..]b is set[..]")
        .run();

    p.cargo("check -p a --features b/bfeat")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] bfeat is set
...
"#]])
        .run();

    p.cargo("check -p a --no-default-features").run();

    p.cargo("check -p a --no-default-features --features b")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] b is set
...
"#]])
        .run();
}

#[cargo_test]
fn non_member() {
    // -p for a non-member
    Package::new("dep", "1.0.0").publish();
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
            dep = "1.0"

            [features]
            f1 = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -p dep --features f1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot specify features for packages outside of workspace

"#]])
        .run();

    p.cargo("check -p dep --all-features")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot specify features for packages outside of workspace

"#]])
        .run();

    p.cargo("check -p dep --no-default-features")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot specify features for packages outside of workspace

"#]])
        .run();

    p.cargo("check -p dep")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v1.0.0 (registry `dummy-registry`)
[CHECKING] dep v1.0.0
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn resolver1_member_features() {
    // --features member-name/feature-name with resolver="1"
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["member1", "member2"]
            "#,
        )
        .file(
            "member1/Cargo.toml",
            r#"
                [package]
                name = "member1"
                version = "0.1.0"
                edition = "2015"

                [features]
                m1-feature = []
            "#,
        )
        .file(
            "member1/src/main.rs",
            r#"
                fn main() {
                    if cfg!(feature = "m1-feature") {
                        println!("m1-feature set");
                    }
                }
            "#,
        )
        .file("member2/Cargo.toml", &basic_manifest("member2", "0.1.0"))
        .file("member2/src/lib.rs", "")
        .build();

    p.cargo("run -p member1 --features member1/m1-feature")
        .cwd("member2")
        .with_stdout_data(str![[r#"
m1-feature set

"#]])
        .run();

    p.cargo("check -p member1 --features member1/m2-feature")
        .cwd("member2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package `member1 v0.1.0 ([ROOT]/foo/member1)` does not have the feature `m2-feature`

[HELP] a feature with a similar name exists: `m1-feature`

"#]])
        .run();
}

#[cargo_test]
fn non_member_feature() {
    // --features for a non-member
    Package::new("jazz", "1.0.0").publish();
    Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("jazz", "1.0").optional(true))
        .publish();
    let make_toml = |resolver, optional| {
        let mut s = String::new();
        write!(
            s,
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                resolver = "{}"

                [dependencies]
            "#,
            resolver
        )
        .unwrap();
        if optional {
            s.push_str(r#"bar = { version = "1.0", optional = true } "#);
        } else {
            s.push_str(r#"bar = "1.0""#)
        }
        s.push('\n');
        s
    };
    let p = project()
        .file("Cargo.toml", &make_toml("1", false))
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch").run();
    ///////////////////////// V1 non-optional
    eprintln!("V1 non-optional");
    p.cargo("check -p bar")
        .with_stderr_data(str![[r#"
[CHECKING] bar v1.0.0
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // TODO: This should not be allowed (future warning?)
    p.cargo("check --features bar/jazz")
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] jazz v1.0.0 (registry `dummy-registry`)
[CHECKING] jazz v1.0.0
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // TODO: This should not be allowed (future warning?)
    p.cargo("check -p bar --features bar/jazz -v")
        .with_stderr_data(str![[r#"
[FRESH] jazz v1.0.0
[FRESH] bar v1.0.0
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    ///////////////////////// V1 optional
    eprintln!("V1 optional");
    p.change_file("Cargo.toml", &make_toml("1", true));

    // This error isn't great, but is probably unlikely to be common in
    // practice, so I'm not going to put much effort into improving it.
    p.cargo("check -p bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `bar` did not match any packages

[HELP] a package with a similar name exists: `foo`

"#]])
        .run();

    p.cargo("check -p bar --features bar -v")
        .with_stderr_data(str![[r#"
[FRESH] bar v1.0.0
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // TODO: This should not be allowed (future warning?)
    p.cargo("check -p bar --features bar/jazz -v")
        .with_stderr_data(str![[r#"
[FRESH] jazz v1.0.0
[FRESH] bar v1.0.0
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    ///////////////////////// V2 non-optional
    eprintln!("V2 non-optional");
    p.change_file("Cargo.toml", &make_toml("2", false));
    // TODO: This should not be allowed (future warning?)
    p.cargo("check --features bar/jazz -v")
        .with_stderr_data(str![[r#"
[FRESH] jazz v1.0.0
[FRESH] bar v1.0.0
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check -p bar -v")
        .with_stderr_data(str![[r#"
[FRESH] bar v1.0.0
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check -p bar --features bar/jazz")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot specify features for packages outside of workspace

"#]])
        .run();

    ///////////////////////// V2 optional
    eprintln!("V2 optional");
    p.change_file("Cargo.toml", &make_toml("2", true));
    p.cargo("check -p bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `bar` did not match any packages

[HELP] a package with a similar name exists: `foo`

"#]])
        .run();
    // New --features behavior does not look at cwd.
    p.cargo("check -p bar --features bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot specify features for packages outside of workspace

"#]])
        .run();
    p.cargo("check -p bar --features bar/jazz")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot specify features for packages outside of workspace

"#]])
        .run();
    p.cargo("check -p bar --features foo/bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot specify features for packages outside of workspace

"#]])
        .run();
}
