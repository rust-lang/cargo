use std::io::File;
use std::io::timer;

use support::{ResultTest, project, execs, main_file, escape_path, cargo_dir};
use support::{COMPILING, FRESH};
use hamcrest::{assert_that, existing_file};
use cargo;
use cargo::util::{process};

fn setup() {
}

test!(cargo_compile_with_nested_deps_shorthand {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "bar"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]

            version = "0.5.0"
            path = "baz"

            [[lib]]

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

            [[lib]]

            name = "baz"
        "#)
        .file("bar/baz/src/baz.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    assert_that(p.cargo_process("cargo-build"),
        execs().with_stdout(format!("{} baz v0.5.0 (file:{})\n\
                                     {} bar v0.5.0 (file:{})\n\
                                     {} foo v0.5.0 (file:{})\n",
                                    COMPILING, p.root().display(),
                                    COMPILING, p.root().display(),
                                    COMPILING, p.root().display())));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("test passed\n"));
})

test!(cargo_compile_with_root_dev_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dev-dependencies.bar]

            version = "0.5.0"
            path = "bar"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn gimme() -> &'static str {
                "zoidberg"
            }
        "#)
        ;

    assert_that(p.cargo_process("cargo-build"),
        execs().with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                                     {} foo v0.5.0 (file:{})\n",
                                    COMPILING, p.root().display(),
                                    COMPILING, p.root().display())));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("zoidberg\n"));
})

test!(cargo_compile_with_transitive_dev_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "bar"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dev-dependencies.baz]

            version = "0.5.0"
            path = "baz"

            [[lib]]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn gimme() -> &'static str {
                "zoidberg"
            }
        "#)
        .file("bar/baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]

            name = "baz"
        "#)
        .file("bar/baz/src/baz.rs", r#"
            pub fn gimme() -> &'static str {
                "nope"
            }
        "#);

    assert_that(p.cargo_process("cargo-build"),
        execs().with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                                     {} foo v0.5.0 (file:{})\n",
                                    COMPILING, p.root().display(),
                                    COMPILING, p.root().display())));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("zoidberg\n"));
})

test!(no_rebuild_dependency {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}"]
        "#, escape_path(&bar)).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
            [dependencies] bar = "0.5.0"
        "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]] name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn bar() {}
        "#);
    // First time around we should compile both foo and bar
    assert_that(p.cargo_process("cargo-build"),
                execs().with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            COMPILING, bar.display(),
                                            COMPILING, p.root().display())));
    // This time we shouldn't compile bar
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            FRESH, bar.display(),
                                            FRESH, p.root().display())));

    p.build(); // rebuild the files (rewriting them in the process)
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            COMPILING, bar.display(),
                                            COMPILING, p.root().display())));
})

test!(deep_dependencies_trigger_rebuild {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");
    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}", "{}"]
        "#, escape_path(&bar), escape_path(&baz)).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
            [dependencies]
            bar = "0.5.0"
        "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]
            name = "bar"
            [dependencies]
            baz = "0.5.0"
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

            [[lib]]
            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn baz() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
                execs().with_stdout(format!("{} baz v0.5.0 (file:{})\n\
                                             {} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            COMPILING, baz.display(),
                                            COMPILING, bar.display(),
                                            COMPILING, p.root().display())));
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} baz v0.5.0 (file:{})\n\
                                             {} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            FRESH, baz.display(),
                                            FRESH, bar.display(),
                                            FRESH, p.root().display())));

    // Make sure an update to baz triggers a rebuild of bar
    //
    // We base recompilation off mtime, so sleep for at least a second to ensure
    // that this write will change the mtime.
    timer::sleep(1000);
    File::create(&p.root().join("baz/src/baz.rs")).write_str(r#"
        pub fn baz() { println!("hello!"); }
    "#).assert();
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} baz v0.5.0 (file:{})\n\
                                             {} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            COMPILING, baz.display(),
                                            COMPILING, bar.display(),
                                            COMPILING, p.root().display())));

    // Make sure an update to bar doesn't trigger baz
    File::create(&p.root().join("bar/src/bar.rs")).write_str(r#"
        extern crate baz;
        pub fn bar() { println!("hello!"); baz::baz(); }
    "#).assert();
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} baz v0.5.0 (file:{})\n\
                                             {} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            FRESH, baz.display(),
                                            COMPILING, bar.display(),
                                            COMPILING, p.root().display())));
})

test!(no_rebuild_two_deps {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");
    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}", "{}"]
        "#, escape_path(&bar), escape_path(&baz)).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
            [dependencies]
            bar = "0.5.0"
            baz = "0.5.0"
        "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]
            name = "bar"
            [dependencies]
            baz = "0.5.0"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn bar() {}
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]
            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn baz() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
                execs().with_stdout(format!("{} baz v0.5.0 (file:{})\n\
                                             {} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            COMPILING, baz.display(),
                                            COMPILING, bar.display(),
                                            COMPILING, p.root().display())));
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} baz v0.5.0 (file:{})\n\
                                             {} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            FRESH, baz.display(),
                                            FRESH, bar.display(),
                                            FRESH, p.root().display())));
})

test!(nested_deps_recompile {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "src/bar"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("src/bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]

            name = "bar"
        "#)
        .file("src/bar/src/bar.rs", "pub fn gimme() {}");
    let bar = p.root();

    assert_that(p.cargo_process("cargo-build"),
                execs().with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            COMPILING, bar.display(),
                                            COMPILING, p.root().display())));
    // See comments for the above `sleep`
    timer::sleep(1000);
    File::create(&p.root().join("src/foo.rs")).write_str(r#"
        fn main() {}
    "#).assert();

    // This shouldn't recompile `bar`
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                                             {} foo v0.5.0 (file:{})\n",
                                            FRESH, bar.display(),
                                            COMPILING, p.root().display())));
})

test!(error_message_for_missing_manifest {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            path = "src/bar"

            [[lib]]

            name = "foo"
        "#)
       .file("src/bar/not-a-manifest", "");

    assert_that(p.cargo_process("cargo-build"),
                execs()
                .with_status(101)
                .with_stderr(format!("Could not find `Cargo.toml` in `{}`\n",
                                     p.root().join_many(&["src", "bar"]).display())));

})
