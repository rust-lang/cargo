use std::env;
use std::old_io::{File, fs};

use support::{project, execs, cargo_dir};
use support::{COMPILING, RUNNING, DOCTEST};
use support::paths::PathExt;
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
            #![feature(env)]
            fn main() {
                std::env::set_exit_status(101);
            }
        "#);
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 ({url})
{running} `rustc build.rs --crate-name build-script-build --crate-type bin [..]`
{running} `[..]build-script-build[..]`
",
url = p.url(), compiling = COMPILING, running = RUNNING))
                       .with_stderr(format!("\
failed to run custom build command for `foo v0.5.0 ({})`
Process didn't exit successfully: `[..]build[..]build-script-build[..]` (status=101)",
p.url())));
});

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
            use std::env;
            use std::old_io::fs::PathExtensions;
            fn main() {{
                let _target = env::var_string("TARGET").unwrap();

                let _ncpus = env::var_string("NUM_JOBS").unwrap();

                let out = env::var_string("CARGO_MANIFEST_DIR").unwrap();
                let p1 = Path::new(out);
                let cwd = env::current_dir().unwrap();
                let p2 = cwd.join(Path::new(file!()).dir_path().dir_path());
                assert!(p1 == p2, "{{}} != {{}}", p1.display(), p2.display());

                let opt = env::var_string("OPT_LEVEL").unwrap();
                assert_eq!(opt.as_slice(), "0");

                let opt = env::var_string("PROFILE").unwrap();
                assert_eq!(opt.as_slice(), "compile");

                let debug = env::var_string("DEBUG").unwrap();
                assert_eq!(debug.as_slice(), "true");

                let out = env::var_string("OUT_DIR").unwrap();
                assert!(out.as_slice().starts_with(r"{0}"));
                assert!(Path::new(out).is_dir());

                let _host = env::var_string("HOST").unwrap();

                let _feat = env::var_string("CARGO_FEATURE_FOO").unwrap();
            }}
        "#,
        p.root().join("target").join("build").display());

    let p = p.file("bar/build.rs", file_content);


    assert_that(p.cargo_process("build").arg("--features").arg("bar_feat"),
                execs().with_status(0));
});

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
Only `-l` and `-L` flags are allowed in build script of `foo v0.5.0 ({})`: \
`-aaa -bbb`",
p.url())));
});

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
        --emit=dep-info,link \
        -L {dir}{sep}target \
        -L {dir}{sep}target{sep}deps`
",
running = RUNNING, compiling = COMPILING, sep = path::SEP,
dir = p.root().display(),
url = p.url(),
)));
});
*/

test!(links_no_build_cmd {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            links = "a"
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
package `foo v0.5.0 (file://[..])` specifies that it links to `a` but does \
not have a custom build script
"));
});

