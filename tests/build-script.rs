extern crate cargotest;
extern crate hamcrest;

use std::fs::{self, File};
use std::io::prelude::*;

use cargotest::{rustc_host, sleep_ms};
use cargotest::support::{project, execs};
use cargotest::support::paths::CargoPathExt;
use cargotest::support::registry::Package;
use hamcrest::{assert_that, existing_file, existing_dir};

#[test]
fn custom_build_script_failed() {
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
                       .with_stderr(&format!("\
[COMPILING] foo v0.5.0 ({url})
[RUNNING] `rustc --crate-name build_script_build build.rs --crate-type bin [..]`
[RUNNING] `[..][/]build-script-build`
[ERROR] failed to run custom build command for `foo v0.5.0 ({url})`
process didn't exit successfully: `[..][/]build-script-build` (exit code: 101)",
url = p.url())));
}

#[test]
fn custom_build_env_vars() {
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

                let rustc = env::var("RUSTC").unwrap();
                assert_eq!(rustc, "rustc");

                let rustdoc = env::var("RUSTDOC").unwrap();
                assert_eq!(rustdoc, "rustdoc");
            }}
        "#,
        p.root().join("target").join("debug").join("build").display());

    let p = p.file("bar/build.rs", &file_content);


    assert_that(p.cargo_process("build").arg("--features").arg("bar_feat"),
                execs().with_status(0));
}

#[test]
fn custom_build_script_wrong_rustc_flags() {
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
                       .with_stderr_contains(&format!("\
[ERROR] Only `-l` and `-L` flags are allowed in build script of `foo v0.5.0 ({})`: \
`-aaa -bbb`",
p.url())));
}

/*
#[test]
fn custom_build_script_rustc_flags() {
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
                       .with_stderr(&format!("\
[COMPILING] bar v0.5.0 ({url})
[RUNNING] `rustc --crate-name test {dir}{sep}src{sep}lib.rs --crate-type lib -C debuginfo=2 \
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
}
*/

#[test]
fn links_no_build_cmd() {
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
}

#[test]
fn links_duplicates() {
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
}

