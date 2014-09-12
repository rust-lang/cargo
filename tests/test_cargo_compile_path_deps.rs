use std::io::{fs, File, TempDir, UserRWX};

use support::{ProjectBuilder, ResultTest, project, execs, main_file, cargo_dir, path2url};
use support::{COMPILING, RUNNING};
use support::paths::{mod, PathExt};
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
        execs().with_stdout(format!("{} baz v0.5.0 ({})\n\
                                     {} bar v0.5.0 ({})\n\
                                     {} foo v0.5.0 ({})\n",
                                    COMPILING, p.url(),
                                    COMPILING, p.url(),
                                    COMPILING, p.url())));

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
            path = "../bar"

            [[bin]]
            name = "foo"
        "#)
        .file("src/main.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice());
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
})

test!(cargo_compile_with_root_dev_deps_with_testing {
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
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice());
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
        execs().with_stdout(format!("\
{compiling} bar v0.5.0 ({url})
{compiling} foo v0.5.0 ({url})
{running} target[..]foo-[..]

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

", compiling = COMPILING, url = p.url(), running = RUNNING)));
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
        execs().with_stdout(format!("{} bar v0.5.0 ({})\n\
                                     {} foo v0.5.0 ({})\n",
                                    COMPILING, p.url(),
                                    COMPILING, p.url())));

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
            paths = ['{}']
        "#, bar.display()).as_slice())
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

            [lib] name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn bar() {}
        "#);
    let bar = path2url(bar);
    // First time around we should compile both foo and bar
    assert_that(p.cargo_process("build"),
                execs().with_stdout(format!("{} bar v0.5.0 ({})\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, bar,
                                            COMPILING, p.url())));
    // This time we shouldn't compile bar
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(""));
    p.root().move_into_the_past().assert();

    p.build(); // rebuild the files (rewriting them in the process)
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(format!("{} bar v0.5.0 ({})\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, bar,
                                            COMPILING, p.url())));
})

test!(deep_dependencies_trigger_rebuild {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");
    p = p
        .file(".cargo/config", format!(r#"
            paths = ['{}', '{}']
        "#, bar.display(), baz.display()).as_slice())
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

            [lib]
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

            [lib]
            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn baz() {}
        "#);
    let baz = path2url(baz);
    let bar = path2url(bar);
    assert_that(p.cargo_process("build"),
                execs().with_stdout(format!("{} baz v0.5.0 ({})\n\
                                             {} bar v0.5.0 ({})\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, baz,
                                            COMPILING, bar,
                                            COMPILING, p.url())));
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(""));

    // Make sure an update to baz triggers a rebuild of bar
    //
    // We base recompilation off mtime, so sleep for at least a second to ensure
    // that this write will change the mtime.
    p.root().move_into_the_past().assert();
    File::create(&p.root().join("baz/src/baz.rs")).write_str(r#"
        pub fn baz() { println!("hello!"); }
    "#).assert();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(format!("{} baz v0.5.0 ({})\n\
                                             {} bar v0.5.0 ({})\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, baz,
                                            COMPILING, bar,
                                            COMPILING, p.url())));

    // Make sure an update to bar doesn't trigger baz
    p.root().move_into_the_past().assert();
    File::create(&p.root().join("bar/src/bar.rs")).write_str(r#"
        extern crate baz;
        pub fn bar() { println!("hello!"); baz::baz(); }
    "#).assert();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(format!("{} bar v0.5.0 ({})\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, bar,
                                            COMPILING, p.url())));

})

test!(no_rebuild_two_deps {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");
    p = p
        .file(".cargo/config", format!(r#"
            paths = ['{}', '{}']
        "#, bar.display(), baz.display()).as_slice())
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

            [lib]
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

            [lib]
            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn baz() {}
        "#);
    let baz = path2url(baz);
    let bar = path2url(bar);
    assert_that(p.cargo_process("build"),
                execs().with_stdout(format!("{} baz v0.5.0 ({})\n\
                                             {} bar v0.5.0 ({})\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, baz,
                                            COMPILING, bar,
                                            COMPILING, p.url())));
    assert_that(&p.bin("foo"), existing_file());
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(""));
    assert_that(&p.bin("foo"), existing_file());
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

            [lib]

            name = "bar"
        "#)
        .file("src/bar/src/bar.rs", "pub fn gimme() {}");
    let bar = p.url();

    assert_that(p.cargo_process("build"),
                execs().with_stdout(format!("{} bar v0.5.0 ({})\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, bar,
                                            COMPILING, p.url())));
    p.root().move_into_the_past().assert();

    File::create(&p.root().join("src/foo.rs")).write_str(r#"
        fn main() {}
    "#).assert();

    // This shouldn't recompile `bar`
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(format!("{} foo v0.5.0 ({})\n",
                                            COMPILING, p.url())));
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

            [lib]

            name = "foo"
        "#)
       .file("src/bar/not-a-manifest", "");

    assert_that(p.cargo_process("build"),
                execs()
                .with_status(101)
                .with_stderr(format!("Could not find `Cargo.toml` in `{}`\n",
                                     p.root().join_many(&["src", "bar"]).display())));

})

test!(override_relative {
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
       .file("src/lib.rs", "");

    fs::mkdir(&paths::root().join(".cargo"), UserRWX).assert();
    File::create(&paths::root().join(".cargo/config")).write_str(r#"
        paths = ["bar"]
    "#).assert();

    let p = project("foo")
        .file("Cargo.toml", format!(r#"
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

})

test!(override_self {
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
        .file(".cargo/config", format!(r#"
            paths = ['{}']
        "#, root.display()))
        .file("Cargo.toml", format!(r#"
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

})

test!(override_path_dep {
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
        .file(".cargo/config", format!(r#"
            paths = ['{}', '{}']
        "#, bar.root().join("p1").display(),
            bar.root().join("p2").display()))
        .file("Cargo.toml", format!(r#"
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

})

test!(path_dep_build_cmd {
    let tmpdir = TempDir::new("cargo").unwrap();
    let p = ProjectBuilder::new("foo", tmpdir.path().clone())
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
            build = "cp src/bar.rs.in src/bar.rs"

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs.in", r#"
            pub fn gimme() -> int { 0 }
        "#);

    assert_that(p.cargo_process("build"),
        execs().with_status(0));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("0\n"));

    // Touching bar.rs.in should cause the `build` command to run again.
    {
        let mut file = fs::File::create(&p.root().join("bar/src/bar.rs.in")).assert();
        file.write_str(r#"pub fn gimme() -> int { 1 }"#).assert();
    }

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
        execs().with_status(0));

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("1\n"));
})

