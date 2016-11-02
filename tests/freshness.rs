extern crate cargotest;
extern crate hamcrest;

use std::fs::{self, File};
use std::io::prelude::*;

use cargotest::sleep_ms;
use cargotest::support::{project, execs, path2url};
use cargotest::support::paths::CargoPathExt;
use hamcrest::{assert_that, existing_file};

#[test]
fn modifying_and_moving() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", r#"
            mod a; fn main() {}
        "#)
        .file("src/a.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = path2url(p.root()))));

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(""));
    p.root().move_into_the_past();
    p.root().join("target").move_into_the_past();

    File::create(&p.root().join("src/a.rs")).unwrap()
         .write_all(b"#[allow(unused)]fn main() {}").unwrap();
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = path2url(p.root()))));

    fs::rename(&p.root().join("src/a.rs"), &p.root().join("src/b.rs")).unwrap();
    assert_that(p.cargo("build"),
                execs().with_status(101));
}

#[test]
fn modify_only_some_files() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "mod a;")
        .file("src/a.rs", "")
        .file("src/main.rs", r#"
            mod b;
            fn main() {}
        "#)
        .file("src/b.rs", "")
        .file("tests/test.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = path2url(p.root()))));
    assert_that(p.cargo("test"),
                execs().with_status(0));
    sleep_ms(1000);

    assert_that(&p.bin("foo"), existing_file());

    let lib = p.root().join("src/lib.rs");
    let bin = p.root().join("src/b.rs");

    File::create(&lib).unwrap().write_all(b"invalid rust code").unwrap();
    File::create(&bin).unwrap().write_all(b"#[allow(unused)]fn foo() {}").unwrap();
    lib.move_into_the_past();

    // Make sure the binary is rebuilt, not the lib
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = path2url(p.root()))));
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn rebuild_sub_package_then_while_package() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"

            [dependencies.a]
            path = "a"
            [dependencies.b]
            path = "b"
        "#)
        .file("src/lib.rs", "extern crate a; extern crate b;")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            authors = []
            version = "0.0.1"
            [dependencies.b]
            path = "../b"
        "#)
        .file("a/src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            authors = []
            version = "0.0.1"
        "#)
        .file("b/src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    File::create(&p.root().join("b/src/lib.rs")).unwrap().write_all(br#"
        pub fn b() {}
    "#).unwrap();

    assert_that(p.cargo("build").arg("-pb"),
                execs().with_status(0));

    File::create(&p.root().join("src/lib.rs")).unwrap().write_all(br#"
        extern crate a;
        extern crate b;
        pub fn toplevel() {}
    "#).unwrap();

    assert_that(p.cargo("build"),
                execs().with_status(0));
}

#[test]
fn changing_features_is_ok() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"

            [features]
            foo = []
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stderr("\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build").arg("--features").arg("foo"),
                execs().with_status(0)
                       .with_stderr("\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stderr("\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stdout(""));
}

#[test]
fn rebuild_tests_if_lib_changes() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}")
        .file("tests/foo.rs", r#"
            extern crate foo;
            #[test]
            fn test() { foo::foo(); }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.cargo("test"),
                execs().with_status(0));

    File::create(&p.root().join("src/lib.rs")).unwrap();
    p.root().move_into_the_past();
    p.root().join("target").move_into_the_past();

    assert_that(p.cargo("build"),
                execs().with_status(0));
    assert_that(p.cargo("test").arg("-v"),
                execs().with_status(101));
}

#[test]
fn no_rebuild_transitive_target_deps() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
            [dev-dependencies]
            b = { path = "b" }
        "#)
        .file("src/lib.rs", "")
        .file("tests/foo.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [target.foo.dependencies]
            c = { path = "../c" }
        "#)
        .file("a/src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.0.1"
            authors = []

            [dependencies]
            c = { path = "../c" }
        "#)
        .file("b/src/lib.rs", "")
        .file("c/Cargo.toml", r#"
            [package]
            name = "c"
            version = "0.0.1"
            authors = []
        "#)
        .file("c/src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.cargo("test").arg("--no-run"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] c v0.0.1 ([..])
[COMPILING] b v0.0.1 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn rerun_if_changed_in_dep() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("a/build.rs", r#"
            fn main() {
                println!("cargo:rerun-if-changed=build.rs");
            }
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(""));
}

#[test]
fn same_build_dir_cached_packages() {
    let p = project("foo")
        .file("a1/Cargo.toml", r#"
            [package]
            name = "a1"
            version = "0.0.1"
            authors = []
            [dependencies]
            b = { path = "../b" }
        "#)
        .file("a1/src/lib.rs", "")
        .file("a2/Cargo.toml", r#"
            [package]
            name = "a2"
            version = "0.0.1"
            authors = []
            [dependencies]
            b = { path = "../b" }
        "#)
        .file("a2/src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.0.1"
            authors = []
            [dependencies]
            c = { path = "../c" }
        "#)
        .file("b/src/lib.rs", "")
        .file("c/Cargo.toml", r#"
            [package]
            name = "c"
            version = "0.0.1"
            authors = []
            [dependencies]
            d = { path = "../d" }
        "#)
        .file("c/src/lib.rs", "")
        .file("d/Cargo.toml", r#"
            [package]
            name = "d"
            version = "0.0.1"
            authors = []
        "#)
        .file("d/src/lib.rs", "")
        .file(".cargo/config", r#"
            [build]
            target-dir = "./target"
        "#);
    p.build();

    assert_that(p.cargo("build").cwd(p.root().join("a1")),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] d v0.0.1 ({dir}/d)
[COMPILING] c v0.0.1 ({dir}/c)
[COMPILING] b v0.0.1 ({dir}/b)
[COMPILING] a1 v0.0.1 ({dir}/a1)
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
    assert_that(p.cargo("build").cwd(p.root().join("a2")),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] a2 v0.0.1 ({dir}/a2)
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.url())));
}

#[test]
fn no_rebuild_if_build_artifacts_move_backwards_in_time() {
    let p = project("backwards_in_time")
        .file("Cargo.toml", r#"
            [package]
            name = "backwards_in_time"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    p.root().move_into_the_past();
    p.root().join("target").move_into_the_past();

    assert_that(p.cargo("build").env("RUST_LOG", ""),
                execs().with_status(0).with_stdout("").with_stderr(""));
}

#[test]
fn rebuild_if_build_artifacts_move_forward_in_time() {
    let p = project("forwards_in_time")
        .file("Cargo.toml", r#"
            [package]
            name = "forwards_in_time"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    p.root().move_into_the_future();
    p.root().join("target").move_into_the_future();

    assert_that(p.cargo("build").env("RUST_LOG", ""),
                execs().with_status(0).with_stdout("").with_stderr("\
[COMPILING] a v0.0.1 ([..])
[COMPILING] forwards_in_time v0.0.1 ([..])
"));
}
