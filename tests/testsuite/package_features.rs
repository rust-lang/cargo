//! Tests for feature selection on the command-line.

use super::features2::switch_to_resolver_2;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{basic_manifest, project};
use std::fmt::Write;

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
        .with_status(101)
        .with_stderr(
            "[ERROR] none of the selected packages contains these features: foo, did you mean: f1?",
        )
        .run();

    p.cargo("check --features a/dep1,b/f1,b/f2,f2")
        .with_status(101)
        .with_stderr("[ERROR] none of the selected packages contains these features: b/f2, f2, did you mean: f1?")
        .run();

    p.cargo("check --features a/dep,b/f1,b/f2,f2")
        .with_status(101)
        .with_stderr("[ERROR] none of the selected packages contains these features: a/dep, b/f2, f2, did you mean: a/dep1, f1?")
        .run();

    p.cargo("check --features a/dep,a/dep1")
        .with_status(101)
        .with_stderr("[ERROR] none of the selected packages contains these features: a/dep, did you mean: b/f1?")
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
            resolver = "2"

            [features]
            deny-warnings = []
            "#,
        )
        .file("src/lib.rs", "")
        .build()
        .cargo("check --features a/deny-warning")
        .with_status(101)
        .with_stderr(
            "[ERROR] none of the selected packages contains these features: a/deny-warning, did you mean: a/deny-warnings?",
        )
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
fn feature_default_resolver() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"

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
        .with_stderr("[ERROR] Package `a[..]` does not have the feature `testt`")
        .run();

    p.cargo("run --features test")
        .with_status(0)
        .with_stdout("feature set")
        .run();

    p.cargo("run --features a/test")
        .with_status(101)
        .with_stderr("[ERROR] package `a[..]` does not have a dependency named `a`")
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

    p.cargo("check -p dep --features f1")
        .with_status(101)
        .with_stderr("[ERROR] cannot specify features for packages outside of workspace")
        .run();

    p.cargo("check -p dep --all-features")
        .with_status(101)
        .with_stderr("[ERROR] cannot specify features for packages outside of workspace")
        .run();

    p.cargo("check -p dep --no-default-features")
        .with_status(101)
        .with_stderr("[ERROR] cannot specify features for packages outside of workspace")
        .run();

    p.cargo("check -p dep")
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[CHECKING] dep [..]
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

    p.cargo("check -p member1 --features member1/m2-feature")
        .cwd("member2")
        .with_status(101)
        .with_stderr("[ERROR] Package `member1[..]` does not have the feature `m2-feature`")
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
        .with_stderr(
            "\
[CHECKING] bar v1.0.0
[FINISHED] [..]
",
        )
        .run();
    // TODO: This should not be allowed (future warning?)
    p.cargo("check --features bar/jazz")
        .with_stderr(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] jazz v1.0.0 [..]
[CHECKING] jazz v1.0.0
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
    // TODO: This should not be allowed (future warning?)
    p.cargo("check -p bar --features bar/jazz -v")
        .with_stderr(
            "\
[FRESH] jazz v1.0.0
[FRESH] bar v1.0.0
[FINISHED] [..]
",
        )
        .run();

    ///////////////////////// V1 optional
    eprintln!("V1 optional");
    p.change_file("Cargo.toml", &make_toml("1", true));

    // This error isn't great, but is probably unlikely to be common in
    // practice, so I'm not going to put much effort into improving it.
    p.cargo("check -p bar")
        .with_status(101)
        .with_stderr(
            "\
error: package ID specification `bar` did not match any packages

<tab>Did you mean `foo`?
",
        )
        .run();

    p.cargo("check -p bar --features bar -v")
        .with_stderr(
            "\
[FRESH] bar v1.0.0
[FINISHED] [..]
",
        )
        .run();

    // TODO: This should not be allowed (future warning?)
    p.cargo("check -p bar --features bar/jazz -v")
        .with_stderr(
            "\
[FRESH] jazz v1.0.0
[FRESH] bar v1.0.0
[FINISHED] [..]
",
        )
        .run();

    ///////////////////////// V2 non-optional
    eprintln!("V2 non-optional");
    p.change_file("Cargo.toml", &make_toml("2", false));
    // TODO: This should not be allowed (future warning?)
    p.cargo("check --features bar/jazz -v")
        .with_stderr(
            "\
[FRESH] jazz v1.0.0
[FRESH] bar v1.0.0
[FRESH] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("check -p bar -v")
        .with_stderr(
            "\
[FRESH] bar v1.0.0
[FINISHED] [..]
",
        )
        .run();
    p.cargo("check -p bar --features bar/jazz")
        .with_status(101)
        .with_stderr("error: cannot specify features for packages outside of workspace")
        .run();

    ///////////////////////// V2 optional
    eprintln!("V2 optional");
    p.change_file("Cargo.toml", &make_toml("2", true));
    p.cargo("check -p bar")
        .with_status(101)
        .with_stderr(
            "\
error: package ID specification `bar` did not match any packages

<tab>Did you mean `foo`?
",
        )
        .run();
    // New --features behavior does not look at cwd.
    p.cargo("check -p bar --features bar")
        .with_status(101)
        .with_stderr("error: cannot specify features for packages outside of workspace")
        .run();
    p.cargo("check -p bar --features bar/jazz")
        .with_status(101)
        .with_stderr("error: cannot specify features for packages outside of workspace")
        .run();
    p.cargo("check -p bar --features foo/bar")
        .with_status(101)
        .with_stderr("error: cannot specify features for packages outside of workspace")
        .run();
}
