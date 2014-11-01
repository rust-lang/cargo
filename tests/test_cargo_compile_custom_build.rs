use std::path;

use support::{project, execs};
use support::{COMPILING, RUNNING};
use hamcrest::{assert_that};

fn setup() {
}

test!(custom_build_script_failed {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("build.rs", r#"
            fn main() {
                std::os::set_exit_status(101);
            }
        "#);
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 ({url})
{running} `rustc build.rs --crate-name build-script-build --crate-type bin [..]`
",
url = p.url(), compiling = COMPILING, running = RUNNING))
                       .with_stderr(format!("\
Failed to run custom build command for `foo v0.5.0 ({})`
Process didn't exit successfully: `[..]build[..]build-script-build` (status=101)",
p.url())));
})

test!(custom_build_env_vars {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [features]
            bar_feat = ["bar/foo"]

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"

            [features]
            foo = []
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn hello() {}
        "#);

    let file_content = format!(r#"
            use std::os;
            use std::io::fs::PathExtensions;
            fn main() {{
                let _target = os::getenv("TARGET").unwrap();

                let _ncpus = os::getenv("NUM_JOBS").unwrap();

                let out = os::getenv("CARGO_MANIFEST_DIR").unwrap();
                let p1 = Path::new(out);
                let p2 = os::make_absolute(&Path::new(file!()).dir_path().dir_path());
                assert!(p1 == p2, "{{}} != {{}}", p1.display(), p2.display());

                let opt = os::getenv("OPT_LEVEL").unwrap();
                assert_eq!(opt.as_slice(), "0");

                let opt = os::getenv("PROFILE").unwrap();
                assert_eq!(opt.as_slice(), "compile");

                let debug = os::getenv("DEBUG").unwrap();
                assert_eq!(debug.as_slice(), "true");

                let out = os::getenv("OUT_DIR").unwrap();
                assert!(out.as_slice().starts_with(r"{0}"));
                assert!(Path::new(out).is_dir());

                let _feat = os::getenv("CARGO_FEATURE_FOO").unwrap();
            }}
        "#,
        p.root().join("target").join("native").display());

    let p = p.file("bar/build.rs", file_content);


    assert_that(p.cargo_process("build").arg("--features").arg("bar_feat"),
                execs().with_status(0));
})

test!(custom_build_script_wrong_rustc_flags {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-flags=-aaa -bbb");
            }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr(format!("\
Only `-l` and `-L` flags are allowed in build script of `foo v0.5.0 ({})`:
`-aaa -bbb
`",
p.url())));
})

/*
test!(custom_build_script_rustc_flags {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.foo]
            path = "foo"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("foo/Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"
        "#)
        .file("foo/src/lib.rs", r#"
        "#)
        .file("foo/build.rs", r#"
            fn main() {
                println!("cargo:rustc-flags=-l nonexistinglib -L /dummy/path1 -L /dummy/path2");
            }
        "#);

    // TODO: TEST FAILS BECAUSE OF WRONG STDOUT (but otherwise, the build works)
    assert_that(p.cargo_process("build").arg("--verbose"),
                execs().with_status(101)
                       .with_stdout(format!("\
{compiling} bar v0.5.0 ({url})
{running} `rustc {dir}{sep}src{sep}lib.rs --crate-name test --crate-type lib -g \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target \
        --dep-info [..] \
        -L {dir}{sep}target \
        -L {dir}{sep}target{sep}deps`
",
running = RUNNING, compiling = COMPILING, sep = path::SEP,
dir = p.root().display(),
url = p.url(),
)));
})
*/