#[test]
fn overrides_and_links() {
    let target = rustc_host();

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
                       .with_stderr("\
[..]
[..]
[..]
[..]
[..]
[RUNNING] `rustc --crate-name foo [..] -L foo -L bar`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn unused_overrides() {
    let target = rustc_host();

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
}

#[test]
fn links_passes_env_vars() {
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
}

#[test]
fn only_rerun_build_script() {
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
    p.root().move_into_the_past();

    File::create(&p.root().join("some-new-file")).unwrap();
    p.root().move_into_the_past();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `[..][/]build-script-build`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn rebuild_continues_to_pass_env_vars() {
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
    a.root().move_into_the_past();

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
    p.root().move_into_the_past();

    File::create(&p.root().join("some-new-file")).unwrap();
    p.root().move_into_the_past();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0));
}

#[test]
fn testing_and_such() {
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
    p.root().move_into_the_past();

    File::create(&p.root().join("src/lib.rs")).unwrap();
    p.root().move_into_the_past();

    println!("test");
    assert_that(p.cargo("test").arg("-vj1"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `[..][/]build-script-build`
[RUNNING] `rustc --crate-name foo [..]`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..][/]foo-[..][EXE]`
[DOCTEST] foo
[RUNNING] `rustdoc --test [..]`")
                       .with_stdout_contains_n("running 0 tests", 2));

    println!("doc");
    assert_that(p.cargo("doc").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[DOCUMENTING] foo v0.5.0 (file://[..])
[RUNNING] `rustdoc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    File::create(&p.root().join("src/main.rs")).unwrap()
         .write_all(b"fn main() {}").unwrap();
    println!("run");
    assert_that(p.cargo("run"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.5.0 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target[/]debug[/]foo[EXE]`
"));
}

#[test]
fn propagation_of_l_flags() {
    let target = rustc_host();
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
                       .with_stderr_contains("\
[RUNNING] `rustc --crate-name a [..] -L bar[..]-L foo[..]`
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc --crate-name foo [..] -L bar -L foo`
"));
}

#[test]
fn propagation_of_l_flags_new() {
    let target = rustc_host();
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
                       .with_stderr_contains("\
[RUNNING] `rustc --crate-name a [..] -L bar[..]-L foo[..]`
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc --crate-name foo [..] -L bar -L foo`
"));
}

#[test]
fn build_deps_simple() {
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
                       .with_stderr("\
[COMPILING] a v0.5.0 (file://[..])
[RUNNING] `rustc --crate-name a [..]`
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc [..] build.rs [..] --extern a=[..]`
[RUNNING] `[..][/]foo-[..][/]build-script-build`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn build_deps_not_for_normal() {
    let target = rustc_host();
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
                       .with_stderr_contains("\
[..]can't find crate for `aaaaa`[..]
")
                       .with_stderr_contains("\
[ERROR] Could not compile `foo`.

Caused by:
  process didn't exit successfully: [..]
"));
}

#[test]
fn build_cmd_with_a_build_cmd() {
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
                       .with_stderr("\
[COMPILING] b v0.5.0 (file://[..])
[RUNNING] `rustc --crate-name b [..]`
[COMPILING] a v0.5.0 (file://[..])
[RUNNING] `rustc [..] a[/]build.rs [..] --extern b=[..]`
[RUNNING] `[..][/]a-[..][/]build-script-build`
[RUNNING] `rustc --crate-name a [..]lib.rs --crate-type lib \
    --emit=dep-info,link -C debuginfo=2 \
    -C metadata=[..] \
    --out-dir [..]target[/]debug[/]deps \
    -L [..]target[/]debug[/]deps`
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc --crate-name build_script_build build.rs --crate-type bin \
    --emit=dep-info,link \
    -C debuginfo=2 -C metadata=[..] --out-dir [..] \
    -L [..]target[/]debug[/]deps \
    --extern a=[..]liba[..].rlib`
[RUNNING] `[..][/]foo-[..][/]build-script-build`
[RUNNING] `rustc --crate-name foo [..]lib.rs --crate-type lib \
    --emit=dep-info,link -C debuginfo=2 \
    -C metadata=[..] \
    --out-dir [..] \
    -L [..]target[/]debug[/]deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn out_dir_is_preserved() {
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
    p.root().move_into_the_past();

    // Change to asserting that it's there
    File::create(&p.root().join("build.rs")).unwrap().write_all(br#"
        use std::env;
        use std::old_io::File;
        fn main() {
            let out = env::var("OUT_DIR").unwrap();
            File::open(&Path::new(&out).join("foo")).unwrap();
        }
    "#).unwrap();
    p.root().move_into_the_past();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0));

    // Run a fresh build where file should be preserved
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0));

    // One last time to make sure it's still there.
    File::create(&p.root().join("foo")).unwrap();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0));
}

#[test]
fn output_separate_lines() {
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
                       .with_stderr_contains("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[..][/]foo-[..][/]build-script-build`
[RUNNING] `rustc --crate-name foo [..] -L foo -l static=foo`
[ERROR] could not find native static library [..]
"));
}

#[test]
fn output_separate_lines_new() {
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
                       .with_stderr_contains("\
[COMPILING] foo v0.5.0 (file://[..])
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[..][/]foo-[..][/]build-script-build`
[RUNNING] `rustc --crate-name foo [..] -L foo -l static=foo`
[ERROR] could not find native static library [..]
"));
}

#[cfg(not(windows))] // FIXME(#867)
#[test]
fn code_generation() {
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
    p.build();

    assert_that(p.cargo("run"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.5.0 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target[/]debug[/]foo`")
                       .with_stdout("\
Hello, World!
"));

    assert_that(p.cargo("test"),
                execs().with_status(0));
}

#[test]
fn release_with_build_script() {
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
}

#[test]
fn build_script_only() {
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
}

#[test]
fn shared_dep_with_a_build_script() {
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
}

#[test]
fn transitive_dep_host() {
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
}

#[test]
fn test_a_lib_with_a_build_command() {
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
}

#[test]
fn test_dev_dep_build_script() {
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
}

#[test]
fn build_script_with_dynamic_native_dependency() {

    let workspace = project("ws")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["builder", "foo"]
        "#);
    workspace.build();

    let build = project("ws/builder")
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
    build.build();

    let foo = project("ws/foo")
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
                println!("cargo:rustc-link-search=native={}/target/debug/deps",
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
    foo.build();

    assert_that(build.cargo("build").arg("-v")
                .env("RUST_LOG", "cargo::ops::cargo_rustc"),
                execs().with_status(0));

    assert_that(foo.cargo("build").arg("-v").env("SRC", build.root())
                .env("RUST_LOG", "cargo::ops::cargo_rustc"),
                execs().with_status(0));
}

#[test]
fn profile_and_opt_level_set_correctly() {
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
}

#[test]
fn build_script_with_lto() {
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
}

#[test]
fn test_duplicate_deps() {
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
}

#[test]
fn cfg_feedback() {
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
}

#[test]
fn cfg_override() {
    let target = rustc_host();

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
}

#[test]
fn cfg_test() {
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
                execs().with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] [..] build.rs [..]
[RUNNING] `[..][/]build-script-build`
[RUNNING] [..] --cfg foo[..]
[RUNNING] [..] --cfg foo[..]
[RUNNING] [..] --cfg foo[..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..][/]foo-[..][EXE]`
[RUNNING] `[..][/]test-[..][EXE]`
[DOCTEST] foo
[RUNNING] [..] --cfg foo[..]", dir = p.url()))
                       .with_stdout_contains("test test_foo ... ok")
                       .with_stdout_contains("test test_bar ... ok")
                       .with_stdout_contains_n("test [..] ... ok", 3));
}

#[test]
fn cfg_doc() {
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
}

#[test]
fn cfg_override_test() {
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
        "#, rustc_host()))
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
                execs().with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] `[..]`
[RUNNING] `[..]`
[RUNNING] `[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..][/]foo-[..][EXE]`
[RUNNING] `[..][/]test-[..][EXE]`
[DOCTEST] foo
[RUNNING] [..] --cfg foo[..]", dir = p.url()))
                       .with_stdout_contains("test test_foo ... ok")
                       .with_stdout_contains("test test_bar ... ok")
                       .with_stdout_contains_n("test [..] ... ok", 3));
}

#[test]
fn cfg_override_doc() {
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
        "#, target = rustc_host()))
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
}

#[test]
fn env_build() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("src/main.rs", r#"
            const FOO: &'static str = env!("FOO");
            fn main() {
                println!("{}", FOO);
            }
        "#)
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-env=FOO=foo");
            }
        "#);
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    assert_that(p.cargo("run").arg("-v"),
                execs().with_status(0).with_stdout("foo\n"));
}

#[test]
fn env_test() {
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
                println!("cargo:rustc-env=FOO=foo");
            }
        "#)
        .file("src/lib.rs", r#"
            pub const FOO: &'static str = env!("FOO");
        "#)
        .file("tests/test.rs", r#"
            extern crate foo;

            #[test]
            fn test_foo() {
                assert_eq!("foo", foo::FOO);
            }
        "#);
    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_stderr(format!("\
[COMPILING] foo v0.0.1 ({dir})
[RUNNING] [..] build.rs [..]
[RUNNING] `[..][/]build-script-build`
[RUNNING] [..] --crate-name foo[..]
[RUNNING] [..] --crate-name foo[..]
[RUNNING] [..] --crate-name test[..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..][/]foo-[..][EXE]`
[RUNNING] `[..][/]test-[..][EXE]`
[DOCTEST] foo
[RUNNING] [..] --crate-name foo[..]", dir = p.url()))
                       .with_stdout_contains_n("running 0 tests", 2)
                       .with_stdout_contains("test test_foo ... ok"));
}

#[test]
fn env_doc() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#)
        .file("src/main.rs", r#"
            const FOO: &'static str = env!("FOO");
            fn main() {}
        "#)
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-env=FOO=foo");
            }
        "#);
    assert_that(p.cargo_process("doc").arg("-v"),
                execs().with_status(0));
}

#[test]
fn flags_go_into_tests() {
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
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] a v0.5.0 ([..]
[RUNNING] `rustc [..] a[/]build.rs [..]`
[RUNNING] `[..][/]build-script-build`
[RUNNING] `rustc [..] a[/]src[/]lib.rs [..] -L test[..]`
[COMPILING] b v0.5.0 ([..]
[RUNNING] `rustc [..] b[/]src[/]lib.rs [..] -L test[..]`
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] src[/]lib.rs [..] -L test[..]`
[RUNNING] `rustc [..] tests[/]foo.rs [..] -L test[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..][/]foo-[..][EXE]`")
                       .with_stdout_contains("running 0 tests"));

    assert_that(p.cargo("test").arg("-v").arg("-pb").arg("--lib"),
                execs().with_status(0)
                       .with_stderr("\
[FRESH] a v0.5.0 ([..]
[COMPILING] b v0.5.0 ([..]
[RUNNING] `rustc [..] b[/]src[/]lib.rs [..] -L test[..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..][/]b-[..][EXE]`")
                       .with_stdout_contains("running 0 tests"));
}

#[test]
fn diamond_passes_args_only_once() {
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
                execs().with_status(0).with_stderr("\
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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn adding_an_override_invalidates() {
    let target = rustc_host();
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
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
[RUNNING] `rustc [..] -L native=foo`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    File::create(p.root().join(".cargo/config")).unwrap().write_all(format!("
        [target.{}.foo]
        rustc-link-search = [\"native=bar\"]
    ", target).as_bytes()).unwrap();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=bar`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn changing_an_override_invalidates() {
    let target = rustc_host();
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
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=foo`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    File::create(p.root().join(".cargo/config")).unwrap().write_all(format!("
        [target.{}.foo]
        rustc-link-search = [\"native=bar\"]
    ", target).as_bytes()).unwrap();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..] -L native=bar`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}


#[test]
fn fresh_builds_possible_with_link_libs() {
    // The bug is non-deterministic. Sometimes you can get a fresh build
    let target = rustc_host();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            links = "nativefoo"
            build = "build.rs"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", &format!("
            [target.{}.nativefoo]
            rustc-link-lib = [\"a\"]
            rustc-link-search = [\"./b\"]
            rustc-flags = \"-l z -L ./\"
        ", target))
        .file("build.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build")
                 .arg("-v")
                 .env("RUST_LOG", "cargo::ops::cargo_rustc::fingerprint=info"),
                execs().with_status(0).with_stderr("\
[FRESH] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}


#[test]
fn fresh_builds_possible_with_multiple_metadata_overrides() {
    // The bug is non-deterministic. Sometimes you can get a fresh build
    let target = rustc_host();
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
            a = \"\"
            b = \"\"
            c = \"\"
            d = \"\"
            e = \"\"
        ", target))
        .file("build.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.5.0 ([..]
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build")
                 .arg("-v")
                 .env("RUST_LOG", "cargo::ops::cargo_rustc::fingerprint=info"),
                execs().with_status(0).with_stderr("\
[FRESH] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}


#[test]
fn rebuild_only_on_explicit_paths() {
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
                execs().with_status(0).with_stderr("\
[COMPILING] a v0.5.0 ([..])
[RUNNING] `[..][/]build-script-build`
[RUNNING] `rustc [..] src[/]lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    sleep_ms(1000);
    File::create(p.root().join("foo")).unwrap();
    File::create(p.root().join("bar")).unwrap();

    // now the exist, so run once, catch the mtime, then shouldn't run again
    println!("run with");
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] a v0.5.0 ([..])
[RUNNING] `[..][/]build-script-build`
[RUNNING] `rustc [..] src[/]lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    println!("run with2");
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[FRESH] a v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    sleep_ms(1000);

    // random other files do not affect freshness
    println!("run baz");
    File::create(p.root().join("baz")).unwrap();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[FRESH] a v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    // but changing dependent files does
    println!("run foo change");
    File::create(p.root().join("foo")).unwrap();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] a v0.5.0 ([..])
[RUNNING] `[..][/]build-script-build`
[RUNNING] `rustc [..] src[/]lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    // .. as does deleting a file
    println!("run foo delete");
    fs::remove_file(p.root().join("bar")).unwrap();
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] a v0.5.0 ([..])
[RUNNING] `[..][/]build-script-build`
[RUNNING] `rustc [..] src[/]lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}


#[test]
fn doctest_recieves_build_link_args() {
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
                       .with_stderr_contains("\
[RUNNING] `rustdoc --test [..] --crate-name foo [..]-L native=bar[..]`
"));
}

#[test]
fn please_respect_the_dag() {
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
                       .with_stderr_contains("\
[RUNNING] `rustc [..] -L native=foo -L native=bar[..]`
"));
}

#[test]
fn non_utf8_output() {
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
}

#[test]
fn custom_target_dir() {
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
}

#[test]
fn panic_abort_with_build_scripts() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [profile.release]
            panic = 'abort'

            [dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "extern crate a;")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            build = "build.rs"

            [build-dependencies]
            b = { path = "../b" }
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

    assert_that(p.cargo_process("build").arg("-v").arg("--release"),
                execs().with_status(0));
}

#[test]
fn warnings_emitted() {
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
                println!("cargo:warning=foo");
                println!("cargo:warning=bar");
            }
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
warning: foo
warning: bar
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn warnings_hidden_for_upstream() {
    Package::new("bar", "0.1.0")
            .file("build.rs", r#"
                fn main() {
                    println!("cargo:warning=foo");
                    println!("cargo:warning=bar");
                }
            "#)
            .file("Cargo.toml", r#"
                [project]
                name = "bar"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#)
            .file("src/lib.rs", "")
            .publish();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] bar v0.1.0 ([..])
[COMPILING] bar v0.1.0
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
[RUNNING] `rustc [..]`
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn warnings_printed_on_vv() {
    Package::new("bar", "0.1.0")
            .file("build.rs", r#"
                fn main() {
                    println!("cargo:warning=foo");
                    println!("cargo:warning=bar");
                }
            "#)
            .file("Cargo.toml", r#"
                [project]
                name = "bar"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#)
            .file("src/lib.rs", "")
            .publish();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-vv"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] bar v0.1.0 ([..])
[COMPILING] bar v0.1.0
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
warning: foo
warning: bar
[RUNNING] `rustc [..]`
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn output_shows_on_vv() {
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
            use std::io::prelude::*;

            fn main() {
                std::io::stderr().write_all(b"stderr\n").unwrap();
                std::io::stdout().write_all(b"stdout\n").unwrap();
            }
        "#);

    assert_that(p.cargo_process("build").arg("-vv"),
                execs().with_status(0)
                       .with_stdout("\
stdout
")
                       .with_stderr("\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `rustc [..]`
[RUNNING] `[..]`
stderr
[RUNNING] `rustc [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn links_with_dots() {
    let target = rustc_host();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
            links = "a.b"
        "#)
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-link-search=bar")
            }
        "#)
        .file(".cargo/config", &format!(r#"
            [target.{}.'a.b']
            rustc-link-search = ["foo"]
        "#, target));

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr_contains("\
[RUNNING] `rustc --crate-name foo [..] [..] -L foo[..]`
"));
}

#[test]
fn rustc_and_rustdoc_set_correctly() {
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
                  assert_eq!(env::var("RUSTC").unwrap(), "rustc");
                  assert_eq!(env::var("RUSTDOC").unwrap(), "rustdoc");
              }
        "#);
    assert_that(build.cargo_process("bench"),
                execs().with_status(0));
}

#[test]
fn cfg_env_vars_available() {
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
                let fam = env::var("CARGO_CFG_TARGET_FAMILY").unwrap();
                if cfg!(unix) {
                    assert_eq!(fam, "unix");
                } else {
                    assert_eq!(fam, "windows");
                }
            }
        "#);
    assert_that(build.cargo_process("bench"),
                execs().with_status(0));
}

#[test]
fn switch_features_rerun() {
    let build = project("builder")
        .file("Cargo.toml", r#"
            [package]
            name = "builder"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [features]
            foo = []
        "#)
        .file("src/main.rs", r#"
            fn main() {
                println!(include_str!(concat!(env!("OUT_DIR"), "/output")));
            }
        "#)
        .file("build.rs", r#"
            use std::env;
            use std::fs::File;
            use std::io::Write;
            use std::path::Path;

            fn main() {
                let out_dir = env::var_os("OUT_DIR").unwrap();
                let out_dir = Path::new(&out_dir).join("output");
                let mut f = File::create(&out_dir).unwrap();

                if env::var_os("CARGO_FEATURE_FOO").is_some() {
                    f.write_all(b"foo").unwrap();
                } else {
                    f.write_all(b"bar").unwrap();
                }
            }
        "#);
    build.build();

    assert_that(build.cargo("run").arg("-v").arg("--features=foo"),
                execs().with_status(0).with_stdout("foo\n"));
    assert_that(build.cargo("run").arg("-v"),
                execs().with_status(0).with_stdout("bar\n"));
    assert_that(build.cargo("run").arg("-v").arg("--features=foo"),
                execs().with_status(0).with_stdout("foo\n"));
}

#[test]
fn assume_build_script_when_build_rs_present() {
    let p = project("builder")
        .file("Cargo.toml", r#"
            [package]
            name = "builder"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {
                if ! cfg!(foo) {
                    panic!("the build script was not run");
                }
            }
        "#)
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-cfg=foo");
            }
        "#);
    p.build();

    assert_that(p.cargo("run").arg("-v"),
                execs().with_status(0));
}

#[test]
fn if_build_set_to_false_dont_treat_build_rs_as_build_script() {
    let p = project("builder")
        .file("Cargo.toml", r#"
            [package]
            name = "builder"
            version = "0.0.1"
            authors = []
            build = false
        "#)
        .file("src/main.rs", r#"
            fn main() {
                if cfg!(foo) {
                    panic!("the build script was run");
                }
            }
        "#)
        .file("build.rs", r#"
            fn main() {
                println!("cargo:rustc-cfg=foo");
            }
        "#);
    p.build();

    assert_that(p.cargo("run").arg("-v"),
                execs().with_status(0));
}

#[test]
fn deterministic_rustc_dependency_flags() {
    // This bug is non-deterministic hence the large number of dependencies
    // in the hopes it will have a much higher chance of triggering it.

    Package::new("dep1", "0.1.0")
            .file("Cargo.toml", r#"
                [project]
                name = "dep1"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#)
            .file("build.rs", r#"
                fn main() {
                    println!("cargo:rustc-flags=-L native=test1");
                }
            "#)
            .file("src/lib.rs", "")
            .publish();
    Package::new("dep2", "0.1.0")
            .file("Cargo.toml", r#"
                [project]
                name = "dep2"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#)
            .file("build.rs", r#"
                fn main() {
                    println!("cargo:rustc-flags=-L native=test2");
                }
            "#)
            .file("src/lib.rs", "")
            .publish();
    Package::new("dep3", "0.1.0")
            .file("Cargo.toml", r#"
                [project]
                name = "dep3"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#)
            .file("build.rs", r#"
                fn main() {
                    println!("cargo:rustc-flags=-L native=test3");
                }
            "#)
            .file("src/lib.rs", "")
            .publish();
    Package::new("dep4", "0.1.0")
            .file("Cargo.toml", r#"
                [project]
                name = "dep4"
                version = "0.1.0"
                authors = []
                build = "build.rs"
            "#)
            .file("build.rs", r#"
                fn main() {
                    println!("cargo:rustc-flags=-L native=test4");
                }
            "#)
            .file("src/lib.rs", "")
            .publish();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            dep1 = "*"
            dep2 = "*"
            dep3 = "*"
            dep4 = "*"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                    .with_stderr_contains("\
[RUNNING] `rustc --crate-name foo [..] -L native=test1 -L native=test2 \
-L native=test3 -L native=test4`
"));
}
