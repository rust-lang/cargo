use support::{project, execs, cargo_dir};
use support::{COMPILING, FRESH};
use support::paths::PathExt;
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
                execs().with_status(101).with_stderr(format!("\
Cargo.toml is not a valid manifest

Feature `bar` includes `baz` which is neither a dependency nor another feature
").as_slice()));
})

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
                execs().with_status(101).with_stderr(format!("\
Cargo.toml is not a valid manifest

Features and dependencies cannot have the same name: `bar`
").as_slice()));
})

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
                execs().with_status(101).with_stderr(format!("\
Cargo.toml is not a valid manifest

Feature `bar` depends on `baz` which is not an optional dependency.
Consider adding `optional = true` to the dependency
").as_slice()));
})

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
                execs().with_status(101).with_stderr(format!("\
Package `bar v0.0.1 ([..])` does not have these features: `bar`
").as_slice()));

    let p = p.file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#);

    assert_that(p.cargo_process("build").arg("--features").arg("test"),
                execs().with_status(101).with_stderr(format!("\
Package `foo v0.0.1 ([..])` does not have these features: `test`
").as_slice()));
})

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
                execs().with_status(101).with_stderr(format!("\
Cargo.toml is not a valid manifest

Dev-dependencies are not allowed to be optional: `bar`
").as_slice()));
})

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
                execs().with_status(101).with_stderr(format!("\
Cargo.toml is not a valid manifest

Feature `foo` requires `bar` which is not an optional dependency
").as_slice()));
})

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
                execs().with_status(101).with_stderr(format!("\
Cargo.toml is not a valid manifest

Feature `foo` requires `bar` which is not an optional dependency
").as_slice()));
})

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
                execs().with_status(101).with_stderr(format!("\
features in dependencies cannot enable features in other dependencies: `foo/bar`
").as_slice()));
})

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
", compiling = COMPILING, dir = p.url()).as_slice()));
    assert_that(p.process(p.bin("foo")), execs().with_status(0).with_stdout(""));

    assert_that(p.process(cargo_dir().join("cargo")).arg("build")
                 .arg("--features").arg("bar"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url()).as_slice()));
    assert_that(p.process(p.bin("foo")),
                execs().with_status(0).with_stdout("bar\n"));
})

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
", compiling = COMPILING, dir = p.url()).as_slice()));
    assert_that(p.process(p.bin("foo")),
                execs().with_status(0).with_stdout("bar\n"));

    assert_that(p.process(cargo_dir().join("cargo")).arg("build")
                 .arg("--no-default-features"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = p.url()).as_slice()));
    assert_that(p.process(p.bin("foo")), execs().with_status(0).with_stdout(""));
})

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
})

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
})

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
", compiling = COMPILING, dir = p.url()).as_slice()));
})

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
", compiling = COMPILING, dir = p.url()).as_slice()));
})

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
", compiling = COMPILING, dir = p.url()).as_slice()));
})

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
", compiling = COMPILING, dir = p.url()).as_slice()));
    p.root().move_into_the_past().unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("build").arg("-v"),
                execs().with_status(0).with_stdout(format!("\
{fresh} a v0.1.0 ([..])
{fresh} b v0.1.0 ([..])
", fresh = FRESH).as_slice()));
})

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
})

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
})
