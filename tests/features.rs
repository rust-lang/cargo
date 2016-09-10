#[macro_use]
extern crate cargotest;
extern crate hamcrest;

use std::fs::File;
use std::io::prelude::*;

use cargotest::support::paths::CargoPathExt;
use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn invalid1() {
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
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` includes `baz` which is neither a dependency nor another feature
"));
}

#[test]
fn invalid2() {
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
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Features and dependencies cannot have the same name: `bar`
"));
}

#[test]
fn invalid3() {
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
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `bar` depends on `baz` which is not an optional dependency.
Consider adding `optional = true` to the dependency
"));
}

#[test]
fn invalid4() {
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
                execs().with_status(101).with_stderr("\
[ERROR] Package `bar v0.0.1 ([..])` does not have these features: `bar`
"));

    let p = p.file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#);

    assert_that(p.cargo_process("build").arg("--features").arg("test"),
                execs().with_status(101).with_stderr("\
[ERROR] Package `foo v0.0.1 ([..])` does not have these features: `test`
"));
}

#[test]
fn invalid5() {
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
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Dev-dependencies are not allowed to be optional: `bar`
"));
}

#[test]
fn invalid6() {
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
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `foo` requires `bar` which is not an optional dependency
"));
}

#[test]
fn invalid7() {
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
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Feature `foo` requires `bar` which is not an optional dependency
"));
}

#[test]
fn invalid8() {
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
                execs().with_status(101).with_stderr("\
[ERROR] feature names may not contain slashes: `foo/bar`
"));
}

#[test]
fn no_transitive_dep_feature_requirement() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.derived]
            path = "derived"

            [features]
            default = ["derived/bar/qux"]
        "#)
        .file("src/main.rs", r#"
            extern crate derived;
            fn main() { derived::test(); }
        "#)
        .file("derived/Cargo.toml", r#"
            [package]
            name = "derived"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("derived/src/lib.rs", r#"
            extern crate bar;
            pub use bar::test;
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [features]
            qux = []
        "#)
        .file("bar/src/lib.rs", r#"
            #[cfg(feature = "qux")]
            pub fn test() { print!("test"); }
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
[ERROR] feature names may not contain slashes: `bar/qux`
"));
}

#[test]
fn no_feature_doesnt_build() {
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
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
    assert_that(p.process(&p.bin("foo")),
                execs().with_status(0).with_stdout(""));

    assert_that(p.cargo("build").arg("--features").arg("bar"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] bar v0.0.1 ({dir}/bar)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
    assert_that(p.process(&p.bin("foo")),
                execs().with_status(0).with_stdout("bar\n"));
}

#[test]
fn default_feature_pulled_in() {
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
                execs().with_status(0).with_stderr(format!("\
[COMPILING] bar v0.0.1 ({dir}/bar)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
    assert_that(p.process(&p.bin("foo")),
                execs().with_status(0).with_stdout("bar\n"));

    assert_that(p.cargo("build").arg("--no-default-features"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
    assert_that(p.process(&p.bin("foo")),
                execs().with_status(0).with_stdout(""));
}

#[test]
fn cyclic_feature() {
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
[ERROR] Cyclic feature dependency: feature `default` depends on itself
"));
}

#[test]
fn cyclic_feature2() {
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
[ERROR] Cyclic feature dependency: feature `[..]` depends on itself
"));
}

#[test]
fn groups_on_groups_on_groups() {
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
                execs().with_status(0).with_stderr(format!("\
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
}

#[test]
fn many_cli_features() {
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
                execs().with_status(0).with_stderr(format!("\
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] ba[..] v0.0.1 ({dir}/ba[..])
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
}

#[test]
fn union_features() {
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
                execs().with_status(0).with_stderr(format!("\
[COMPILING] d2 v0.0.1 ({dir}/d2)
[COMPILING] d1 v0.0.1 ({dir}/d1)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
}

#[test]
fn many_features_no_rebuilds() {
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
                execs().with_status(0).with_stderr(format!("\
[COMPILING] a v0.1.0 ({dir}/a)
[COMPILING] b v0.1.0 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
    p.root().move_into_the_past();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[FRESH] a v0.1.0 ([..]/a)
[FRESH] b v0.1.0 ([..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

// Tests that all cmd lines work with `--features ""`
#[test]
fn empty_features() {
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
}

// Tests that all cmd lines work with `--features ""`
#[test]
fn transitive_features() {
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
}

#[test]
fn everything_in_the_lockfile() {
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
    t!(t!(File::open(&loc)).read_to_string(&mut lockfile));
    assert!(lockfile.contains(r#"name = "d1""#), "d1 not found\n{}", lockfile);
    assert!(lockfile.contains(r#"name = "d2""#), "d2 not found\n{}", lockfile);
    assert!(lockfile.contains(r#"name = "d3""#), "d3 not found\n{}", lockfile);
}

#[test]
fn no_rebuild_when_frobbing_default_feature() {
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
}

#[test]
fn unions_work_with_no_default_features() {
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
}

#[test]
fn optional_and_dev_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name    = "test"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = { path = "foo", optional = true }
            [dev-dependencies]
            foo = { path = "foo" }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[COMPILING] test v0.1.0 ([..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn activating_feature_activates_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name    = "test"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = { path = "foo", optional = true }

            [features]
            a = ["foo/a"]
        "#)
        .file("src/lib.rs", "
            extern crate foo;
            pub fn bar() {
                foo::bar();
            }
        ")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [features]
            a = []
        "#)
        .file("foo/src/lib.rs", r#"
            #[cfg(feature = "a")]
            pub fn bar() {}
        "#);

    assert_that(p.cargo_process("build").arg("--features").arg("a").arg("-v"),
                execs().with_status(0));
}

#[test]
fn dep_feature_in_cmd_line() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.derived]
            path = "derived"
        "#)
        .file("src/main.rs", r#"
            extern crate derived;
            fn main() { derived::test(); }
        "#)
        .file("derived/Cargo.toml", r#"
            [package]
            name = "derived"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"

            [features]
            default = []
            derived-feat = ["bar/some-feat"]
        "#)
        .file("derived/src/lib.rs", r#"
            extern crate bar;
            pub use bar::test;
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [features]
            some-feat = []
        "#)
        .file("bar/src/lib.rs", r#"
            #[cfg(feature = "some-feat")]
            pub fn test() { print!("test"); }
        "#);

    // The foo project requires that feature "some-feat" in "bar" is enabled.
    // Building without any features enabled should fail:
    assert_that(p.cargo_process("build"),
                execs().with_status(101));

    // We should be able to enable the feature "derived-feat", which enables "some-feat",
    // on the command line. The feature is enabled, thus building should be successful:
    assert_that(p.cargo_process("build").arg("--features").arg("derived/derived-feat"),
                execs().with_status(0));

    // Trying to enable features of transitive dependencies is an error
    assert_that(p.cargo_process("build").arg("--features").arg("bar/some-feat"),
                execs().with_status(101).with_stderr("\
[ERROR] Package `foo v0.0.1 ([..])` does not have these features: `bar`
"));

    // Hierarchical feature specification should still be disallowed
    assert_that(p.cargo_process("build").arg("--features").arg("derived/bar/some-feat"),
                execs().with_status(101).with_stderr("\
[ERROR] feature names may not contain slashes: `bar/some-feat`
"));
}

#[test]
fn all_features_flag_enables_all_features() {
    let p = project("foo")
        .file("Cargo.toml", r#"
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
        "#)
        .file("src/main.rs", r#"
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
        "#)
        .file("baz/Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []
        "#)
        .file("baz/src/lib.rs", "pub fn baz() {}");

    assert_that(p.cargo_process("build").arg("--all-features"),
                execs().with_status(0));
}
