extern crate cargo;
extern crate cargotest;
extern crate hamcrest;

use std::env;

use cargo::util::process;
use cargotest::{is_nightly, rustc_host};
use cargotest::support::{project, execs, basic_bin_manifest};
use hamcrest::{assert_that, existing_file};

fn disabled() -> bool {
    // First, disable if ./configure requested so
    match env::var("CFG_DISABLE_CROSS_TESTS") {
        Ok(ref s) if *s == "1" => return true,
        _ => {}
    }

    // Right now the windows bots cannot cross compile due to the mingw setup,
    // so we disable ourselves on all but macos/linux setups where the rustc
    // install script ensures we have both architectures
    !(cfg!(target_os = "macos") ||
      cfg!(target_os = "linux") ||
      cfg!(target_env = "msvc"))
}

fn alternate() -> String {
    let platform = match env::consts::OS {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        _ => unreachable!(),
    };
    let arch = match env::consts::ARCH {
        "x86" => "x86_64",
        "x86_64" => "i686",
        _ => unreachable!(),
    };
    format!("{}-{}", arch, platform)
}

fn alternate_arch() -> &'static str {
    match env::consts::ARCH {
        "x86" => "x86_64",
        "x86_64" => "x86",
        _ => unreachable!(),
    }
}

fn host() -> String {
    let platform = match env::consts::OS {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        _ => unreachable!(),
    };
    let arch = match env::consts::ARCH {
        "x86" => "i686",
        "x86_64" => "x86_64",
        _ => unreachable!(),
    };
    format!("{}-{}", arch, platform)
}

