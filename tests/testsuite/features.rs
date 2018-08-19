use std::fs::File;
use std::io::prelude::*;

use support::paths::CargoPathExt;
use support::{basic_manifest, execs, project};
use support::ChannelChanger;
use support::hamcrest::assert_that;
use support::registry::Package;

#[test]
fn invalid1() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            bar = ["baz"]
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `baz` which is neither a dependency nor another feature
",
        ),
    );
}

#[test]
fn invalid2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            bar = ["baz"]

            [dependencies.bar]
            path = "foo"
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Features and dependencies cannot have the same name: `bar`
",
        ),
    );
}

#[test]
fn invalid3() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            bar = ["baz"]

            [dependencies.baz]
            path = "foo"
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` depends on `baz` which is not an optional dependency.
Consider adding `optional = true` to the dependency
",
        ),
    );
}

#[test]
fn invalid4() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: failed to select a version for `bar`.
    ... required by package `foo v0.0.1 ([..])`
versions that meet the requirements `*` are: 0.0.1

the package `foo` depends on `bar`, with features: `bar` but `bar` does not have these features.


failed to select a version for `bar` which could resolve this conflict",
        ),
    );

    p.change_file("Cargo.toml", &basic_manifest("foo", "0.0.1"));

    assert_that(
        p.cargo("build --features test"),
        execs()
            .with_status(101)
            .with_stderr("error: Package `foo v0.0.1 ([..])` does not have these features: `test`"),
    );
}

#[test]
fn invalid5() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dev-dependencies.bar]
            path = "bar"
            optional = true
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Dev-dependencies are not allowed to be optional: `bar`
",
        ),
    );
}

#[test]
fn invalid6() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            foo = ["bar/baz"]
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build --features foo"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `foo` requires a feature of `bar` which is not a dependency
",
        ),
    );
}

#[test]
fn invalid7() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            foo = ["bar/baz"]
            bar = []
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build --features foo"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `foo` requires a feature of `bar` which is not a dependency
",
        ),
    );
}

#[test]
fn invalid8() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build --features foo"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] feature names may not contain slashes: `foo/bar`"),
    );
}

#[test]
fn invalid9() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build --features bar"),
                execs().with_stderr("\
warning: Package `foo v0.0.1 ([..])` does not have feature `bar`. It has a required dependency with \
that name, but only optional dependencies can be used as features. [..]
   Compiling bar v0.0.1 ([..])
   Compiling foo v0.0.1 ([..])
    Finished dev [unoptimized + debuginfo] target(s) in [..]s
"));
}

#[test]
fn invalid10() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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
            authors = []

            [dependencies.baz]
            path = "baz"
        "#,
        )
        .file("bar/src/lib.rs", "")
        .file("bar/baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("bar/baz/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"),
                execs().with_stderr("\
warning: Package `bar v0.0.1 ([..])` does not have feature `baz`. It has a required dependency with \
that name, but only optional dependencies can be used as features. [..]
   Compiling baz v0.0.1 ([..])
   Compiling bar v0.0.1 ([..])
   Compiling foo v0.0.1 ([..])
    Finished dev [unoptimized + debuginfo] target(s) in [..]s
"));
}

#[test]
fn no_transitive_dep_feature_requirement() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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
    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] feature names may not contain slashes: `bar/qux`"),
    );
}

#[test]
fn no_feature_doesnt_build() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
    assert_that(
        p.process(&p.bin("foo")),
        execs().with_stdout(""),
    );

    assert_that(
        p.cargo("build --features bar"),
        execs().with_stderr(format!(
            "\
[COMPILING] bar v0.0.1 ({dir}/bar)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
    assert_that(
        p.process(&p.bin("foo")),
        execs().with_stdout("bar\n"),
    );
}

#[test]
fn default_feature_pulled_in() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(format!(
            "\
[COMPILING] bar v0.0.1 ({dir}/bar)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
    assert_that(
        p.process(&p.bin("foo")),
        execs().with_stdout("bar\n"),
    );

    assert_that(
        p.cargo("build --no-default-features"),
        execs().with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
    assert_that(
        p.process(&p.bin("foo")),
        execs().with_stdout(""),
    );
}

#[test]
fn cyclic_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["default"]
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] Cyclic feature dependency: feature `default` depends on itself"),
    );
}

#[test]
fn cyclic_feature2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            foo = ["bar"]
            bar = ["foo"]
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("build"), execs().with_stdout(""));
}

#[test]
fn groups_on_groups_on_groups() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(format!(
            "\
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
}

#[test]
fn many_cli_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build --features").arg("bar baz"),
        execs().with_stderr(format!(
            "\
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
}

#[test]
fn union_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(format!(
            "\
[COMPILING] d2 v0.0.1 ({dir}/d2)
[COMPILING] d1 v0.0.1 ({dir}/d1)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
}

#[test]
fn many_features_no_rebuilds() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name    = "b"
            version = "0.1.0"
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
            authors = []

            [features]
            ftest  = []
            ftest2 = []
            fall   = ["ftest", "ftest2"]
        "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(format!(
            "\
[COMPILING] a v0.1.0 ({dir}/a)
[COMPILING] b v0.1.0 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
    p.root().move_into_the_past();

    assert_that(
        p.cargo("build -v"),
        execs().with_stderr(
            "\
[FRESH] a v0.1.0 ([..]/a)
[FRESH] b v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

// Tests that all cmd lines work with `--features ""`
#[test]
fn empty_features() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --features").arg(""),
        execs(),
    );
}

// Tests that all cmd lines work with `--features ""`
#[test]
fn transitive_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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
            authors = []

            [features]
            baz = []
        "#,
        )
        .file("bar/src/lib.rs", r#"#[cfg(feature = "baz")] pub fn baz() {}"#)
        .build();

    assert_that(
        p.cargo("build --features foo"),
        execs(),
    );
}

#[test]
fn everything_in_the_lockfile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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
            authors = []

            [features]
            f3 = []
        "#,
        )
        .file("d3/src/lib.rs", "")
        .build();

    assert_that(p.cargo("fetch"), execs());
    let loc = p.root().join("Cargo.lock");
    let mut lockfile = String::new();
    t!(t!(File::open(&loc)).read_to_string(&mut lockfile));
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

#[test]
fn no_rebuild_when_frobbing_default_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
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
            authors = []

            [features]
            default = ["f1"]
            f1 = []
        "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(p.cargo("build"), execs().with_stdout(""));
    assert_that(p.cargo("build"), execs().with_stdout(""));
}

#[test]
fn unions_work_with_no_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
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
            authors = []

            [features]
            default = ["f1"]
            f1 = []
        "#,
        )
        .file("a/src/lib.rs", r#"#[cfg(feature = "f1")] pub fn a() {}"#)
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(p.cargo("build"), execs().with_stdout(""));
    assert_that(p.cargo("build"), execs().with_stdout(""));
}

#[test]
fn optional_and_dev_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name    = "test"
            version = "0.1.0"
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[COMPILING] test v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn activating_feature_activates_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name    = "test"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = { path = "foo", optional = true }

            [features]
            a = ["foo/a"]
        "#,
        )
        .file("src/lib.rs", "extern crate foo; pub fn bar() { foo::bar(); }")
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [features]
            a = []
        "#,
        )
        .file("foo/src/lib.rs", r#"#[cfg(feature = "a")] pub fn bar() {}"#)
        .build();

    assert_that(
        p.cargo("build --features a -v"),
        execs(),
    );
}

#[test]
fn dep_feature_in_cmd_line() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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
    assert_that(p.cargo("build"), execs().with_status(101));

    // We should be able to enable the feature "derived-feat", which enables "some-feat",
    // on the command line. The feature is enabled, thus building should be successful:
    assert_that(
        p.cargo("build --features derived/derived-feat"),
        execs(),
    );

    // Trying to enable features of transitive dependencies is an error
    assert_that(
        p.cargo("build --features bar/some-feat"),
        execs()
            .with_status(101)
            .with_stderr("error: Package `foo v0.0.1 ([..])` does not have these features: `bar`"),
    );

    // Hierarchical feature specification should still be disallowed
    assert_that(
        p.cargo("build --features derived/bar/some-feat"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] feature names may not contain slashes: `bar/some-feat`"),
    );
}

#[test]
fn all_features_flag_enables_all_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build --all-features"),
        execs(),
    );
}

#[test]
fn many_cli_features_comma_delimited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build --features bar,baz"),
        execs().with_stderr(format!(
            "\
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
}

#[test]
fn many_cli_features_comma_and_space_delimited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
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

    assert_that(
        p.cargo("build --features").arg("bar,baz bam bap"),
        execs().with_stderr(format!(
            "\
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
}

#[test]
fn combining_features_and_package() {
    Package::new("dep", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [workspace]
            members = ["bar"]

            [dependencies]
            dep = "1"
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
            [features]
            main = []
        "#,
        )
        .file(
            "bar/src/main.rs",
            r#"
            #[cfg(feature = "main")]
            fn main() {}
        "#,
        )
        .build();

    assert_that(
        p.cargo("build -Z package-features --all --features main")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr_contains(
            "\
             [ERROR] cannot specify features for more than one package",
        ),
    );

    assert_that(
        p.cargo("build -Z package-features --package dep --features main")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr_contains(
            "\
             [ERROR] cannot specify features for packages outside of workspace",
        ),
    );
    assert_that(
        p.cargo("build -Z package-features --package dep --all-features")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr_contains(
            "\
             [ERROR] cannot specify features for packages outside of workspace",
        ),
    );
    assert_that(
        p.cargo("build -Z package-features --package dep --no-default-features")
            .masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr_contains(
            "\
             [ERROR] cannot specify features for packages outside of workspace",
        ),
    );

    assert_that(
        p.cargo("build -Z package-features --all --all-features")
            .masquerade_as_nightly_cargo(),
        execs(),
    );
    assert_that(
        p.cargo("run -Z package-features --package bar --features main")
            .masquerade_as_nightly_cargo(),
        execs(),
    );
}

#[test]
fn namespaced_invalid_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["namespaced-features"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            namespaced-features = true

            [features]
            bar = ["baz"]
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `baz` which is not defined as a feature
",
        ),
    );
}

#[test]
fn namespaced_invalid_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["namespaced-features"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            namespaced-features = true

            [features]
            bar = ["crate:baz"]
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `crate:baz` which is not a known dependency
",
        ),
    );
}

#[test]
fn namespaced_non_optional_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["namespaced-features"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            namespaced-features = true

            [features]
            bar = ["crate:baz"]

            [dependencies]
            baz = "0.1"
        "#,
        )
        .file("src/main.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `crate:baz` which is not an optional dependency.
Consider adding `optional = true` to the dependency
",
        ),
    );
}

#[test]
fn namespaced_implicit_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["namespaced-features"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            namespaced-features = true

            [features]
            bar = ["baz"]

            [dependencies]
            baz = { version = "0.1", optional = true }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs(),
    );
}

#[test]
fn namespaced_shadowed_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["namespaced-features"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            namespaced-features = true

            [features]
            baz = []

            [dependencies]
            baz = { version = "0.1", optional = true }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `baz` includes the optional dependency of the same name, but this is left implicit in the features included by this feature.
Consider adding `crate:baz` to this feature's requirements.
",
        ),
    );
}

#[test]
fn namespaced_shadowed_non_optional() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["namespaced-features"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            namespaced-features = true

            [features]
            baz = []

            [dependencies]
            baz = "0.1"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `baz` includes the dependency of the same name, but this is left implicit in the features included by this feature.
Additionally, the dependency must be marked as optional to be included in the feature definition.
Consider adding `crate:baz` to this feature's requirements and marking the dependency as `optional = true`
",
        ),
    );
}

#[test]
fn namespaced_implicit_non_optional() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["namespaced-features"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            namespaced-features = true

            [features]
            bar = ["baz"]

            [dependencies]
            baz = "0.1"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `baz` which is not defined as a feature.
A non-optional dependency of the same name is defined; consider adding `optional = true` to its definition
",
        ),
    );
}

#[test]
fn namespaced_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["namespaced-features"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            namespaced-features = true

            [features]
            baz = ["crate:baz"]

            [dependencies]
            baz = { version = "0.1", optional = true }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs(),
    );
}

#[test]
fn only_dep_is_optional() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    assert_that(
        p.cargo("build"),
        execs(),
    );
}

#[test]
fn all_features_all_crates() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [workspace]
                members = ['bar']
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.0.1"
                authors = []

                [features]
                foo = []
            "#,
        )
        .file("bar/src/main.rs", "#[cfg(feature = \"foo\")] fn main() {}")
        .build();

    assert_that(
        p.cargo("build --all-features --all"),
        execs(),
    );
}
