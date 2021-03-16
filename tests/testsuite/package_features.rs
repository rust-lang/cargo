//! Tests for feature selection on the command-line.

use super::features2::switch_to_resolver_2;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, project};

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
        .with_stderr_unordered(
            "\
[UPDATING] [..]
[CHECKING] a v0.1.0 [..]
[CHECKING] b v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("check --features foo")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr("[ERROR] none of the selected packages contains these features: foo")
        .run();

    p.cargo("check --features a/dep1,b/f1,b/f2,f2")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr("[ERROR] none of the selected packages contains these features: b/f2, f2")
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
        .with_stderr_unordered(
            "\
[CHECKING] a [..]
[CHECKING] b [..]
[FINISHED] [..]
",
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

            [features]
            f1 = []
            f2 = []
            "#,
        )
        .file(
            "a/src/lib.rs",
            r#"
            #[cfg(not_feature = "f1")]
            compile_error!{"f1 is missing"}
            #[cfg(not_feature = "f2")]
            compile_error!{"f2 is missing"}
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"

            [features]
            f2 = []
            f3 = []
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
            #[cfg(not_feature = "f2")]
            compile_error!{"f2 is missing"}
            #[cfg(not_feature = "f3")]
            compile_error!{"f3 is missing"}
            "#,
        )
        .build();

    p.cargo("check -p a -p b --features f1,f2,f3")
        .with_stderr_unordered(
            "\
[CHECKING] a [..]
[CHECKING] b [..]
[FINISHED] [..]
",
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
        .with_stdout("f3f4")
        .run();

    p.cargo("run -p bar --features f1,f2")
        .with_status(101)
        .with_stderr("[ERROR] Package `foo[..]` does not have the feature `f2`")
        .run();

    p.cargo("run -p bar --features bar/f1")
        .with_stdout("f1f3")
        .run();

    // New behavior.
    switch_to_resolver_2(&p);
    p.cargo("run -p bar --features f1").with_stdout("f1").run();

    p.cargo("run -p bar --features f1,f2")
        .with_stdout("f1f2")
        .run();

    p.cargo("run -p bar --features bar/f1")
        .with_stdout("f1")
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
        .with_stderr_contains("[..]f1 is set[..]")
        .with_stderr_does_not_contain("[..]f2 is set[..]")
        .with_stderr_does_not_contain("[..]b is set[..]")
        .run();

    p.cargo("check -p a --features a/f1")
        .with_status(101)
        .with_stderr_contains("[..]f1 is set[..]")
        .with_stderr_does_not_contain("[..]f2 is set[..]")
        .with_stderr_does_not_contain("[..]b is set[..]")
        .run();

    p.cargo("check -p a --features a/f2")
        .with_status(101)
        .with_stderr_contains("[..]f1 is set[..]")
        .with_stderr_contains("[..]f2 is set[..]")
        .with_stderr_does_not_contain("[..]b is set[..]")
        .run();

    p.cargo("check -p a --features b/bfeat")
        .with_status(101)
        .with_stderr_contains("[..]bfeat is set[..]")
        .run();

    p.cargo("check -p a --no-default-features").run();

    p.cargo("check -p a --no-default-features --features b")
        .with_status(101)
        .with_stderr_contains("[..]b is set[..]")
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
            resolver = "2"

            [dependencies]
            dep = "1.0"

            [features]
            f1 = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -p dep --features f1")
        .with_status(101)
        .with_stderr(
            "[UPDATING][..]\n[ERROR] cannot specify features for packages outside of workspace",
        )
        .run();

    p.cargo("build -p dep --all-features")
        .with_status(101)
        .with_stderr("[ERROR] cannot specify features for packages outside of workspace")
        .run();

    p.cargo("build -p dep --no-default-features")
        .with_status(101)
        .with_stderr("[ERROR] cannot specify features for packages outside of workspace")
        .run();

    p.cargo("build -p dep")
        .with_stderr(
            "\
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[COMPILING] dep [..]
[FINISHED] [..]
",
        )
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
        .with_stdout("m1-feature set")
        .run();
}

#[cargo_test]
fn resolver1_non_member_optional_feature() {
    // --features x/y for an optional dependency `x` with the v1 resolver.
    Package::new("bar", "1.0.0")
        .feature("feat1", &[])
        .file(
            "src/lib.rs",
            r#"
                #[cfg(not(feature = "feat1"))]
                compile_error!("feat1 should be activated");
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

                [dependencies]
                bar = { version="1.0", optional=true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -p bar --features bar/feat1").run();
}
