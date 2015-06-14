use std::fs::File;
use std::io::prelude::*;

use support::{project, execs};
use support::{COMPILING, FRESH};
use support::paths::CargoPathExt;
use hamcrest::assert_that;

fn setup() {
}

test!(invalid1 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            bar = ["baz"]
        "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(&format!("\
failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `baz` which is neither a dependency nor another feature
")));
});

test!(invalid2 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            bar = ["baz"]

            [dependencies.bar]
            path = "foo"
        "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(&format!("\
failed to parse manifest at `[..]`

Caused by:
  Features and dependencies cannot have the same name: `bar`
")));
});

test!(invalid3 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            bar = ["baz"]

            [dependencies.baz]
            path = "foo"
        "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(&format!("\
failed to parse manifest at `[..]`

Caused by:
  Feature `bar` depends on `baz` which is not an optional dependency.
Consider adding `optional = true` to the dependency
")));
});

test!(invalid4 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
            features = ["bar"]
        "#)
        .file("src/main.rs", "")
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(&format!("\
Package `bar v0.0.1 ([..])` does not have these features: `bar`
")));

    let p = p.file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#);

    assert_that(p.cargo_process("build").arg("--features").arg("test"),
                execs().with_status(101).with_stderr(&format!("\
Package `foo v0.0.1 ([..])` does not have these features: `test`
")));
});

test!(invalid5 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dev-dependencies.bar]
            path = "bar"
            optional = true
        "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(&format!("\
failed to parse manifest at `[..]`

Caused by:
  Dev-dependencies are not allowed to be optional: `bar`
")));
});

test!(invalid6 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            foo = ["bar/baz"]
        "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build").arg("--features").arg("foo"),
                execs().with_status(101).with_stderr(&format!("\
failed to parse manifest at `[..]`

Caused by:
  Feature `foo` requires `bar` which is not an optional dependency
")));
});

test!(invalid7 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            foo = ["bar/baz"]
            bar = []
        "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build").arg("--features").arg("foo"),
                execs().with_status(101).with_stderr(&format!("\
failed to parse manifest at `[..]`

Caused by:
  Feature `foo` requires `bar` which is not an optional dependency
")));
});

test!(invalid8 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
            features = ["foo/bar"]
        "#)
        .file("src/main.rs", "")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("--features").arg("foo"),
                execs().with_status(101).with_stderr(&format!("\
features in dependencies cannot enable features in other dependencies: `foo/bar`
")));
});

test!(no_feature_doesnt_build {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
            optional = true
        "#)
        .file("src/main.rs", r#"
            #[cfg(feature = "bar")]
            extern crate bar;
            #[cfg(feature = "bar")]
            fn main() { bar::bar(); println!("bar") }
            #[cfg(not(feature = "bar"))]
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url())));
    assert_that(p.process(&p.bin("foo")),
                execs().with_status(0).with_stdout(""));

    assert_that(p.cargo("build").arg("--features").arg("bar"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url())));
    assert_that(p.process(&p.bin("foo")),
                execs().with_status(0).with_stdout("bar\n"));
});

test!(default_feature_pulled_in {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["bar"]

            [dependencies.bar]
            path = "bar"
            optional = true
        "#)
        .file("src/main.rs", r#"
            #[cfg(feature = "bar")]
            extern crate bar;
            #[cfg(feature = "bar")]
            fn main() { bar::bar(); println!("bar") }
            #[cfg(not(feature = "bar"))]
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url())));
    assert_that(p.process(&p.bin("foo")),
                execs().with_status(0).with_stdout("bar\n"));

    assert_that(p.cargo("build").arg("--no-default-features"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url())));
    assert_that(p.process(&p.bin("foo")),
                execs().with_status(0).with_stdout(""));
});

test!(cyclic_feature {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["default"]
        "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
Cyclic feature dependency: feature `default` depends on itself
"));
});

test!(cyclic_feature2 {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            foo = ["bar"]
            bar = ["foo"]
        "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
Cyclic feature dependency: feature `[..]` depends on itself
"));
});

test!(groups_on_groups_on_groups {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            extern crate baz;
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []
        "#)
        .file("baz/src/lib.rs", "pub fn baz() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} ba[..] v0.0.1 ({dir})
{compiling} ba[..] v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url())));
});

