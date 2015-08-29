use support::{project, execs, basic_bin_manifest, basic_lib_manifest, main_file, git};
use support::paths::{self};
use support::registry as r;
use hamcrest::assert_that;

fn setup() {
    r::init();
}

test!(cargo_print_source_root_registry_dep {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = ">= 0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");

    r::mock_pkg("bar", "0.0.1", &[]);

    assert_that(foo.cargo_process("build"),
                execs().with_status(0));

    assert_that(foo.cargo("print-source-root").cwd(&foo.root()),
                execs().with_status(0)
                       .with_stdout(&format!(
                           "bar = \"{}\"\n\
                            foo = \"{}\"\n",
                            paths::home().join(
                                ".cargo[..]registry[..]src[..]bar-0.0.1"
                            ).display(),
                            foo.root().display()))
                       .with_stderr(""));
});

test!(cargo_print_source_root_git_dep {
    let foo = project("foo");
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]

                name = "dep1"
            "#)
            .file("src/dep1.rs", r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#)
    }).unwrap();

    let foo = foo
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'

            [[bin]]

            name = "foo"
        "#, git_project.url()))
        .file("src/foo.rs", &main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    assert_that(foo.cargo_process("build"),
                execs().with_status(0));

    assert_that(foo.cargo("print-source-root").cwd(&foo.root()),
                execs().with_status(0)
                       .with_stdout(&format!(
                           "dep1 = \"{}\"\n\
                           foo = \"{}\"\n",
                           paths::home().join(
                               ".cargo[..]git[..]checkouts[..]dep1-[..]master"
                           ).display(),
                           foo.root().display()))
                       .with_stderr(""));
});

test!(cargo_print_source_root_path_dep {
    let foo = project("foo")
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
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);
    assert_that(foo.cargo_process("build"),
                execs().with_status(0));

    assert_that(foo.cargo("print-source-root").cwd(&foo.root()),
                execs().with_status(0)
                       .with_stdout(&format!("bar = \"{}[..]bar\"\n\
                                              foo = \"{}\"\n",
                                              foo.root().display(),
                                              foo.root().display()))
                       .with_stderr(""));
});

test!(cargo_print_source_root_dev_dep {
    let foo = project("foo")
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
    let bar = project("bar")
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

    bar.build();
    assert_that(foo.cargo_process("test"),
                execs().with_status(0));

    assert_that(foo.cargo("print-source-root").cwd(&foo.root()),
                execs().with_status(0)
                       .with_stdout(&format!(
                           "bar = \"{}\"\n\
                            foo = \"{}\"\n",
                            bar.root().display(),
                            foo.root().display()))
                       .with_stderr(""));
});

test!(cargo_print_source_root_overridden_registry_dep {
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
       .file("src/lib.rs", "");

    let foo = project("foo")
        .file(".cargo/config", &format!(r#"
            paths = ['{}']
        "#, bar.root().display()))
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = ">= 0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");

    r::mock_pkg("bar", "0.0.1", &[]);

    assert_that(foo.cargo_process("build"),
                execs().with_status(0));

    assert_that(foo.cargo("print-source-root").cwd(&foo.root()),
                execs().with_status(0)
                       .with_stdout(&format!(
                           "bar = \"{}\"\n\
                            foo = \"{}\"\n",
                            bar.root().display(),
                            foo.root().display()))
                       .with_stderr(""));
});

test!(cargo_print_source_root_no_deps {
    let foo = project("foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "");
    assert_that(foo.cargo_process("build"),
                execs().with_status(0));

    assert_that(foo.cargo("print-source-root").cwd(&foo.root()),
                execs().with_status(0)
                       .with_stdout(&format!("foo = \"{}\"", foo.root().display()))
                       .with_stderr(""));
});

test!(cargo_print_source_root_without_lock {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"));

    assert_that(p.cargo_process("print-source-root"),
                execs().with_status(1)
                       .with_stdout("")
                       .with_stderr("A Cargo.lock must exist for this command"));
});
