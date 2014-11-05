use std::path;

use support::{project, execs, cargo_dir};
use hamcrest::{assert_that};

fn setup() {
}

test!(old_custom_build {
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() { println!("Hello!"); }
        "#);
    assert_that(build.cargo_process("build"),
                execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = '{}'

            [[bin]] name = "foo"
        "#, build.bin("foo").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stdout(format!("   Compiling foo v0.5.0 ({})\n",
                                            p.url()))
                       .with_stderr("warning: the old build command has been deprecated"));
})

test!(old_custom_multiple_build {
    let mut build1 = project("builder1");
    build1 = build1
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() {
                let args = ::std::os::args();
                assert_eq!(args[1], "hello".to_string());
                assert_eq!(args[2], "world".to_string());
            }
        "#);
    assert_that(build1.cargo_process("build"),
                execs().with_status(0));

    let mut build2 = project("builder2");
    build2 = build2
        .file("Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "bar"
        "#)
        .file("src/bar.rs", r#"
            fn main() {
                let args = ::std::os::args();
                assert_eq!(args[1], "cargo".to_string());
            }
        "#);
    assert_that(build2.cargo_process("build"),
                execs().with_status(0));

    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = [ '{} hello world', '{} cargo' ]

            [[bin]] name = "foo"
        "#, build1.bin("foo").display(), build2.bin("bar").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stdout(format!("   Compiling foo v0.5.0 ({})\n",
                                            p.url()))
                       .with_stderr("warning: the old build command has been deprecated"));
})

test!(old_custom_build_failure {
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() { panic!("nope") }
        "#);
    assert_that(build.cargo_process("build"), execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = '{}'

            [[bin]]
            name = "foo"
        "#, build.bin("foo").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(format!("\
warning: the old build command has been deprecated\n\
Failed to run custom build command for `foo v0.5.0 ({dir})`
Process didn't exit successfully: `{}` (status=101)\n\
--- stderr\n\
task '<main>' panicked at 'nope', {filename}:2\n\
\n\
", build.bin("foo").display(), filename = format!("src{}foo.rs", path::SEP),
   dir = p.url())));
})

test!(old_custom_second_build_failure {
    let mut build1 = project("builder1");
    build1 = build1
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() { println!("Hello!"); }
        "#);
    assert_that(build1.cargo_process("build"),
                execs().with_status(0));

    let mut build2 = project("builder2");
    build2 = build2
        .file("Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "bar"
        "#)
        .file("src/bar.rs", r#"
            fn main() { panic!("nope") }
        "#);
    assert_that(build2.cargo_process("build"), execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = [ '{}', '{}' ]

            [[bin]]
            name = "foo"
        "#, build1.bin("foo").display(), build2.bin("bar").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(format!("\
warning: the old build command has been deprecated\n\
Failed to run custom build command for `foo v0.5.0 ({dir})`
Process didn't exit successfully: `{}` (status=101)\n\
--- stderr\n\
task '<main>' panicked at 'nope', {filename}:2\n\
\n\
", build2.bin("bar").display(), filename = format!("src{}bar.rs", path::SEP),
   dir = p.url())));
})

test!(old_custom_build_env_vars {
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar-bar"
            version = "0.0.1"
            authors = []
            build = "true"
        "#)
        .file("src/lib.rs", "");
    bar.build();

    let mut p = project("foo");
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [features]
            foo = []

            [[bin]]
            name = "foo"
        "#)
        .file("src/foo.rs", format!(r#"
            use std::os;
            use std::io::fs::PathExtensions;
            fn main() {{
                let _ncpus = os::getenv("NUM_JOBS").unwrap();
                let _feat = os::getenv("CARGO_FEATURE_FOO").unwrap();
                let debug = os::getenv("DEBUG").unwrap();
                assert_eq!(debug.as_slice(), "true");

                let opt = os::getenv("OPT_LEVEL").unwrap();
                assert_eq!(opt.as_slice(), "0");

                let opt = os::getenv("PROFILE").unwrap();
                assert_eq!(opt.as_slice(), "compile");

                let out = os::getenv("OUT_DIR").unwrap();
                assert!(out.as_slice().starts_with(r"{0}"));
                assert!(Path::new(out).is_dir());

                let out = os::getenv("DEP_BAR_BAR_OUT_DIR").unwrap();
                assert!(out.as_slice().starts_with(r"{0}"));
                assert!(Path::new(out).is_dir());

                let out = os::getenv("CARGO_MANIFEST_DIR").unwrap();
                let p1 = Path::new(out);
                let p2 = os::make_absolute(&Path::new(file!()).dir_path().dir_path());
                assert!(p1 == p2, "{{}} != {{}}", p1.display(), p2.display());
            }}
        "#,
        p.root().join("target").join("native").display()));
    assert_that(build.cargo_process("build").arg("--features").arg("foo"),
                execs().with_status(0));


    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = '{}'

            [features]
            foo = []

            [[bin]]
            name = "foo"

            [dependencies.bar-bar]
            path = '{}'
        "#, build.bin("foo").display(), bar.root().display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build").arg("--features").arg("foo"),
                execs().with_status(0));
})

test!(old_custom_build_in_dependency {
    let mut p = project("foo");
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
        "#)
        .file("src/foo.rs", format!(r#"
            use std::os;
            fn main() {{
                assert!(os::getenv("OUT_DIR").unwrap().as_slice()
                           .starts_with(r"{}"));
            }}
        "#,
        p.root().join("target/native/bar-").display()));
    assert_that(build.cargo_process("build"), execs().with_status(0));


    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", format!(r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = '{}'
        "#, build.bin("foo").display()))
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0));
})

// tests that custom build in dep can be built twice in a row - issue 227
test!(old_custom_build_in_dependency_twice {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
            [dependencies.bar]
            path = "./bar"
            "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
            "#)
        .file("bar/Cargo.toml", format!(r#"
            [project]

            name = "bar"
            version = "0.0.1"
            authors = ["wycats@example.com"]
            build = '{}'
        "#, "echo test"))
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));
})
