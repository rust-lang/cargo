extern crate cargo;
#[macro_use]
extern crate cargotest;
extern crate hamcrest;

use std::fs::{self, File};
use std::io::prelude::*;

use cargo::util::process;
use cargotest::sleep_ms;
use cargotest::support::paths::{self, CargoPathExt};
use cargotest::support::{project, execs, main_file};
use cargotest::support::registry::Package;
use hamcrest::{assert_that, existing_file};

#[test]
#[cfg(not(windows))] // I have no idea why this is failing spuriously on
                     // Windows, for more info see #3466.
fn cargo_compile_with_nested_deps_shorthand() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "bar"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]

            version = "0.5.0"
            path = "baz"

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#)
        .file("bar/baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "baz"
        "#)
        .file("bar/baz/src/baz.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    assert_that(p.cargo_process("build"),
        execs().with_status(0)
               .with_stderr(&format!("[COMPILING] baz v0.5.0 ({}/bar/baz)\n\
                                     [COMPILING] bar v0.5.0 ({}/bar)\n\
                                     [COMPILING] foo v0.5.0 ({})\n\
                                     [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                     in [..]\n",
                                    p.url(),
                                    p.url(),
                                    p.url())));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("test passed\n").with_status(0));

    println!("cleaning");
    assert_that(p.cargo("clean").arg("-v"),
                execs().with_stdout("").with_status(0));
    println!("building baz");
    assert_that(p.cargo("build").arg("-p").arg("baz"),
                execs().with_status(0)
                       .with_stderr(&format!("[COMPILING] baz v0.5.0 ({}/bar/baz)\n\
                                              [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                              in [..]\n",
                                            p.url())));
    println!("building foo");
    assert_that(p.cargo("build")
                 .arg("-p").arg("foo"),
                execs().with_status(0)
                       .with_stderr(&format!("[COMPILING] bar v0.5.0 ({}/bar)\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            p.url(),
                                            p.url())));
}

#[test]
fn cargo_compile_with_root_dev_deps() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dev-dependencies.bar]

            version = "0.5.0"
            path = "../bar"

            [[bin]]
            name = "foo"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]));
    let p2 = project("bar")
        .file("Cargo.toml", r#"
            [package]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", r#"
            pub fn gimme() -> &'static str {
                "zoidberg"
            }
        "#);

    p2.build();
    assert_that(p.cargo_process("build"),
                execs().with_status(101))
}

#[test]
fn cargo_compile_with_root_dev_deps_with_testing() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dev-dependencies.bar]

            version = "0.5.0"
            path = "../bar"

            [[bin]]
            name = "foo"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]));
    let p2 = project("bar")
        .file("Cargo.toml", r#"
            [package]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", r#"
            pub fn gimme() -> &'static str {
                "zoidberg"
            }
        "#);

    p2.build();
    assert_that(p.cargo_process("test"),
                execs().with_stderr("\
[COMPILING] [..] v0.5.0 ([..])
[COMPILING] [..] v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]")
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn cargo_compile_with_transitive_dev_deps() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "bar"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dev-dependencies.baz]

            git = "git://example.com/path/to/nowhere"

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn gimme() -> &'static str {
                "zoidberg"
            }
        "#);

    assert_that(p.cargo_process("build"),
        execs().with_stderr(&format!("[COMPILING] bar v0.5.0 ({}/bar)\n\
                                     [COMPILING] foo v0.5.0 ({})\n\
                                     [FINISHED] dev [unoptimized + debuginfo] target(s) in \
                                     [..]\n",
                                    p.url(),
                                    p.url())));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("zoidberg\n"));
}

#[test]
fn no_rebuild_dependency() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]
            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn bar() {}
        "#);
    // First time around we should compile both foo and bar
    assert_that(p.cargo_process("build"),
                execs().with_stderr(&format!("[COMPILING] bar v0.5.0 ({}/bar)\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            p.url(),
                                            p.url())));

    sleep_ms(1000);
    p.change_file("src/main.rs", r#"
        extern crate bar;
        fn main() { bar::bar(); }
    "#);
    // Don't compile bar, but do recompile foo.
    assert_that(p.cargo("build"),
                execs().with_stderr("\
                     [COMPILING] foo v0.5.0 ([..])\n\
                     [FINISHED] dev [unoptimized + debuginfo] target(s) \
                     in [..]\n"));
}