test!(many_cli_features {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            extern crate baz;
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []
        "#)
        .file("baz/src/lib.rs", "pub fn baz() {}");

    assert_that(p.cargo_process("build").arg("--features").arg("bar baz"),
                execs().with_status(0).with_stdout(format!("\
{compiling} ba[..] v0.0.1 ({dir})
{compiling} ba[..] v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url())));
});

test!(union_features {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/main.rs", r#"
            extern crate d1;
            extern crate d2;
            fn main() {
                d2::f1();
                d2::f2();
            }
        "#)
        .file("d1/Cargo.toml", r#"
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
        "#)
        .file("d1/src/lib.rs", "")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.1"
            authors = []

            [features]
            f1 = []
            f2 = []
        "#)
        .file("d2/src/lib.rs", r#"
            #[cfg(feature = "f1")] pub fn f1() {}
            #[cfg(feature = "f2")] pub fn f2() {}
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} d2 v0.0.1 ({dir})
{compiling} d1 v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url())));
});

test!(many_features_no_rebuilds {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name    = "b"
            version = "0.1.0"
            authors = []

            [dependencies.a]
            path = "a"
            features = ["fall"]
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name    = "a"
            version = "0.1.0"
            authors = []

            [features]
            ftest  = []
            ftest2 = []
            fall   = ["ftest", "ftest2"]
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} a v0.1.0 ({dir})
{compiling} b v0.1.0 ({dir})
", compiling = COMPILING, dir = p.url())));
    p.root().move_into_the_past().unwrap();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout(format!("\
{fresh} a v0.1.0 ([..])
{fresh} b v0.1.0 ([..])
", fresh = FRESH)));
});

// Tests that all cmd lines work with `--features ""`
test!(empty_features {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("--features").arg(""),
                execs().with_status(0));
});

// Tests that all cmd lines work with `--features ""`
test!(transitive_features {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            foo = ["bar/baz"]

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", "
            extern crate bar;
            fn main() { bar::baz(); }
        ")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [features]
            baz = []
        "#)
        .file("bar/src/lib.rs", r#"
            #[cfg(feature = "baz")]
            pub fn baz() {}
        "#);

    assert_that(p.cargo_process("build").arg("--features").arg("foo"),
                execs().with_status(0));
});

test!(everything_in_the_lockfile {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.1"
            authors = []

            [features]
            f1 = []
        "#)
        .file("d1/src/lib.rs", "")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.2"
            authors = []
        "#)
        .file("d2/src/lib.rs", "")
        .file("d3/Cargo.toml", r#"
            [package]
            name = "d3"
            version = "0.0.3"
            authors = []

            [features]
            f3 = []
        "#)
        .file("d3/src/lib.rs", "");

    assert_that(p.cargo_process("fetch"), execs().with_status(0));
    let loc = p.root().join("Cargo.lock");
    let mut lockfile = String::new();
    File::open(&loc).unwrap().read_to_string(&mut lockfile).unwrap();
    assert!(lockfile.contains(r#"name = "d1""#), "d1 not found\n{}", lockfile);
    assert!(lockfile.contains(r#"name = "d2""#), "d2 not found\n{}", lockfile);
    assert!(lockfile.contains(r#"name = "d3""#), "d3 not found\n{}", lockfile);
});

test!(no_rebuild_when_frobbing_default_feature {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            a = { path = "a" }
            b = { path = "b" }
        "#)
        .file("src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.1.0"
            authors = []

            [dependencies]
            a = { path = "../a", features = ["f1"], default-features = false }
        "#)
        .file("b/src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.1.0"
            authors = []

            [features]
            default = ["f1"]
            f1 = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
});

test!(unions_work_with_no_default_features {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            a = { path = "a" }
            b = { path = "b" }
        "#)
        .file("src/lib.rs", r#"
            extern crate a;
            pub fn foo() { a::a(); }
        "#)
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.1.0"
            authors = []

            [dependencies]
            a = { path = "../a", features = [], default-features = false }
        "#)
        .file("b/src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.1.0"
            authors = []

            [features]
            default = ["f1"]
            f1 = []
        "#)
        .file("a/src/lib.rs", r#"
            #[cfg(feature = "f1")]
            pub fn a() {}
        "#);

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
});