#[test]
fn simple_cross() {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"
        "#)
        .file("build.rs", &format!(r#"
            fn main() {{
                assert_eq!(std::env::var("TARGET").unwrap(), "{}");
            }}
        "#, alternate()))
        .file("src/main.rs", &format!(r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#, alternate_arch()));

    let target = alternate();
    assert_that(p.cargo_process("build").arg("--target").arg(&target).arg("-v"),
                execs().with_status(0));
    assert_that(&p.target_bin(&target, "foo"), existing_file());

    assert_that(process(&p.target_bin(&target, "foo")),
                execs().with_status(0));
}

#[test]
fn simple_cross_config() {
    if disabled() { return }

    let p = project("foo")
        .file(".cargo/config", &format!(r#"
            [build]
            target = "{}"
        "#, alternate()))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"
        "#)
        .file("build.rs", &format!(r#"
            fn main() {{
                assert_eq!(std::env::var("TARGET").unwrap(), "{}");
            }}
        "#, alternate()))
        .file("src/main.rs", &format!(r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#, alternate_arch()));

    let target = alternate();
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    assert_that(&p.target_bin(&target, "foo"), existing_file());

    assert_that(process(&p.target_bin(&target, "foo")),
                execs().with_status(0));
}

#[test]
fn simple_deps() {
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
    assert_that(p.cargo_process("build").arg("--target").arg(&target),
                execs().with_status(0));
    assert_that(&p.target_bin(&target, "foo"), existing_file());

    assert_that(process(&p.target_bin(&target, "foo")),
                execs().with_status(0));
}

#[test]
fn plugin_deps() {
    if disabled() { return }
    if !is_nightly() { return }

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
            #![plugin(bar)]
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
            #![feature(plugin_registrar, quote, rustc_private)]

            extern crate rustc_plugin;
            extern crate syntax;

            use rustc_plugin::Registry;
            use syntax::tokenstream::TokenTree;
            use syntax::codemap::Span;
            use syntax::ext::base::{ExtCtxt, MacEager, MacResult};

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
                reg.register_macro("bar", expand_bar);
            }

            fn expand_bar(cx: &mut ExtCtxt, sp: Span, tts: &[TokenTree])
                          -> Box<MacResult + 'static> {
                MacEager::expr(quote_expr!(cx, 1))
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
    assert_that(foo.cargo_process("build").arg("--target").arg(&target),
                execs().with_status(0));
    assert_that(&foo.target_bin(&target, "foo"), existing_file());

    assert_that(process(&foo.target_bin(&target, "foo")),
                execs().with_status(0));
}

#[test]
fn plugin_to_the_max() {
    if disabled() { return }
    if !is_nightly() { return }

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
            #![plugin(bar)]
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
            #![feature(plugin_registrar, quote, rustc_private)]

            extern crate rustc_plugin;
            extern crate syntax;
            extern crate baz;

            use rustc_plugin::Registry;
            use syntax::tokenstream::TokenTree;
            use syntax::codemap::Span;
            use syntax::ext::base::{ExtCtxt, MacEager, MacResult};

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
                reg.register_macro("bar", expand_bar);
            }

            fn expand_bar(cx: &mut ExtCtxt, sp: Span, tts: &[TokenTree])
                          -> Box<MacResult + 'static> {
                MacEager::expr(quote_expr!(cx, baz::baz()))
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
    assert_that(foo.cargo_process("build").arg("--target").arg(&target).arg("-v"),
                execs().with_status(0));
    println!("second");
    assert_that(foo.cargo("build").arg("-v")
                   .arg("--target").arg(&target),
                execs().with_status(0));
    assert_that(&foo.target_bin(&target, "foo"), existing_file());

    assert_that(process(&foo.target_bin(&target, "foo")),
                execs().with_status(0));
}

#[test]
fn linker_and_ar() {
    if disabled() { return }

    let target = alternate();
    let p = project("foo")
        .file(".cargo/config", &format!(r#"
            [target.{}]
            ar = "my-ar-tool"
            linker = "my-linker-tool"
        "#, target))
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &format!(r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#, alternate_arch()));

    assert_that(p.cargo_process("build").arg("--target").arg(&target)
                                              .arg("-v"),
                execs().with_status(101)
                       .with_stderr_contains(&format!("\
[COMPILING] foo v0.5.0 ({url})
[RUNNING] `rustc src[..]foo.rs --crate-name foo --crate-type bin -g \
    -C metadata=[..] \
    --out-dir {dir}[..]target[..]{target}[..]debug \
    --emit=dep-info,link \
    --target {target} \
    -C ar=my-ar-tool -C linker=my-linker-tool \
    -L dependency={dir}[..]target[..]{target}[..]debug[..]deps`
",
                            dir = p.root().display(),
                            url = p.url(),
                            target = target,
                            )));
}

#[test]
fn plugin_with_extra_dylib_dep() {
    if disabled() { return }
    if !is_nightly() { return }

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
            #![plugin(bar)]

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
            #![feature(plugin_registrar, rustc_private)]

            extern crate rustc_plugin;
            extern crate baz;

            use rustc_plugin::Registry;

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
        .file("src/lib.rs", "pub fn baz() -> i32 { 1 }");
    bar.build();
    baz.build();

    let target = alternate();
    assert_that(foo.cargo_process("build").arg("--target").arg(&target),
                execs().with_status(0));
}

#[test]
fn cross_tests() {
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
        .file("src/main.rs", &format!(r#"
            extern crate foo;
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
            #[test] fn test() {{ main() }}
        "#, alternate_arch()))
        .file("src/lib.rs", &format!(r#"
            use std::env;
            pub fn foo() {{ assert_eq!(env::consts::ARCH, "{}"); }}
            #[test] fn test_foo() {{ foo() }}
        "#, alternate_arch()));

    let target = alternate();
    assert_that(p.cargo_process("test").arg("--target").arg(&target),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.0 ({foo})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]{triple}[..]bar-[..]
[RUNNING] target[..]{triple}[..]foo-[..]", foo = p.url(), triple = target))
                       .with_stdout("
running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured


running 1 test
test test_foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

"));
}

#[test]
fn no_cross_doctests() {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            authors = []
            version = "0.0.0"
        "#)
        .file("src/lib.rs", r#"
            //! ```
            //! extern crate foo;
            //! assert!(true);
            //! ```
        "#);

    let host_output = format!("\
[COMPILING] foo v0.0.0 ({foo})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]foo-[..]
[DOCTEST] foo
", foo = p.url());

    println!("a");
    assert_that(p.cargo_process("test"),
                execs().with_status(0)
                       .with_stderr(&host_output));

    println!("b");
    let target = host();
    assert_that(p.cargo_process("test").arg("--target").arg(&target),
                execs().with_status(0)
                       .with_stderr(&host_output));

    println!("c");
    let target = alternate();
    assert_that(p.cargo_process("test").arg("--target").arg(&target),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.0 ({foo})
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]{triple}[..]foo-[..]
", foo = p.url(), triple = target)));
}

#[test]
fn simple_cargo_run() {
    if disabled() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/main.rs", &format!(r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#, alternate_arch()));

    let target = alternate();
    assert_that(p.cargo_process("run").arg("--target").arg(&target),
                execs().with_status(0));
}

#[test]
fn cross_with_a_build_script() {
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
        .file("build.rs", &format!(r#"
            use std::env;
            use std::path::PathBuf;
            fn main() {{
                assert_eq!(env::var("TARGET").unwrap(), "{0}");
                let mut path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
                assert_eq!(path.file_name().unwrap().to_str().unwrap(), "out");
                path.pop();
                assert!(path.file_name().unwrap().to_str().unwrap()
                            .starts_with("foo-"));
                path.pop();
                assert_eq!(path.file_name().unwrap().to_str().unwrap(), "build");
                path.pop();
                assert_eq!(path.file_name().unwrap().to_str().unwrap(), "debug");
                path.pop();
                assert_eq!(path.file_name().unwrap().to_str().unwrap(), "{0}");
                path.pop();
                assert_eq!(path.file_name().unwrap().to_str().unwrap(), "target");
            }}
        "#, target))
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("--target").arg(&target).arg("-v"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.0 (file://[..])
[RUNNING] `rustc build.rs [..] --out-dir {dir}[..]target[..]build[..]foo-[..]`
[RUNNING] `{dir}[..]target[..]build[..]foo-[..]build-script-build`
[RUNNING] `rustc src[..]main.rs [..] --target {target} [..]`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", target = target,
   dir = p.root().display())));
}

#[test]
fn build_script_needed_for_host_and_target() {
    if disabled() { return }

    let target = alternate();
    let host = rustc_host();
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
            use std::env;
            fn main() {
                let target = env::var("TARGET").unwrap();
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
                       .with_stderr_contains(&format!("\
[COMPILING] d1 v0.0.0 ({url}/d1)", url = p.url()))
                       .with_stderr_contains(&format!("\
[RUNNING] `rustc d1[..]build.rs [..] --out-dir {dir}[..]target[..]build[..]d1-[..]`",
    dir = p.root().display()))
                       .with_stderr_contains(&format!("\
[RUNNING] `{dir}[..]target[..]build[..]d1-[..]build-script-build`",
    dir = p.root().display()))
                       .with_stderr_contains("\
[RUNNING] `rustc d1[..]src[..]lib.rs [..]`")
                       .with_stderr_contains(&format!("\
[COMPILING] d2 v0.0.0 ({url}/d2)", url = p.url()))
                       .with_stderr_contains(&format!("\
[RUNNING] `rustc d2[..]src[..]lib.rs [..] \
           -L /path/to/{host}`", host = host))
                       .with_stderr_contains(&format!("\
[COMPILING] foo v0.0.0 ({url})", url = p.url()))
                       .with_stderr_contains(&format!("\
[RUNNING] `rustc build.rs [..] --out-dir {dir}[..]target[..]build[..]foo-[..] \
           -L /path/to/{host}`", dir = p.root().display(), host = host))
                       .with_stderr_contains(&format!("\
[RUNNING] `rustc src[..]main.rs [..] --target {target} [..] \
           -L /path/to/{target}`", target = target)));
}

#[test]
fn build_deps_for_the_right_arch() {
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
}

#[test]
fn build_script_only_host() {
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
            use std::env;

            fn main() {
                assert!(env::var("OUT_DIR").unwrap().replace("\\", "/")
                                           .contains("target/debug/build/d1-"),
                        "bad: {:?}", env::var("OUT_DIR"));
            }
        "#);

    let target = alternate();
    assert_that(p.cargo_process("build").arg("--target").arg(&target).arg("-v"),
                execs().with_status(0));
}

#[test]
fn plugin_build_script_right_arch() {
    if disabled() { return }
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [lib]
            name = "foo"
            plugin = true
        "#)
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v").arg("--target").arg(alternate()),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc build.rs [..]`
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc src[..]lib.rs [..]`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn build_script_with_platform_specific_dependencies() {
    if disabled() { return }

    let target = alternate();
    let host = rustc_host();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [build-dependencies.d1]
            path = "d1"
        "#)
        .file("build.rs", "extern crate d1; fn main() {}")
        .file("src/lib.rs", "")
        .file("d1/Cargo.toml", &format!(r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []

            [target.{}.dependencies]
            d2 = {{ path = "../d2" }}
        "#, host))
        .file("d1/src/lib.rs", "extern crate d2;")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.0"
            authors = []
        "#)
        .file("d2/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v").arg("--target").arg(&target),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] d2 v0.0.0 ([..])
[RUNNING] `rustc d2[..]src[..]lib.rs [..]`
[COMPILING] d1 v0.0.0 ([..])
[RUNNING] `rustc d1[..]src[..]lib.rs [..]`
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc build.rs [..]`
[RUNNING] `{dir}[..]target[..]build[..]foo-[..]build-script-build`
[RUNNING] `rustc src[..]lib.rs [..] --target {target} [..]`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", dir = p.root().display(), target = target)));
}

#[test]
fn platform_specific_dependencies_do_not_leak() {
    if disabled() { return }

    let target = alternate();
    let host = rustc_host();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [dependencies.d1]
            path = "d1"

            [build-dependencies.d1]
            path = "d1"
        "#)
        .file("build.rs", "extern crate d1; fn main() {}")
        .file("src/lib.rs", "")
        .file("d1/Cargo.toml", &format!(r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []

            [target.{}.dependencies]
            d2 = {{ path = "../d2" }}
        "#, host))
        .file("d1/src/lib.rs", "extern crate d2;")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.0"
            authors = []
        "#)
        .file("d2/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v").arg("--target").arg(&target),
                execs().with_status(101)
                       .with_stderr_contains("\
[..] can't find crate for `d2`[..]"));
}

#[test]
fn platform_specific_variables_reflected_in_build_scripts() {
    if disabled() { return }

    let target = alternate();
    let host = rustc_host();
    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [target.{host}.dependencies]
            d1 = {{ path = "d1" }}

            [target.{target}.dependencies]
            d2 = {{ path = "d2" }}
        "#, host = host, target = target))
        .file("build.rs", &format!(r#"
            use std::env;

            fn main() {{
                let platform = env::var("TARGET").unwrap();
                let (expected, not_expected) = match &platform[..] {{
                    "{host}" => ("DEP_D1_VAL", "DEP_D2_VAL"),
                    "{target}" => ("DEP_D2_VAL", "DEP_D1_VAL"),
                    _ => panic!("unknown platform")
                }};

                env::var(expected).ok()
                    .expect(&format!("missing {{}}", expected));
                env::var(not_expected).err()
                    .expect(&format!("found {{}}", not_expected));
            }}
        "#, host = host, target = target))
        .file("src/lib.rs", "")
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []
            links = "d1"
            build = "build.rs"
        "#)
        .file("d1/build.rs", r#"
            fn main() { println!("cargo:val=1") }
        "#)
        .file("d1/src/lib.rs", "")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.0"
            authors = []
            links = "d2"
            build = "build.rs"
        "#)
        .file("d2/build.rs", r#"
            fn main() { println!("cargo:val=1") }
        "#)
        .file("d2/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0));
    assert_that(p.cargo_process("build").arg("-v").arg("--target").arg(&target),
                execs().with_status(0));
}