#[test]
fn deep_dependencies_trigger_rebuild() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]
            name = "bar"
            [dependencies.baz]
            path = "../baz"
        "#)
        .file("bar/src/bar.rs", r#"
            extern crate baz;
            pub fn bar() { baz::baz() }
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]
            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn baz() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_stderr(&format!("[COMPILING] baz v0.5.0 ({}/baz)\n\
                                             [COMPILING] bar v0.5.0 ({}/bar)\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            p.url(),
                                            p.url(),
                                            p.url())));
    assert_that(p.cargo("build"),
                execs().with_stdout(""));

    // Make sure an update to baz triggers a rebuild of bar
    //
    // We base recompilation off mtime, so sleep for at least a second to ensure
    // that this write will change the mtime.
    sleep_ms(1000);
    File::create(&p.root().join("baz/src/baz.rs")).unwrap().write_all(br#"
        pub fn baz() { println!("hello!"); }
    "#).unwrap();
    assert_that(p.cargo("build"),
                execs().with_stderr(&format!("[COMPILING] baz v0.5.0 ({}/baz)\n\
                                             [COMPILING] bar v0.5.0 ({}/bar)\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            p.url(),
                                            p.url(),
                                            p.url())));

    // Make sure an update to bar doesn't trigger baz
    sleep_ms(1000);
    File::create(&p.root().join("bar/src/bar.rs")).unwrap().write_all(br#"
        extern crate baz;
        pub fn bar() { println!("hello!"); baz::baz(); }
    "#).unwrap();
    assert_that(p.cargo("build"),
                execs().with_stderr(&format!("[COMPILING] bar v0.5.0 ({}/bar)\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            p.url(),
                                            p.url())));

}

#[test]
fn no_rebuild_two_deps() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
            [dependencies.baz]
            path = "baz"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]
            name = "bar"
            [dependencies.baz]
            path = "../baz"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn bar() {}
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]
            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn baz() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_stderr(&format!("[COMPILING] baz v0.5.0 ({}/baz)\n\
                                             [COMPILING] bar v0.5.0 ({}/bar)\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            p.url(),
                                            p.url(),
                                            p.url())));
    assert_that(&p.bin("foo"), existing_file());
    assert_that(p.cargo("build"),
                execs().with_stdout(""));
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn nested_deps_recompile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "src/bar"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("src/bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "bar"
        "#)
        .file("src/bar/src/bar.rs", "pub fn gimme() -> i32 { 92 }");
    let bar = p.url();

    assert_that(p.cargo_process("build"),
                execs().with_stderr(&format!("[COMPILING] bar v0.5.0 ({}/src/bar)\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            bar,
                                            p.url())));
    sleep_ms(1000);

    File::create(&p.root().join("src/main.rs")).unwrap().write_all(br#"
        fn main() {}
    "#).unwrap();

    // This shouldn't recompile `bar`
    assert_that(p.cargo("build"),
                execs().with_stderr(&format!("[COMPILING] foo v0.5.0 ({})\n\
                                              [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                              in [..]\n",
                                            p.url())));
}

#[test]
fn error_message_for_missing_manifest() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            path = "src/bar"
        "#)
        .file("src/lib.rs", "")
        .file("src/bar/not-a-manifest", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] failed to load source for a dependency on `bar`

Caused by:
  Unable to update file://[..]

Caused by:
  failed to read `[..]bar[/]Cargo.toml`

Caused by:
  [..] (os error [..])
"));

}

#[test]
fn override_relative() {
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
       .file("src/lib.rs", "");

    fs::create_dir(&paths::root().join(".cargo")).unwrap();
    File::create(&paths::root().join(".cargo/config")).unwrap()
         .write_all(br#"paths = ["bar"]"#).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = '{}'
        "#, bar.root().display()))
       .file("src/lib.rs", "");
    bar.build();
    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0));

}

