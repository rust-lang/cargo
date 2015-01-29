use std::os;
use std::path;

use support::{project, execs, basic_bin_manifest};
use support::{RUNNING, COMPILING, DOCTEST, cargo_dir};
use hamcrest::{assert_that, existing_file};
use cargo::util::process;
use cargo::ops::rustc_version;

fn setup() {
}

fn disabled() -> bool {
    // First, disable if ./configure requested so
    match os::getenv("CFG_DISABLE_CROSS_TESTS") {
        Some(ref s) if s.as_slice() == "1" => return true,
        _ => {}
    }

    // Right now the windows bots cannot cross compile due to the mingw setup,
    // so we disable ourselves on all but macos/linux setups where the rustc
    // install script ensures we have both architectures
    return !cfg!(target_os = "macos") && !cfg!(target_os = "linux");
}

fn alternate() -> &'static str {
    match os::consts::SYSNAME {
        "linux" => "i686-unknown-linux-gnu",
        "macos" => "i686-apple-darwin",
        _ => unreachable!(),
    }
}

test!(simple_cross {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"
        "#)
        .file("build.rs", format!(r#"
            fn main() {{
                assert_eq!(std::os::getenv("TARGET").unwrap().as_slice(), "{}");
            }}
        "#, alternate()).as_slice())
        .file("src/main.rs", r#"
            use std::os;
            fn main() {
                assert_eq!(os::consts::ARCH, "x86");
            }
        "#);

    let target = alternate();
    assert_that(p.cargo_process("build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&p.target_bin(target, "foo"), existing_file());

    assert_that(
      process(p.target_bin(target, "foo")).unwrap(),
      execs().with_status(0));
});

test!(simple_deps {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() { bar::bar(); }
        "#);
    let p2 = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn bar() {}");
    p2.build();

    let target = alternate();
    assert_that(p.cargo_process("build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&p.target_bin(target, "foo"), existing_file());

    assert_that(
      process(p.target_bin(target, "foo")).unwrap(),
      execs().with_status(0));
});

test!(plugin_deps {
    if disabled() { return }

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"

            [dependencies.baz]
            path = "../baz"
        "#)
        .file("src/main.rs", r#"
            #![feature(plugin)]
            #[plugin] #[no_link]
            extern crate bar;
            extern crate baz;
            fn main() {
                assert_eq!(bar!(), baz::baz());
            }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true
        "#)
        .file("src/lib.rs", r#"
            #![feature(plugin_registrar, quote)]

            extern crate rustc;
            extern crate syntax;

            use rustc::plugin::Registry;
            use syntax::ast::TokenTree;
            use syntax::codemap::Span;
            use syntax::ext::base::{ExtCtxt, MacExpr, MacResult};

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
                reg.register_macro("bar", expand_bar);
            }

            fn expand_bar(cx: &mut ExtCtxt, sp: Span, tts: &[TokenTree])
                          -> Box<MacResult + 'static> {
                MacExpr::new(quote_expr!(cx, 1i))
            }
        "#);
    let baz = project("baz")
        .file("Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn baz() -> int { 1 }");
    bar.build();
    baz.build();

    let target = alternate();
    assert_that(foo.cargo_process("build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&foo.target_bin(target, "foo"), existing_file());

    assert_that(
      process(foo.target_bin(target, "foo")).unwrap(),
      execs().with_status(0));
});

test!(plugin_to_the_max {
    if disabled() { return }

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"

            [dependencies.baz]
            path = "../baz"
        "#)
        .file("src/main.rs", r#"
            #![feature(plugin)]
            #[plugin] #[no_link]
            extern crate bar;
            extern crate baz;
            fn main() {
                assert_eq!(bar!(), baz::baz());
            }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true

            [dependencies.baz]
            path = "../baz"
        "#)
        .file("src/lib.rs", r#"
            #![feature(plugin_registrar, quote)]

            extern crate rustc;
            extern crate syntax;
            extern crate baz;

            use rustc::plugin::Registry;
            use syntax::ast::TokenTree;
            use syntax::codemap::Span;
            use syntax::ext::base::{ExtCtxt, MacExpr, MacResult};

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
                reg.register_macro("bar", expand_bar);
            }

            fn expand_bar(cx: &mut ExtCtxt, sp: Span, tts: &[TokenTree])
                          -> Box<MacResult + 'static> {
                MacExpr::new(quote_expr!(cx, baz::baz()))
            }
        "#);
    let baz = project("baz")
        .file("Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn baz() -> i32 { 1 }");
    bar.build();
    baz.build();

    let target = alternate();
    assert_that(foo.cargo_process("build").arg("--target").arg(target).arg("-v"),
                execs().with_status(0));
    println!("second");
    assert_that(foo.process(cargo_dir().join("cargo")).arg("build").arg("-v")
                   .arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&foo.target_bin(target, "foo"), existing_file());

    assert_that(
      process(foo.target_bin(target, "foo")).unwrap(),
      execs().with_status(0));
});

test!(linker_and_ar {
    if disabled() { return }

    let target = alternate();
    let p = project("foo")
        .file(".cargo/config", format!(r#"
            [target.{}]
            ar = "my-ar-tool"
            linker = "my-linker-tool"
        "#, target).as_slice())
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", r#"
            use std::os;
            fn main() {
                assert_eq!(os::consts::ARCH, "x86");
            }
        "#);

    assert_that(p.cargo_process("build").arg("--target").arg(target)
                                              .arg("-v"),
                execs().with_status(101)
                       .with_stdout(format!("\
{compiling} foo v0.5.0 ({url})
{running} `rustc src/foo.rs --crate-name foo --crate-type bin -g \
    --out-dir {dir}{sep}target{sep}{target} \
    --emit=dep-info,link \
    --target {target} \
    -C ar=my-ar-tool -C linker=my-linker-tool \
    -L dependency={dir}{sep}target{sep}{target} \
    -L dependency={dir}{sep}target{sep}{target}{sep}deps`
",
                            running = RUNNING,
                            compiling = COMPILING,
                            dir = p.root().display(),
                            url = p.url(),
                            target = target,
                            sep = path::SEP,
                            ).as_slice()));
});

test!(plugin_with_extra_dylib_dep {
    if disabled() { return }

    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            #![feature(plugin)]
            #[plugin] #[no_link] extern crate bar;

            fn main() {}
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true

            [dependencies.baz]
            path = "../baz"
        "#)
        .file("src/lib.rs", r#"
            #![feature(plugin_registrar)]

            extern crate rustc;
            extern crate baz;

            use rustc::plugin::Registry;

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
                println!("{}", baz::baz());
            }
        "#);
    let baz = project("baz")
        .file("Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []

            [lib]
            name = "baz"
            crate_type = ["dylib"]
        "#)
        .file("src/lib.rs", "pub fn baz() -> int { 1 }");
    bar.build();
    baz.build();

    let target = alternate();
    assert_that(foo.cargo_process("build").arg("--target").arg(target),
                execs().with_status(0));
});

test!(cross_tests {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            authors = []
            version = "0.0.0"

            [[bin]]
            name = "bar"
        "#)
        .file("src/main.rs", r#"
            extern crate foo;
            use std::os;
            fn main() {
                assert_eq!(os::consts::ARCH, "x86");
            }
            #[test] fn test() { main() }
        "#)
        .file("src/lib.rs", r#"
            use std::os;
            pub fn foo() { assert_eq!(os::consts::ARCH, "x86"); }
            #[test] fn test_foo() { foo() }
        "#);

    let target = alternate();
    assert_that(p.cargo_process("test").arg("--target").arg(target),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 ({foo})
{running} target[..]{triple}[..]bar-[..]

running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{running} target[..]{triple}[..]foo-[..]

running 1 test
test test_foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

{doctest} foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured

", compiling = COMPILING, running = RUNNING, foo = p.url(), triple = target,
   doctest = DOCTEST)));
});

test!(simple_cargo_run {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/main.rs", r#"
            use std::os;
            fn main() {
                assert_eq!(os::consts::ARCH, "x86");
            }
        "#);

    let target = alternate();
    assert_that(p.cargo_process("run").arg("--target").arg(target),
                execs().with_status(0));
});

test!(cross_with_a_build_script {
    if disabled() { return }

    let target = alternate();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = 'build.rs'
        "#)
        .file("build.rs", format!(r#"
            use std::os;
            fn main() {{
                assert_eq!(os::getenv("TARGET").unwrap().as_slice(), "{0}");
                let mut path = Path::new(os::getenv("OUT_DIR").unwrap());
                assert_eq!(path.filename().unwrap(), b"out");
                path.pop();
                assert!(path.filename().unwrap().starts_with(b"foo-"));
                path.pop();
                assert_eq!(path.filename().unwrap(), b"build");
                path.pop();
                assert_eq!(path.filename().unwrap(), b"{0}");
                path.pop();
                assert_eq!(path.filename().unwrap(), b"target");
            }}
        "#, target).as_slice())
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("--target").arg(&target).arg("-v"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 (file://[..])
{running} `rustc build.rs [..] --out-dir {dir}{sep}target{sep}build{sep}foo-[..]`
{running} `{dir}{sep}target{sep}build{sep}foo-[..]build-script-build`
{running} `rustc src{sep}main.rs [..] --target {target} [..]`
", compiling = COMPILING, running = RUNNING, target = target,
   dir = p.root().display(), sep = path::SEP).as_slice()));
});

test!(build_script_needed_for_host_and_target {
    if disabled() { return }

    let target = alternate();
    let (_, host) = rustc_version().unwrap();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = 'build.rs'

            [dependencies.d1]
            path = "d1"
            [build-dependencies.d2]
            path = "d2"
        "#)

        .file("build.rs", r#"
            extern crate d2;
            fn main() { d2::d2(); }
        "#)
        .file("src/main.rs", "
            extern crate d1;
            fn main() { d1::d1(); }
        ")
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []
            build = 'build.rs'
        "#)
        .file("d1/src/lib.rs", "
            pub fn d1() {}
        ")
        .file("d1/build.rs", r#"
            use std::os;
            fn main() {
                let target = os::getenv("TARGET").unwrap();
                println!("cargo:rustc-flags=-L /path/to/{}", target);
            }
        "#)
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.0"
            authors = []

            [dependencies.d1]
            path = "../d1"
        "#)
        .file("d2/src/lib.rs", "
            extern crate d1;
            pub fn d2() { d1::d1(); }
        ");

    assert_that(p.cargo_process("build").arg("--target").arg(&target).arg("-v"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} d1 v0.0.0 (file://{dir})
{running} `rustc d1{sep}build.rs [..] --out-dir {dir}{sep}target{sep}build{sep}d1-[..]`
{running} `{dir}{sep}target{sep}build{sep}d1-[..]build-script-build`
{running} `{dir}{sep}target{sep}build{sep}d1-[..]build-script-build`
{running} `rustc d1{sep}src{sep}lib.rs [..] --target {target} [..] \
           -L /path/to/{target}`
{running} `rustc d1{sep}src{sep}lib.rs [..] \
           -L /path/to/{host}`
{compiling} d2 v0.0.0 (file://{dir})
{running} `rustc d2{sep}src{sep}lib.rs [..] \
           -L /path/to/{host}`
{compiling} foo v0.0.0 (file://{dir})
{running} `rustc build.rs [..] --out-dir {dir}{sep}target{sep}build{sep}foo-[..] \
           -L /path/to/{host}`
{running} `{dir}{sep}target{sep}build{sep}foo-[..]build-script-build`
{running} `rustc src{sep}main.rs [..] --target {target} [..] \
           -L /path/to/{target}`
", compiling = COMPILING, running = RUNNING, target = target, host = host,
   dir = p.root().display(), sep = path::SEP).as_slice()));
});

test!(build_deps_for_the_right_arch {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies.d2]
            path = "d2"
        "#)
        .file("src/main.rs", "extern crate d2; fn main() {}")
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []
        "#)
        .file("d1/src/lib.rs", "
            pub fn d1() {}
        ")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.0"
            authors = []
            build = "build.rs"

            [build-dependencies.d1]
            path = "../d1"
        "#)
        .file("d2/build.rs", "extern crate d1; fn main() {}")
        .file("d2/src/lib.rs", "");

    let target = alternate();
    assert_that(p.cargo_process("build").arg("--target").arg(&target).arg("-v"),
                execs().with_status(0));
});

test!(build_script_only_host {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"

            [build-dependencies.d1]
            path = "d1"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "extern crate d1; fn main() {}")
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []
            build = "build.rs"
        "#)
        .file("d1/src/lib.rs", "
            pub fn d1() {}
        ")
        .file("d1/build.rs", r#"
            use std::os;

            fn main() {
                assert!(os::getenv("OUT_DIR").unwrap()
                                             .contains("target/build/d1-"),
                        "bad: {:?}", os::getenv("OUT_DIR"));
            }
        "#);

    let target = alternate();
    assert_that(p.cargo_process("build").arg("--target").arg(&target).arg("-v"),
                execs().with_status(0));
});
