use std::fs::{self, File};
use std::io::prelude::*;

use support::{project, execs};
use support::paths::CargoPathExt;
use hamcrest::{assert_that, existing_file, existing_dir};

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
                std::process::exit(101);
            }
        "#);
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stdout(&format!("\
[COMPILING] foo v0.5.0 ({url})
[RUNNING] `rustc build.rs --crate-name build_script_build --crate-type bin [..]`
[RUNNING] `[..]build-script-build[..]`
",
url = p.url()))
                       .with_stderr(&format!("\
[ERROR] failed to run custom build command for `foo v0.5.0 ({})`
Process didn't exit successfully: `[..]build[..]build-script-build[..]` \
    (exit code: 101)",
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
            use std::io::prelude::*;
            use std::path::Path;
            use std::fs;

            fn main() {{
                let _target = env::var("TARGET").unwrap();
                let _ncpus = env::var("NUM_JOBS").unwrap();
                let _dir = env::var("CARGO_MANIFEST_DIR").unwrap();

                let opt = env::var("OPT_LEVEL").unwrap();
                assert_eq!(opt, "0");

                let opt = env::var("PROFILE").unwrap();
                assert_eq!(opt, "debug");

                let debug = env::var("DEBUG").unwrap();
                assert_eq!(debug, "true");

                let out = env::var("OUT_DIR").unwrap();
                assert!(out.starts_with(r"{0}"));
                assert!(fs::metadata(&out).map(|m| m.is_dir()).unwrap_or(false));

                let _host = env::var("HOST").unwrap();

                let _feat = env::var("CARGO_FEATURE_FOO").unwrap();
            }}
        "#,
        p.root().join("target").join("debug").join("build").display());

    let p = p.file("bar/build.rs", &file_content);


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
                       .with_stderr(&format!("\
[ERROR] Only `-l` and `-L` flags are allowed in build script of `foo v0.5.0 ({})`: \
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
                       .with_stdout(&format!("\
[COMPILING] bar v0.5.0 ({url})
[RUNNING] `rustc {dir}{sep}src{sep}lib.rs --crate-name test --crate-type lib -g \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target \
        --emit=dep-info,link \
        -L {dir}{sep}target \
        -L {dir}{sep}target{sep}deps`
", sep = path::SEP,
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
[ERROR] package `foo v0.5.0 (file://[..])` specifies that it links to `a` but does \
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
[ERROR] native library `a` is being linked to by more than one package, and can only be \
linked to by one package

  [..] v0.5.0 (file://[..])
  [..] v0.5.0 (file://[..])
"));
});

test!(overrides_and_links {
    let target = ::rustc_host();

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
                assert_eq!(env::var("DEP_FOO_FOO").ok().expect("FOO missing"),
                           "bar");
                assert_eq!(env::var("DEP_FOO_BAR").ok().expect("BAR missing"),
                           "baz");
            }
        "#)
        .file(".cargo/config", &format!(r#"
            [target.{}.foo]
            rustc-flags = "-L foo -L bar"
            foo = "bar"
            bar = "baz"
        "#, target))
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
                       .with_stdout("\
[..]
[..]
[..]
[..]
[..]
[RUNNING] `rustc [..] --crate-name foo [..] -L foo -L bar[..]`
"));
});

test!(unused_overrides {
    let target = ::rustc_host();

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
        .file(".cargo/config", &format!(r#"
            [target.{}.foo]
            rustc-flags = "-L foo -L bar"
            foo = "bar"
            bar = "baz"
        "#, target));

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
                assert_eq!(env::var("DEP_FOO_FOO").unwrap(), "bar");
                assert_eq!(env::var("DEP_FOO_BAR").unwrap(), "baz");
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
            use std::env;
            fn main() {
                let lib = env::var("CARGO_MANIFEST_LINKS").unwrap();
                assert_eq!(lib, "foo");

                println!("cargo:foo=bar");
                println!("cargo:bar=baz");
            }
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
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

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0)
                       .with_stdout("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc [..] --crate-name foo [..]`
"));
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
            use std::time::Duration;
            fn main() {
                println!("cargo:foo=bar");
                println!("cargo:bar=baz");
                std::thread::sleep(Duration::from_millis(500));
            }
        "#);
    a.build();
    a.root().move_into_the_past().unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
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
                assert_eq!(env::var("DEP_FOO_FOO").unwrap(), "bar");
                assert_eq!(env::var("DEP_FOO_BAR").unwrap(), "baz");
            }
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    File::create(&p.root().join("some-new-file")).unwrap();
    p.root().move_into_the_past().unwrap();

    assert_that(p.cargo("build").arg("-v"),
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

    println!("build");
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    File::create(&p.root().join("src/lib.rs")).unwrap();
    p.root().move_into_the_past().unwrap();

    println!("test");
    assert_that(p.cargo("test").arg("-vj1"),
                execs().with_status(0)
                       .with_stdout("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc [..] --crate-name foo [..]`
[RUNNING] `rustc [..] --crate-name foo [..]`
[RUNNING] `[..]foo-[..][..]`

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

[DOCTEST] foo
[RUNNING] `rustdoc --test [..]`

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

"));

    println!("doc");
    assert_that(p.cargo("doc").arg("-v"),
                execs().with_status(0)
                       .with_stdout("\
[DOCUMENTING] foo v0.5.0 (file://[..])
[RUNNING] `rustdoc [..]`
"));

    File::create(&p.root().join("src/main.rs")).unwrap()
         .write_all(b"fn main() {}").unwrap();
    println!("run");
    assert_that(p.cargo("run"),
                execs().with_status(0)
                       .with_stdout("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `target[..]foo[..]`
"));
});

test!(propagation_of_l_flags {
    let target = ::rustc_host();
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
        .file(".cargo/config", &format!(r#"
            [target.{}.foo]
            rustc-flags = "-L foo"
        "#, target));

    assert_that(p.cargo_process("build").arg("-v").arg("-j1"),
                execs().with_status(0)
                       .with_stdout_contains("\
[RUNNING] `rustc [..] --crate-name a [..]-L bar[..]-L foo[..]`
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc [..] --crate-name foo [..] -L bar -L foo`
"));
});

test!(propagation_of_l_flags_new {
    let target = ::rustc_host();
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
                println!("cargo:rustc-link-search=bar");
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
        .file(".cargo/config", &format!(r#"
            [target.{}.foo]
            rustc-link-search = ["foo"]
        "#, target));

    assert_that(p.cargo_process("build").arg("-v").arg("-j1"),
                execs().with_status(0)
                       .with_stdout_contains("\
[RUNNING] `rustc [..] --crate-name a [..]-L bar[..]-L foo[..]`
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc [..] --crate-name foo [..] -L bar -L foo`
"));
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
                       .with_stdout("\
[COMPILING] a v0.5.0 (file://[..])
[RUNNING] `rustc [..] --crate-name a [..]`
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc build.rs [..] --extern a=[..]`
[RUNNING] `[..]foo-[..]build-script-build[..]`
[RUNNING] `rustc [..] --crate-name foo [..]`
"));
});

test!(build_deps_not_for_normal {
    let target = ::rustc_host();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
            [build-dependencies.aaaaa]
            path = "a"
        "#)
        .file("src/lib.rs", "extern crate aaaaa;")
        .file("build.rs", "
            extern crate aaaaa;
            fn main() {}
        ")
        .file("a/Cargo.toml", r#"
            [project]
            name = "aaaaa"
            version = "0.5.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v").arg("--target").arg(&target),
                execs().with_status(101)
                       .with_stderr("\
[..]lib.rs[..] error: can't find crate for `aaaaa`[..]
[..]lib.rs[..] extern crate aaaaa;
[..]           ^~~~~~~~~~~~~~~~~~~
error: aborting due to previous error
[ERROR] Could not compile `foo`.

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
                       .with_stdout("\
[COMPILING] b v0.5.0 (file://[..])
[RUNNING] `rustc [..] --crate-name b [..]`
[COMPILING] a v0.5.0 (file://[..])
[RUNNING] `rustc a[..]build.rs [..] --extern b=[..]`
[RUNNING] `[..]a-[..]build-script-build[..]`
[RUNNING] `rustc [..]lib.rs --crate-name a --crate-type lib -g \
    -C metadata=[..] -C extra-filename=-[..] \
    --out-dir [..]target[..]deps --emit=dep-info,link \
    -L [..]target[..]deps -L [..]target[..]deps`
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc build.rs --crate-name build_script_build --crate-type bin \
    -g \
    --out-dir [..]build[..]foo-[..] --emit=dep-info,link \
    -L [..]target[..]debug -L [..]target[..]deps \
    --extern a=[..]liba-[..].rlib`
[RUNNING] `[..]foo-[..]build-script-build[..]`
[RUNNING] `rustc [..]lib.rs --crate-name foo --crate-type lib -g \
    --out-dir [..]target[..]debug --emit=dep-info,link \
    -L [..]target[..]debug -L [..]target[..]deps`
"));
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
            use std::fs::File;
            use std::path::Path;
            fn main() {
                let out = env::var("OUT_DIR").unwrap();
                File::create(Path::new(&out).join("foo")).unwrap();
            }
        "#);

    // Make the file
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    // Change to asserting that it's there
    File::create(&p.root().join("build.rs")).unwrap().write_all(br#"
        use std::env;
        use std::old_io::File;
        fn main() {
            let out = env::var("OUT_DIR").unwrap();
            File::open(&Path::new(&out).join("foo")).unwrap();
        }
    "#).unwrap();
    p.root().move_into_the_past().unwrap();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0));

    // Run a fresh build where file should be preserved
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0));

    // One last time to make sure it's still there.
    File::create(&p.root().join("foo")).unwrap();
    assert_that(p.cargo("build").arg("-v"),
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
                println!("cargo:rustc-flags=-l static=foo");
            }
        "#);
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stdout("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc build.rs [..]`
[RUNNING] `[..]foo-[..]build-script-build[..]`
[RUNNING] `rustc [..] --crate-name foo [..] -L foo -l static=foo`
"));
});

test!(output_separate_lines_new {
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
                println!("cargo:rustc-link-search=foo");
                println!("cargo:rustc-link-lib=static=foo");
            }
        "#);
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stdout("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc build.rs [..]`
[RUNNING] `[..]foo-[..]build-script-build[..]`
[RUNNING] `rustc [..] --crate-name foo [..] -L foo -l static=foo`
"));
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
            use std::fs::File;
            use std::io::prelude::*;
            use std::path::PathBuf;

            fn main() {
                let dst = PathBuf::from(env::var("OUT_DIR").unwrap());
                let mut f = File::create(&dst.join("hello.rs")).unwrap();
                f.write_all(b"
                    pub fn message() -> &'static str {
                        \"Hello, World!\"
                    }
                ").unwrap();
            }
        "#);
    assert_that(p.cargo_process("run"),
                execs().with_status(0)
                       .with_stdout("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `target[..]foo`
Hello, World!
"));

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
[ERROR] failed to parse manifest at `[..]`

Caused by:
  no targets specified in the manifest
  either src/lib.rs, src/main.rs, a [lib] section, or [[bin]] section must be present"));
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
            path = "../a"
        "#)
        .file("b/src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v"),
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
            use std::io::prelude::*;
            use std::fs::File;
            use std::path::PathBuf;

            fn main() {
                let out = PathBuf::from(env::var("OUT_DIR").unwrap());
                File::create(out.join("foo.rs")).unwrap().write_all(b"
                    fn foo() -> i32 { 1 }
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
            plugin = true
        "#)
        .file("src/lib.rs", r#"
            #[no_mangle]
            pub extern fn foo() {}
        "#);
    assert_that(build.cargo_process("build"),
                execs().with_status(0));

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
            use std::path::PathBuf;

            fn main() {
                let src = PathBuf::from(env::var("SRC").unwrap());
                println!("cargo:rustc-link-search={}/target/debug",
                         src.display());
            }
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {
                #[cfg_attr(not(target_env = "msvc"), link(name = "builder"))]
                #[cfg_attr(target_env = "msvc", link(name = "builder.dll"))]
                extern { fn foo(); }
                unsafe { foo() }
            }
        "#);

    assert_that(foo.cargo_process("build").env("SRC", build.root()),
                execs().with_status(0));
});

test!(profile_and_opt_level_set_correctly {
    let build = project("builder")
        .file("Cargo.toml", r#"
            [package]
            name = "builder"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
              use std::env;

              fn main() {
                  assert_eq!(env::var("OPT_LEVEL").unwrap(), "3");
                  assert_eq!(env::var("PROFILE").unwrap(), "release");
                  assert_eq!(env::var("DEBUG").unwrap(), "false");
              }
        "#);
    assert_that(build.cargo_process("bench"),
                execs().with_status(0));
});

test!(build_script_with_lto {
    let build = project("builder")
        .file("Cargo.toml", r#"
            [package]
            name = "builder"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [profile.dev]
            lto = true
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
              fn main() {
              }
        "#);
    assert_that(build.cargo_process("build"),
                execs().with_status(0));
});

test!(test_duplicate_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = []
            build = "build.rs"

            [dependencies.bar]
            path = "bar"

            [build-dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() { bar::do_nothing() }
        "#)
        .file("build.rs", r#"
            extern crate bar;
            fn main() { bar::do_nothing() }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("bar/src/lib.rs", "pub fn do_nothing() {}");

    assert_that(p.cargo_process("build"), execs().with_status(0));
});

test!(cfg_feedback {
    let build = project("builder")
        .file("Cargo.toml", r#"
            [package]
            name = "builder"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("src/main.rs", "
            #[cfg(foo)]
            fn main() {}
        ")
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-cfg=foo");
            }
        "#);
    assert_that(build.cargo_process("build").arg("-v"),
                execs().with_status(0));
});

test!(cfg_override {
    let target = ::rustc_host();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            links = "a"
            build = "build.rs"
        "#)
        .file("src/main.rs", "
            #[cfg(foo)]
            fn main() {}
        ")
        .file("build.rs", "")
        .file(".cargo/config", &format!(r#"
            [target.{}.a]
            rustc-cfg = ["foo"]
        "#, target));

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
});

test!(cfg_test {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-cfg=foo");
            }
        "#)
        .file("src/lib.rs", r#"
            ///
            /// ```
            /// extern crate foo;
            ///
            /// fn main() {
            ///     foo::foo()
            /// }
            /// ```
            ///
            #[cfg(foo)]
            pub fn foo() {}

            #[cfg(foo)]
            #[test]
            fn test_foo() {
                foo()
            }
        "#)
        .file("tests/test.rs", r#"
            #[cfg(foo)]
            #[test]
            fn test_bar() {}
        "#);
    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_stdout(format!("\
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] [..] build.rs [..]
[RUNNING] [..]build-script-build[..]
[RUNNING] [..] --cfg foo[..]
[RUNNING] [..] --cfg foo[..]
[RUNNING] [..] --cfg foo[..]
[RUNNING] [..]foo-[..]

running 1 test
test test_foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

[RUNNING] [..]test-[..]

running 1 test
test test_bar ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

[DOCTEST] foo
[RUNNING] [..] --cfg foo[..]

running 1 test
test foo_0 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

", dir = p.url())));
});

test!(cfg_doc {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [dependencies.bar]
            path = "bar"
        "#)
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-cfg=foo");
            }
        "#)
        .file("src/lib.rs", r#"
            #[cfg(foo)]
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("bar/build.rs", r#"
            fn main() {
                println!("cargo:rustc-cfg=bar");
            }
        "#)
        .file("bar/src/lib.rs", r#"
            #[cfg(bar)]
            pub fn bar() {}
        "#);
    assert_that(p.cargo_process("doc"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/fn.foo.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/fn.bar.html"), existing_file());
});

test!(cfg_override_test {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
            links = "a"
        "#)
        .file("build.rs", "")
        .file(".cargo/config", &format!(r#"
            [target.{}.a]
            rustc-cfg = ["foo"]
        "#, ::rustc_host()))
        .file("src/lib.rs", r#"
            ///
            /// ```
            /// extern crate foo;
            ///
            /// fn main() {
            ///     foo::foo()
            /// }
            /// ```
            ///
            #[cfg(foo)]
            pub fn foo() {}

            #[cfg(foo)]
            #[test]
            fn test_foo() {
                foo()
            }
        "#)
        .file("tests/test.rs", r#"
            #[cfg(foo)]
            #[test]
            fn test_bar() {}
        "#);
    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_stdout(format!("\
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] `[..]`
[RUNNING] `[..]`
[RUNNING] `[..]`
[RUNNING] [..]foo-[..]

running 1 test
test test_foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

[RUNNING] [..]test-[..]

running 1 test
test test_bar ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

[DOCTEST] foo
[RUNNING] [..] --cfg foo[..]

running 1 test
test foo_0 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

", dir = p.url())));
});

test!(cfg_override_doc {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
            links = "a"

            [dependencies.bar]
            path = "bar"
        "#)
        .file(".cargo/config", &format!(r#"
            [target.{target}.a]
            rustc-cfg = ["foo"]
            [target.{target}.b]
            rustc-cfg = ["bar"]
        "#, target = ::rustc_host()))
        .file("build.rs", "")
        .file("src/lib.rs", r#"
            #[cfg(foo)]
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
            build = "build.rs"
            links = "b"
        "#)
        .file("bar/build.rs", "")
        .file("bar/src/lib.rs", r#"
            #[cfg(bar)]
            pub fn bar() {}
        "#) ;
    assert_that(p.cargo_process("doc"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/fn.foo.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/fn.bar.html"), existing_file());
});

test!(flags_go_into_tests {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            b = { path = "b" }
        "#)
        .file("src/lib.rs", "")
        .file("tests/foo.rs", "")
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.5.0"
            authors = []
            [dependencies]
            a = { path = "../a" }
        "#)
        .file("b/src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("a/src/lib.rs", "")
        .file("a/build.rs", r#"
            fn main() {
                println!("cargo:rustc-link-search=test");
            }
        "#);

    assert_that(p.cargo_process("test").arg("-v").arg("--test=foo"),
                execs().with_status(0).with_stdout("\
[COMPILING] a v0.5.0 ([..]
[RUNNING] `rustc a[..]build.rs [..]`
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc a[..]src[..]lib.rs [..] -L test[..]`
[COMPILING] b v0.5.0 ([..]
[RUNNING] `rustc b[..]src[..]lib.rs [..] -L test[..]`
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc src[..]lib.rs [..] -L test[..]`
[RUNNING] `rustc tests[..]foo.rs [..] -L test[..]`
[RUNNING] `[..]foo-[..]`

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

"));

    assert_that(p.cargo("test").arg("-v").arg("-pb").arg("--lib"),
                execs().with_status(0).with_stdout("\
[FRESH] a v0.5.0 ([..]
[COMPILING] b v0.5.0 ([..]
[RUNNING] `rustc b[..]src[..]lib.rs [..] -L test[..]`
[RUNNING] `[..]b-[..]`

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

"));
});

test!(diamond_passes_args_only_once {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            a = { path = "a" }
            b = { path = "b" }
        "#)
        .file("src/lib.rs", "")
        .file("tests/foo.rs", "")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            [dependencies]
            b = { path = "../b" }
            c = { path = "../c" }
        "#)
        .file("a/src/lib.rs", "")
        .file("b/Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.5.0"
            authors = []
            [dependencies]
            c = { path = "../c" }
        "#)
        .file("b/src/lib.rs", "")
        .file("c/Cargo.toml", r#"
            [project]
            name = "c"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("c/build.rs", r#"
            fn main() {
                println!("cargo:rustc-link-search=native=test");
            }
        "#)
        .file("c/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] c v0.5.0 ([..]
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
[RUNNING] `rustc [..]`
[COMPILING] b v0.5.0 ([..]
[RUNNING] `rustc [..]`
[COMPILING] a v0.5.0 ([..]
[RUNNING] `rustc [..]`
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `[..]rlib -L native=test`
"));
});

test!(adding_an_override_invalidates {
    let target = ::rustc_host();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            links = "foo"
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", "")
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-link-search=native=foo");
            }
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
[RUNNING] `rustc [..] -L native=foo`
"));

    File::create(p.root().join(".cargo/config")).unwrap().write_all(format!("
        [target.{}.foo]
        rustc-link-search = [\"native=bar\"]
    ", target).as_bytes()).unwrap();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=bar`
"));
});

test!(changing_an_override_invalidates {
    let target = ::rustc_host();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            links = "foo"
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", &format!("
            [target.{}.foo]
            rustc-link-search = [\"native=foo\"]
        ", target))
        .file("build.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=foo`
"));

    File::create(p.root().join(".cargo/config")).unwrap().write_all(format!("
        [target.{}.foo]
        rustc-link-search = [\"native=bar\"]
    ", target).as_bytes()).unwrap();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=bar`
"));
});

test!(rebuild_only_on_explicit_paths {
    let p = project("a")
        .file("Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rerun-if-changed=foo");
                println!("cargo:rerun-if-changed=bar");
            }
        "#);
    p.build();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0));

    // files don't exist, so should always rerun if they don't exist
    println!("run without");
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] a v0.5.0 ([..])
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc src[..]lib.rs [..]`
"));

    ::sleep_ms(1000);
    File::create(p.root().join("foo")).unwrap();
    File::create(p.root().join("bar")).unwrap();

    // now the exist, so run once, catch the mtime, then shouldn't run again
    println!("run with");
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] a v0.5.0 ([..])
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc src[..]lib.rs [..]`
"));

    println!("run with2");
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[FRESH] a v0.5.0 ([..])
"));

    ::sleep_ms(1000);

    // random other files do not affect freshness
    println!("run baz");
    File::create(p.root().join("baz")).unwrap();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[FRESH] a v0.5.0 ([..])
"));

    // but changing dependent files does
    println!("run foo change");
    File::create(p.root().join("foo")).unwrap();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] a v0.5.0 ([..])
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc src[..]lib.rs [..]`
"));

    // .. as does deleting a file
    println!("run foo delete");
    fs::remove_file(p.root().join("bar")).unwrap();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stdout("\
[COMPILING] a v0.5.0 ([..])
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc src[..]lib.rs [..]`
"));
});


test!(doctest_recieves_build_link_args {
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
        "#)
        .file("a/src/lib.rs", "")
        .file("a/build.rs", r#"
            fn main() {
                println!("cargo:rustc-link-search=native=bar");
            }
        "#);

    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0)
                       .with_stdout_contains("\
[RUNNING] `rustdoc --test [..] --crate-name foo [..]-L native=bar[..]`
"));
});

test!(please_respect_the_dag {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [dependencies]
            a = { path = 'a' }
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-link-search=native=foo");
            }
        "#)
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            links = "bar"
            build = "build.rs"
        "#)
        .file("a/src/lib.rs", "")
        .file("a/build.rs", r#"
            fn main() {
                println!("cargo:rustc-link-search=native=bar");
            }
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stdout_contains("\
[RUNNING] `rustc [..] -L native=foo -L native=bar[..]`
"));
});

test!(non_utf8_output {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("build.rs", r#"
            use std::io::prelude::*;

            fn main() {
                let mut out = std::io::stdout();
                // print something that's not utf8
                out.write_all(b"\xff\xff\n").unwrap();

                // now print some cargo metadata that's utf8
                println!("cargo:rustc-cfg=foo");

                // now print more non-utf8
                out.write_all(b"\xff\xff\n").unwrap();
            }
        "#)
        .file("src/main.rs", r#"
            #[cfg(foo)]
            fn main() {}
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
});

test!(custom_target_dir {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            [build]
            target-dir = 'test'
        "#)
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("a/build.rs", "fn main() {}")
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
});
