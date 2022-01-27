//! Tests for the `cargo tree` command with -e features option.

use cargo_test_support::project;
use cargo_test_support::registry::{Dependency, Package};

#[cargo_test]
fn dep_feature_various() {
    // Checks different ways of setting features via dependencies.
    Package::new("optdep", "1.0.0")
        .feature("default", &["cat"])
        .feature("cat", &[])
        .publish();
    Package::new("defaultdep", "1.0.0")
        .feature("default", &["f1"])
        .feature("f1", &["optdep"])
        .add_dep(Dependency::new("optdep", "1.0").optional(true))
        .publish();
    Package::new("nodefaultdep", "1.0.0")
        .feature("default", &["f1"])
        .feature("f1", &[])
        .publish();
    Package::new("nameddep", "1.0.0")
        .add_dep(Dependency::new("serde", "1.0").optional(true))
        .feature("default", &["serde-stuff"])
        .feature("serde-stuff", &["serde/derive"])
        .feature("vehicle", &["car"])
        .feature("car", &[])
        .publish();
    Package::new("serde_derive", "1.0.0").publish();
    Package::new("serde", "1.0.0")
        .feature("derive", &["serde_derive"])
        .add_dep(Dependency::new("serde_derive", "1.0").optional(true))
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            defaultdep = "1.0"
            nodefaultdep = {version="1.0", default-features = false}
            nameddep = {version="1.0", features = ["vehicle", "serde"]}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e features")
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo)
├── nodefaultdep v1.0.0
├── defaultdep feature \"default\"
│   ├── defaultdep v1.0.0
│   │   └── optdep feature \"default\"
│   │       ├── optdep v1.0.0
│   │       └── optdep feature \"cat\"
│   │           └── optdep v1.0.0
│   └── defaultdep feature \"f1\"
│       ├── defaultdep v1.0.0 (*)
│       └── defaultdep feature \"optdep\"
│           └── defaultdep v1.0.0 (*)
├── nameddep feature \"default\"
│   ├── nameddep v1.0.0
│   │   └── serde feature \"default\"
│   │       └── serde v1.0.0
│   │           └── serde_derive feature \"default\"
│   │               └── serde_derive v1.0.0
│   └── nameddep feature \"serde-stuff\"
│       ├── nameddep v1.0.0 (*)
│       ├── nameddep feature \"serde\"
│       │   └── nameddep v1.0.0 (*)
│       └── serde feature \"derive\"
│           ├── serde v1.0.0 (*)
│           └── serde feature \"serde_derive\"
│               └── serde v1.0.0 (*)
├── nameddep feature \"serde\" (*)
└── nameddep feature \"vehicle\"
    ├── nameddep v1.0.0 (*)
    └── nameddep feature \"car\"
        └── nameddep v1.0.0 (*)
",
        )
        .run();
}

#[cargo_test]
fn graph_features_ws_interdependent() {
    // A workspace with interdependent crates.
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
            b = {path="../b", features=["feat2"]}

            [features]
            default = ["a1"]
            a1 = []
            a2 = []
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
            default = ["feat1"]
            feat1 = []
            feat2 = []
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("tree -e features")
        .with_stdout(
            "\
a v0.1.0 ([..]/foo/a)
├── b feature \"default\" (command-line)
│   ├── b v0.1.0 ([..]/foo/b)
│   └── b feature \"feat1\"
│       └── b v0.1.0 ([..]/foo/b)
└── b feature \"feat2\"
    └── b v0.1.0 ([..]/foo/b)

b v0.1.0 ([..]/foo/b)
",
        )
        .run();

    p.cargo("tree -e features -i a -i b")
        .with_stdout(
            "\
a v0.1.0 ([..]/foo/a)
├── a feature \"a1\"
│   └── a feature \"default\" (command-line)
└── a feature \"default\" (command-line)

b v0.1.0 ([..]/foo/b)
├── b feature \"default\" (command-line)
│   └── a v0.1.0 ([..]/foo/a) (*)
├── b feature \"feat1\"
│   └── b feature \"default\" (command-line) (*)
└── b feature \"feat2\"
    └── a v0.1.0 ([..]/foo/a) (*)
",
        )
        .run();
}