test!(links_duplicates {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            links = "a"
            build = "build.rs"

            [dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", "")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            links = "a"
            build = "build.rs"
        "#)
        .file("a/src/lib.rs", "")
        .file("a/build.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
native library `a` is being linked to by more than one package, and can only be \
linked to by one package

  [..] v0.5.0 (file://[..])
  [..] v0.5.0 (file://[..])
"));
});

test!(overrides_and_links {
    let (_, target) = ::cargo::ops::rustc_version().unwrap();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            use std::env;
            fn main() {
                assert_eq!(env::var_string("DEP_FOO_FOO").unwrap().as_slice(), "bar");
                assert_eq!(env::var_string("DEP_FOO_BAR").unwrap().as_slice(), "baz");
            }
        "#)
        .file(".cargo/config", format!(r#"
            [target.{}.foo]
            rustc-flags = "-L foo -L bar"
            foo = "bar"
            bar = "baz"
        "#, target).as_slice())
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            links = "foo"
            build = "build.rs"
        "#)
        .file("a/src/lib.rs", "")
        .file("a/build.rs", "not valid rust code");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stdout(format!("\
[..]
[..]
[..]
[..]
[..]
{running} `rustc [..] --crate-name foo [..] -L foo -L bar[..]`
", running = RUNNING).as_slice()));
});

test!(unused_overrides {
    let (_, target) = ::cargo::ops::rustc_version().unwrap();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file(".cargo/config", format!(r#"
            [target.{}.foo]
            rustc-flags = "-L foo -L bar"
            foo = "bar"
            bar = "baz"
        "#, target).as_slice());

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
});

test!(links_passes_env_vars {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            use std::env;
            fn main() {
                assert_eq!(env::var_string("DEP_FOO_FOO").unwrap().as_slice(), "bar");
                assert_eq!(env::var_string("DEP_FOO_BAR").unwrap().as_slice(), "baz");
            }
        "#)
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            links = "foo"
            build = "build.rs"
        "#)
        .file("a/src/lib.rs", "")
        .file("a/build.rs", r#"
            fn main() {
                println!("cargo:foo=bar");
                println!("cargo:bar=baz");
            }
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} [..] v0.5.0 (file://[..])
{running} `rustc [..]build.rs [..]`
{compiling} [..] v0.5.0 (file://[..])
{running} `rustc [..]build.rs [..]`
{running} `[..]`
{running} `[..]`
{running} `[..]`
{running} `rustc [..] --crate-name foo [..]`
", compiling = COMPILING, running = RUNNING).as_slice()));
});

test!(only_rerun_build_script {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    File::create(&p.root().join("some-new-file")).unwrap();
    p.root().move_into_the_past().unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("build").arg("-v"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 (file://[..])
{running} `[..]build-script-build[..]`
{running} `rustc [..] --crate-name foo [..]`
", compiling = COMPILING, running = RUNNING).as_slice()));
});

test!(rebuild_continues_to_pass_env_vars {
    let a = project("a")
        .file("Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            links = "foo"
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {
                println!("cargo:foo=bar");
                println!("cargo:bar=baz");
            }
        "#);
    a.build();
    a.root().move_into_the_past().unwrap();

    let p = project("foo")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [dependencies.a]
            path = '{}'
        "#, a.root().display()))
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            use std::env;
            fn main() {
                assert_eq!(env::var_string("DEP_FOO_FOO").unwrap().as_slice(), "bar");
                assert_eq!(env::var_string("DEP_FOO_BAR").unwrap().as_slice(), "baz");
            }
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    File::create(&p.root().join("some-new-file")).unwrap();
    p.root().move_into_the_past().unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("build").arg("-v"),
                execs().with_status(0));
});

test!(testing_and_such {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    File::create(&p.root().join("src/lib.rs")).unwrap();
    p.root().move_into_the_past().unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("test").arg("-vj1"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 (file://[..])
{running} `[..]build-script-build[..]`
{running} `rustc [..] --crate-name foo [..]`
{running} `rustc [..] --crate-name foo [..]`
{running} `[..]foo-[..]`

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo
{running} `rustdoc --test [..]`

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

", compiling = COMPILING, running = RUNNING, doctest = DOCTEST).as_slice()));

    assert_that(p.process(cargo_dir().join("cargo")).arg("doc").arg("-v"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 (file://[..])
{running} `rustdoc [..]`
{running} `rustc [..]`
", compiling = COMPILING, running = RUNNING).as_slice()));

    File::create(&p.root().join("src/main.rs")).write_str("fn main() {}").unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("run"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 (file://[..])
{running} `target[..]foo`
", compiling = COMPILING, running = RUNNING).as_slice()));
});

test!(propagation_of_l_flags {
    let (_, target) = ::cargo::ops::rustc_version().unwrap();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            [dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            links = "bar"
            build = "build.rs"

            [dependencies.b]
            path = "../b"
        "#)
        .file("a/src/lib.rs", "")
        .file("a/build.rs", r#"
            fn main() {
                println!("cargo:rustc-flags=-L bar");
            }
        "#)
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.5.0"
            authors = []
            links = "foo"
            build = "build.rs"
        "#)
        .file("b/src/lib.rs", "")
        .file("b/build.rs", "bad file")
        .file(".cargo/config", format!(r#"
            [target.{}.foo]
            rustc-flags = "-L foo"
        "#, target).as_slice());

    assert_that(p.cargo_process("build").arg("-v").arg("-j1"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} a v0.5.0 (file://[..])
{running} `rustc a[..]build.rs [..]`
{compiling} b v0.5.0 (file://[..])
{running} `rustc [..] --crate-name b [..]-L foo[..]`
{running} `[..]a-[..]build-script-build[..]`
{running} `rustc [..] --crate-name a [..]-L bar[..]-L foo[..]`
{compiling} foo v0.5.0 (file://[..])
{running} `rustc [..] --crate-name foo [..] -L bar -L foo`
", compiling = COMPILING, running = RUNNING).as_slice()));
});

test!(build_deps_simple {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
            [build-dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", "
            extern crate a;
            fn main() {}
        ")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} a v0.5.0 (file://[..])
{running} `rustc [..] --crate-name a [..]`
{compiling} foo v0.5.0 (file://[..])
{running} `rustc build.rs [..] --extern a=[..]`
{running} `[..]foo-[..]build-script-build[..]`
{running} `rustc [..] --crate-name foo [..]`
", compiling = COMPILING, running = RUNNING).as_slice()));
});

test!(build_deps_not_for_normal {
    let (_, target) = ::cargo::ops::rustc_version().unwrap();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
            [build-dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "extern crate a;")
        .file("build.rs", "
            extern crate a;
            fn main() {}
        ")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v").arg("--target").arg(target),
                execs().with_status(101)
                       .with_stderr("\
[..]lib.rs[..] error: can't find crate for `a`
[..]lib.rs[..] extern crate a;
[..]           ^~~~~~~~~~~~~~~
error: aborting due to previous error
Could not compile `foo`.

Caused by:
  Process didn't exit successfully: [..]
"));
});

test!(build_cmd_with_a_build_cmd {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [build-dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", "
            extern crate a;
            fn main() {}
        ")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [build-dependencies.b]
            path = "../b"
        "#)
        .file("a/src/lib.rs", "")
        .file("a/build.rs", "extern crate b; fn main() {}")
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.5.0"
            authors = []
        "#)
        .file("b/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} b v0.5.0 (file://[..])
{running} `rustc [..] --crate-name b [..]`
{compiling} a v0.5.0 (file://[..])
{running} `rustc a[..]build.rs [..] --extern b=[..]`
{running} `[..]a-[..]build-script-build[..]`
{running} `rustc [..]lib.rs --crate-name a --crate-type lib -g \
    -C metadata=[..] -C extra-filename=-[..] \
    --out-dir [..]target[..]deps --emit=dep-info,link \
    -L [..]target[..]deps -L [..]target[..]deps`
{compiling} foo v0.5.0 (file://[..])
{running} `rustc build.rs --crate-name build-script-build --crate-type bin \
    -C prefer-dynamic -g \
    --out-dir [..]build[..]foo-[..] --emit=dep-info,link \
    -L [..]target -L [..]target[..]deps \
    --extern a=[..]liba-[..].rlib`
{running} `[..]foo-[..]build-script-build[..]`
{running} `rustc [..]lib.rs --crate-name foo --crate-type lib -g \
    -C metadata=[..] -C extra-filename=-[..] \
    --out-dir [..]target --emit=dep-info,link \
    -L [..]target -L [..]target[..]deps`
", compiling = COMPILING, running = RUNNING).as_slice()));
});

test!(out_dir_is_preserved {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            use std::env;
            use std::old_io::File;
            fn main() {
                let out = env::var_string("OUT_DIR").unwrap();
                File::create(&Path::new(out).join("foo")).unwrap();
            }
        "#);

    // Make the file
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    // Change to asserting that it's there
    File::create(&p.root().join("build.rs")).write_str(r#"
        use std::env;
        use std::old_io::File;
        fn main() {
            let out = env::var_string("OUT_DIR").unwrap();
            File::open(&Path::new(out).join("foo")).unwrap();
        }
    "#).unwrap();
    p.root().move_into_the_past().unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build").arg("-v"),
                execs().with_status(0));

    // Run a fresh build where file should be preserved
    assert_that(p.process(cargo_dir().join("cargo")).arg("build").arg("-v"),
                execs().with_status(0));

    // One last time to make sure it's still there.
    File::create(&p.root().join("foo")).unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build").arg("-v"),
                execs().with_status(0));
});

test!(output_separate_lines {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-flags=-L foo");
                println!("cargo:rustc-flags=-l foo:static");
            }
        "#);
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 (file://[..])
{running} `rustc build.rs [..]`
{running} `[..]foo-[..]build-script-build[..]`
{running} `rustc [..] --crate-name foo [..] -L foo -l foo:static`
", compiling = COMPILING, running = RUNNING).as_slice()));
});

#[cfg(not(windows))] // FIXME(#867)
test!(code_generation {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/main.rs", r#"
            include!(concat!(env!("OUT_DIR"), "/hello.rs"));

            fn main() {
                println!("{}", message());
            }
        "#)
        .file("build.rs", r#"
            use std::env;
            use std::old_io::File;

            fn main() {
                let dst = Path::new(env::var_string("OUT_DIR").unwrap());
                let mut f = File::create(&dst.join("hello.rs")).unwrap();
                f.write_str("
                    pub fn message() -> &'static str {
                        \"Hello, World!\"
                    }
                ").unwrap();
            }
        "#);
    assert_that(p.cargo_process("run"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 (file://[..])
{running} `target[..]foo`
Hello, World!
", compiling = COMPILING, running = RUNNING).as_slice()));

    assert_that(p.cargo_process("test"),
                execs().with_status(0));
});

test!(release_with_build_script {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("build").arg("-v").arg("--release"),
                execs().with_status(0));
});

test!(build_script_only {
    let p = project("foo")
        .file("Cargo.toml", r#"
              [project]
              name = "foo"
              version = "0.0.0"
              authors = []
              build = "build.rs"
        "#)
        .file("build.rs", r#"fn main() {}"#);
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stderr("\
failed to parse manifest at `[..]`

Caused by:
  either a [lib] or [[bin]] section must be present"));
});

test!(shared_dep_with_a_build_script {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [dependencies.a]
            path = "a"

            [build-dependencies.b]
            path = "b"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("a/build.rs", "fn main() {}")
        .file("a/src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.5.0"
            authors = []

            [dependencies.a]
            path = "../b"
        "#)
        .file("b/src/lib.rs", "");
    assert_that(p.cargo_process("build"),
                execs().with_status(0));
});

test!(transitive_dep_host {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [build-dependencies.b]
            path = "b"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.5.0"
            authors = []
            links = "foo"
            build = "build.rs"
        "#)
        .file("a/build.rs", "fn main() {}")
        .file("a/src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.5.0"
            authors = []

            [lib]
            name = "b"
            plugin = true

            [dependencies.a]
            path = "../a"
        "#)
        .file("b/src/lib.rs", "");
    assert_that(p.cargo_process("build"),
                execs().with_status(0));
});

test!(test_a_lib_with_a_build_command {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", r#"
            include!(concat!(env!("OUT_DIR"), "/foo.rs"));

            /// ```
            /// foo::bar();
            /// ```
            pub fn bar() {
                assert_eq!(foo(), 1);
            }
        "#)
        .file("build.rs", r#"
            use std::env;
            use std::old_io::File;

            fn main() {
                let out = Path::new(env::var_string("OUT_DIR").unwrap());
                File::create(&out.join("foo.rs")).write_str("
                    fn foo() -> int { 1 }
                ").unwrap();
            }
        "#);
    assert_that(p.cargo_process("test"),
                execs().with_status(0));
});

test!(test_dev_dep_build_script {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dev-dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("a/build.rs", "fn main() {}")
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("test"), execs().with_status(0));
});

test!(build_script_with_dynamic_native_dependency {
    let build = project("builder")
        .file("Cargo.toml", r#"
            [package]
            name = "builder"
            version = "0.0.1"
            authors = []

            [lib]
            name = "builder"
            crate-type = ["dylib"]
        "#)
        .file("src/lib.rs", r#"
            #[no_mangle]
            pub extern fn foo() {}
        "#);
    assert_that(build.cargo_process("build"),
                execs().with_status(0).with_stderr(""));
    let src = build.root().join("target");
    let lib = fs::readdir(&src).unwrap().into_iter().find(|lib| {
        let lib = lib.filename_str().unwrap();
        lib.starts_with(env::consts::DLL_PREFIX) &&
            lib.ends_with(env::consts::DLL_SUFFIX)
    }).unwrap();
    let libname = lib.filename_str().unwrap();
    let libname = &libname[env::consts::DLL_PREFIX.len()..
                           libname.len() - env::consts::DLL_SUFFIX.len()];

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [build-dependencies.bar]
            path = "bar"
        "#)
        .file("build.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("bar/build.rs", r#"
            use std::env;

            fn main() {
                let src = Path::new(env::var_string("SRC").unwrap());
                println!("cargo:rustc-flags=-L {}", src.dir_path().display());
            }
        "#)
        .file("bar/src/lib.rs", format!(r#"
            pub fn bar() {{
                #[link(name = "{}")]
                extern {{ fn foo(); }}
                unsafe {{ foo() }}
            }}
        "#, libname));

    assert_that(foo.cargo_process("build").env("SRC", Some(lib.as_vec())),
                execs().with_status(0));
});