#[test]
fn override_self() {
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
       .file("src/lib.rs", "");

    let p = project("foo");
    let root = p.root().clone();
    let p = p
        .file(".cargo/config", &format!(r#"
            paths = ['{}']
        "#, root.display()))
        .file("Cargo.toml", &format!(r#"
            [package]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = '{}'

        "#, bar.root().display()))
       .file("src/lib.rs", "")
       .file("src/main.rs", "fn main() {}");

    bar.build();
    assert_that(p.cargo_process("build"), execs().with_status(0));

}

#[test]
fn override_path_dep() {
    let bar = project("bar")
       .file("p1/Cargo.toml", r#"
            [package]
            name = "p1"
            version = "0.5.0"
            authors = []

            [dependencies.p2]
            path = "../p2"
       "#)
       .file("p1/src/lib.rs", "")
       .file("p2/Cargo.toml", r#"
            [package]
            name = "p2"
            version = "0.5.0"
            authors = []
       "#)
       .file("p2/src/lib.rs", "");

    let p = project("foo")
        .file(".cargo/config", &format!(r#"
            paths = ['{}', '{}']
        "#, bar.root().join("p1").display(),
            bar.root().join("p2").display()))
        .file("Cargo.toml", &format!(r#"
            [package]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.p2]
            path = '{}'

        "#, bar.root().join("p2").display()))
       .file("src/lib.rs", "");

    bar.build();
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));

}

#[test]
fn path_dep_build_cmd() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "bar"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"

            [lib]
            name = "bar"
            path = "src/bar.rs"
        "#)
        .file("bar/build.rs", r#"
            use std::fs;
            fn main() {
                fs::copy("src/bar.rs.in", "src/bar.rs").unwrap();
            }
        "#)
        .file("bar/src/bar.rs.in", r#"
            pub fn gimme() -> i32 { 0 }
        "#);

    p.build();
    p.root().join("bar").move_into_the_past();

    assert_that(p.cargo("build"),
        execs().with_stderr(&format!("[COMPILING] bar v0.5.0 ({}/bar)\n\
                                     [COMPILING] foo v0.5.0 ({})\n\
                                     [FINISHED] dev [unoptimized + debuginfo] target(s) in \
                                     [..]\n",
                                    p.url(),
                                    p.url())));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("0\n"));

    // Touching bar.rs.in should cause the `build` command to run again.
    {
        let file = fs::File::create(&p.root().join("bar/src/bar.rs.in"));
        file.unwrap().write_all(br#"pub fn gimme() -> i32 { 1 }"#).unwrap();
    }

    assert_that(p.cargo("build"),
        execs().with_stderr(&format!("[COMPILING] bar v0.5.0 ({}/bar)\n\
                                     [COMPILING] foo v0.5.0 ({})\n\
                                     [FINISHED] dev [unoptimized + debuginfo] target(s) in \
                                     [..]\n",
                                    p.url(),
                                    p.url())));

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("1\n"));
}

#[test]
fn dev_deps_no_rebuild_lib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
                name = "foo"
                version = "0.5.0"
                authors = []

            [dev-dependencies.bar]
                path = "bar"

            [lib]
                name = "foo"
                doctest = false
        "#)
        .file("src/lib.rs", r#"
            #[cfg(test)] extern crate bar;
            #[cfg(not(test))] pub fn foo() { env!("FOO"); }
        "#)
        .file("bar/Cargo.toml", r#"
            [package]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("bar/src/lib.rs", "pub fn bar() {}");
    p.build();
    assert_that(p.cargo("build")
                 .env("FOO", "bar"),
                execs().with_status(0)
                       .with_stderr(&format!("[COMPILING] foo v0.5.0 ({})\n\
                                              [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                              in [..]\n",
                                              p.url())));

    assert_that(p.cargo("test"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] [..] v0.5.0 ({url}[..])
[COMPILING] [..] v0.5.0 ({url}[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]", url = p.url()))
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn custom_target_no_rebuild() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            [dependencies]
            a = { path = "a" }
            [workspace]
            members = ["a", "b"]
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.5.0"
            authors = []
            [dependencies]
            a = { path = "../a" }
        "#)
        .file("b/src/lib.rs", "");
    p.build();
    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] a v0.5.0 ([..])
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    t!(fs::rename(p.root().join("target"), p.root().join("target_moved")));
    assert_that(p.cargo("build")
                 .arg("--manifest-path=b/Cargo.toml")
                 .env("CARGO_TARGET_DIR", "target_moved"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] b v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn override_and_depend() {
    let p = project("foo")
        .file("a/a1/Cargo.toml", r#"
            [project]
            name = "a1"
            version = "0.5.0"
            authors = []
            [dependencies]
            a2 = { path = "../a2" }
        "#)
        .file("a/a1/src/lib.rs", "")
        .file("a/a2/Cargo.toml", r#"
            [project]
            name = "a2"
            version = "0.5.0"
            authors = []
        "#)
        .file("a/a2/src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.5.0"
            authors = []
            [dependencies]
            a1 = { path = "../a/a1" }
            a2 = { path = "../a/a2" }
        "#)
        .file("b/src/lib.rs", "")
        .file("b/.cargo/config", r#"
            paths = ["../a"]
        "#);
    p.build();
    assert_that(p.cargo("build").cwd(p.root().join("b")),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] a2 v0.5.0 ([..])
[COMPILING] a1 v0.5.0 ([..])
[COMPILING] b v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn missing_path_dependency() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            paths = ["../whoa-this-does-not-exist"]
        "#);
    p.build();
    assert_that(p.cargo("build"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] failed to update path override `[..]../whoa-this-does-not-exist` \
(defined in `[..]`)

Caused by:
  failed to read directory `[..]`

Caused by:
  [..] (os error [..])
"));
}

#[test]
fn invalid_path_dep_in_workspace_with_lockfile() {
    Package::new("bar", "1.0.0").publish();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "top"
            version = "0.5.0"
            authors = []

            [workspace]

            [dependencies]
            foo = { path = "foo" }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("foo/src/lib.rs", "");
    p.build();

    // Generate a lock file
    assert_that(p.cargo("build"), execs().with_status(0));

    // Change the dependency on `bar` to an invalid path
    File::create(&p.root().join("foo/Cargo.toml")).unwrap().write_all(br#"
        [project]
        name = "foo"
        version = "0.5.0"
        authors = []

        [dependencies]
        bar = { path = "" }
    "#).unwrap();

    // Make sure we get a nice error. In the past this actually stack
    // overflowed!
    assert_that(p.cargo("build"),
                execs().with_status(101)
                       .with_stderr("\
error: no matching package named `bar` found (required by `foo`)
location searched: [..]
version required: *
"));
}

#[test]
fn workspace_produces_rlib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "top"
            version = "0.5.0"
            authors = []

            [workspace]

            [dependencies]
            foo = { path = "foo" }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build"), execs().with_status(0));

    assert_that(&p.root().join("target/debug/libtop.rlib"), existing_file());
    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());

}