#[cargo_test]
fn slash_feature_name() {
    // dep_name/feat_name syntax
    Package::new("opt", "1.0.0").feature("feat1", &[]).publish();
    Package::new("notopt", "1.0.0")
        .feature("cat", &[])
        .feature("animal", &["cat"])
        .publish();
    Package::new("opt2", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            opt = {version = "1.0", optional=true}
            opt2 = {version = "1.0", optional=true}
            notopt = "1.0"

            [features]
            f1 = ["opt/feat1", "notopt/animal"]
            f2 = ["f1"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree -e features --features f1")
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo)
├── notopt feature \"default\"
│   └── notopt v1.0.0
└── opt feature \"default\"
    └── opt v1.0.0
",
        )
        .run();

    p.cargo("tree -e features --features f1 -i foo")
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo)
├── foo feature \"default\" (command-line)
├── foo feature \"f1\" (command-line)
└── foo feature \"opt\"
    └── foo feature \"f1\" (command-line)
",
        )
        .run();

    p.cargo("tree -e features --features f1 -i notopt")
        .with_stdout(
            "\
notopt v1.0.0
├── notopt feature \"animal\"
│   └── foo feature \"f1\" (command-line)
├── notopt feature \"cat\"
│   └── notopt feature \"animal\" (*)
└── notopt feature \"default\"
    └── foo v0.1.0 ([..]/foo)
        ├── foo feature \"default\" (command-line)
        ├── foo feature \"f1\" (command-line)
        └── foo feature \"opt\"
            └── foo feature \"f1\" (command-line)
",
        )
        .run();

    p.cargo("tree -e features --features notopt/animal -i notopt")
        .with_stdout(
            "\
notopt v1.0.0
├── notopt feature \"animal\" (command-line)
├── notopt feature \"cat\"
│   └── notopt feature \"animal\" (command-line)
└── notopt feature \"default\"
    └── foo v0.1.0 ([..]/foo)
        └── foo feature \"default\" (command-line)
",
        )
        .run();

    p.cargo("tree -e features --all-features")
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo)
├── notopt feature \"default\"
│   └── notopt v1.0.0
├── opt feature \"default\"
│   └── opt v1.0.0
└── opt2 feature \"default\"
    └── opt2 v1.0.0
",
        )
        .run();

    p.cargo("tree -e features --all-features -i opt2")
        .with_stdout(
            "\
opt2 v1.0.0
└── opt2 feature \"default\"
    └── foo v0.1.0 ([..]/foo)
        ├── foo feature \"default\" (command-line)
        ├── foo feature \"f1\" (command-line)
        │   └── foo feature \"f2\" (command-line)
        ├── foo feature \"f2\" (command-line)
        ├── foo feature \"opt\" (command-line)
        │   └── foo feature \"f1\" (command-line) (*)
        └── foo feature \"opt2\" (command-line)
",
        )
        .run();
}

#[cargo_test]
fn features_enables_inactive_target() {
    // Features that enable things on targets that are not enabled.
    Package::new("optdep", "1.0.0")
        .feature("feat1", &[])
        .publish();
    Package::new("dep1", "1.0.0")
        .feature("somefeat", &[])
        .publish();
    Package::new("dep2", "1.0.0")
        .add_dep(
            Dependency::new("optdep", "1.0.0")
                .optional(true)
                .target("cfg(whatever)"),
        )
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [target.'cfg(whatever)'.dependencies]
            optdep = {version="1.0", optional=true}
            dep1 = "1.0"

            [dependencies]
            dep2 = "1.0"

            [features]
            f1 = ["optdep"]
            f2 = ["optdep/feat1"]
            f3 = ["dep1/somefeat"]
            f4 = ["dep2/optdep"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("tree -e features")
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo)
└── dep2 feature \"default\"
    └── dep2 v1.0.0
",
        )
        .run();
    p.cargo("tree -e features --all-features")
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo)
└── dep2 feature \"default\"
    └── dep2 v1.0.0
",
        )
        .run();
    p.cargo("tree -e features --all-features --target=all")
        .with_stdout(
            "\
foo v0.1.0 ([..]/foo)
├── dep1 feature \"default\"
│   └── dep1 v1.0.0
├── dep2 feature \"default\"
│   └── dep2 v1.0.0
│       └── optdep feature \"default\"
│           └── optdep v1.0.0
└── optdep feature \"default\" (*)
",
        )
        .run();
}
