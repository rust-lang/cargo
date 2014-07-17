// Currently the only cross compilers available via nightlies are on linux/osx,
// so we can only run these tests on those platforms
#![cfg(target_os = "linux")]
#![cfg(target_os = "macos")]

use std::os;
use std::path;

use support::{project, execs, basic_bin_manifest};
use support::{RUNNING, COMPILING};
use hamcrest::{assert_that, existing_file};
use cargo::util::process;

fn setup() {
}

fn alternate() -> &'static str {
    match os::consts::SYSNAME {
        "linux" => "i686-unknown-linux-gnu",
        "macos" => "i686-apple-darwin",
        _ => unreachable!(),
    }
}

test!(simple_cross {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", r#"
            use std::os;
            fn main() {
                assert_eq!(os::consts::ARCH, "x86");
            }
        "#);

    let target = alternate();
    assert_that(p.cargo_process("cargo-build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&p.target_bin(target, "foo"), existing_file());

    assert_that(
      process(p.target_bin(target, "foo")),
      execs().with_status(0));
})

test!(simple_deps {
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
    assert_that(p.cargo_process("cargo-build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&p.target_bin(target, "foo"), existing_file());

    assert_that(
      process(p.target_bin(target, "foo")),
      execs().with_status(0));
})

test!(plugin_deps {
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
            #![feature(phase)]
            #[phase(plugin)]
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

            [[lib]]
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
                          -> Box<MacResult> {
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
    assert_that(foo.cargo_process("cargo-build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&foo.target_bin(target, "foo"), existing_file());

    assert_that(
      process(foo.target_bin(target, "foo")),
      execs().with_status(0));
})

test!(plugin_to_the_max {
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
            #![feature(phase)]
            #[phase(plugin)]
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

            [[lib]]
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
                          -> Box<MacResult> {
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
        .file("src/lib.rs", "pub fn baz() -> int { 1 }");
    bar.build();
    baz.build();

    let target = alternate();
    assert_that(foo.cargo_process("cargo-build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&foo.target_bin(target, "foo"), existing_file());

    assert_that(
      process(foo.target_bin(target, "foo")),
      execs().with_status(0));
})

test!(linker_and_ar {
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

    assert_that(p.cargo_process("cargo-build").arg("--target").arg(target)
                                              .arg("-v"),
                execs().with_status(101)
                       .with_stdout(format!("\
{running} `rustc src/foo.rs --crate-name foo --crate-type bin \
    --out-dir {dir}{sep}target{sep}{target} \
    --target {target} \
    -C ar=my-ar-tool -C linker=my-linker-tool \
    -L {dir}{sep}target{sep}{target} \
    -L {dir}{sep}target{sep}{target}{sep}deps`
{compiling} foo v0.5.0 (file:{dir})
",
                            running = RUNNING,
                            compiling = COMPILING,
                            dir = p.root().display(),
                            target = target,
                            sep = path::SEP,
                            ).as_slice()));
})
