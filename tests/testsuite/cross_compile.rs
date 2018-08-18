use cargo::util::process;
use support::{is_nightly, rustc_host};
use support::{basic_manifest, basic_bin_manifest, cross_compile, execs, project};
use support::hamcrest::{assert_that, existing_file};

#[test]
fn simple_cross() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"
        "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
            fn main() {{
                assert_eq!(std::env::var("TARGET").unwrap(), "{}");
            }}
        "#,
                cross_compile::alternate()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();
    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs(),
    );
    assert_that(&p.target_bin(&target, "foo"), existing_file());

    assert_that(
        process(&p.target_bin(&target, "foo")),
        execs(),
    );
}

#[test]
fn simple_cross_config() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
            [build]
            target = "{}"
        "#,
                cross_compile::alternate()
            ),
        )
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"
        "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
            fn main() {{
                assert_eq!(std::env::var("TARGET").unwrap(), "{}");
            }}
        "#,
                cross_compile::alternate()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();
    assert_that(p.cargo("build -v"), execs());
    assert_that(&p.target_bin(&target, "foo"), existing_file());

    assert_that(
        process(&p.target_bin(&target, "foo")),
        execs(),
    );
}

#[test]
fn simple_deps() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() { bar::bar(); }")
        .build();
    let _p2 = project().at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    let target = cross_compile::alternate();
    assert_that(
        p.cargo("build --target").arg(&target),
        execs(),
    );
    assert_that(&p.target_bin(&target, "foo"), existing_file());

    assert_that(
        process(&p.target_bin(&target, "foo")),
        execs(),
    );
}

#[test]
fn plugin_deps() {
    if cross_compile::disabled() {
        return;
    }
    if !is_nightly() {
        return;
    }

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"

            [dependencies.baz]
            path = "../baz"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #![feature(plugin)]
            #![plugin(bar)]
            extern crate baz;
            fn main() {
                assert_eq!(bar!(), baz::baz());
            }
        "#,
        )
        .build();
    let _bar = project().at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #![feature(plugin_registrar, rustc_private)]

            extern crate rustc_plugin;
            extern crate syntax;

            use rustc_plugin::Registry;
            use syntax::tokenstream::TokenTree;
            use syntax::codemap::Span;
            use syntax::ast::*;
            use syntax::ext::base::{ExtCtxt, MacEager, MacResult};
            use syntax::ext::build::AstBuilder;

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
                reg.register_macro("bar", expand_bar);
            }

            fn expand_bar(cx: &mut ExtCtxt, sp: Span, tts: &[TokenTree])
                          -> Box<MacResult + 'static> {
                MacEager::expr(cx.expr_lit(sp, LitKind::Int(1, LitIntType::Unsuffixed)))
            }
        "#,
        )
        .build();
    let _baz = project().at("baz")
        .file("Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("src/lib.rs", "pub fn baz() -> i32 { 1 }")
        .build();

    let target = cross_compile::alternate();
    assert_that(
        foo.cargo("build --target").arg(&target),
        execs(),
    );
    assert_that(&foo.target_bin(&target, "foo"), existing_file());

    assert_that(
        process(&foo.target_bin(&target, "foo")),
        execs(),
    );
}

#[test]
fn plugin_to_the_max() {
    if cross_compile::disabled() {
        return;
    }
    if !is_nightly() {
        return;
    }

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"

            [dependencies.baz]
            path = "../baz"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #![feature(plugin)]
            #![plugin(bar)]
            extern crate baz;
            fn main() {
                assert_eq!(bar!(), baz::baz());
            }
        "#,
        )
        .build();
    let _bar = project().at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true

            [dependencies.baz]
            path = "../baz"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #![feature(plugin_registrar, rustc_private)]

            extern crate rustc_plugin;
            extern crate syntax;
            extern crate baz;

            use rustc_plugin::Registry;
            use syntax::tokenstream::TokenTree;
            use syntax::codemap::Span;
            use syntax::ast::*;
            use syntax::ext::base::{ExtCtxt, MacEager, MacResult};
            use syntax::ext::build::AstBuilder;
            use syntax::ptr::P;

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
                reg.register_macro("bar", expand_bar);
            }

            fn expand_bar(cx: &mut ExtCtxt, sp: Span, tts: &[TokenTree])
                          -> Box<MacResult + 'static> {
                let bar = Ident::from_str("baz");
                let path = cx.path(sp, vec![bar.clone(), bar]);
                MacEager::expr(cx.expr_call(sp, cx.expr_path(path), vec![]))
            }
        "#,
        )
        .build();
    let _baz = project().at("baz")
        .file("Cargo.toml",  &basic_manifest("baz", "0.0.1"))
        .file("src/lib.rs", "pub fn baz() -> i32 { 1 }")
        .build();

    let target = cross_compile::alternate();
    assert_that(
        foo.cargo("build -v --target").arg(&target),
        execs(),
    );
    println!("second");
    assert_that(
        foo.cargo("build -v --target").arg(&target),
        execs(),
    );
    assert_that(&foo.target_bin(&target, "foo"), existing_file());

    assert_that(
        process(&foo.target_bin(&target, "foo")),
        execs(),
    );
}

#[test]
fn linker_and_ar() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
            [target.{}]
            ar = "my-ar-tool"
            linker = "my-linker-tool"
        "#,
                target
            ),
        )
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            &format!(
                r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs().with_status(101).with_stderr_contains(&format!(
            "\
[COMPILING] foo v0.5.0 ({url})
[RUNNING] `rustc --crate-name foo src/foo.rs --crate-type bin \
    --emit=dep-info,link -C debuginfo=2 \
    -C metadata=[..] \
    --out-dir {dir}/target/{target}/debug/deps \
    --target {target} \
    -C ar=my-ar-tool -C linker=my-linker-tool \
    -L dependency={dir}/target/{target}/debug/deps \
    -L dependency={dir}/target/debug/deps`
",
            dir = p.root().display(),
            url = p.url(),
            target = target,
        )),
    );
}

#[test]
fn plugin_with_extra_dylib_dep() {
    if cross_compile::disabled() {
        return;
    }
    if !is_nightly() {
        return;
    }

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            #![feature(plugin)]
            #![plugin(bar)]

            fn main() {}
        "#,
        )
        .build();
    let _bar = project().at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            plugin = true

            [dependencies.baz]
            path = "../baz"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #![feature(plugin_registrar, rustc_private)]

            extern crate rustc_plugin;
            extern crate baz;

            use rustc_plugin::Registry;

            #[plugin_registrar]
            pub fn foo(reg: &mut Registry) {
                println!("{}", baz::baz());
            }
        "#,
        )
        .build();
    let _baz = project().at("baz")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []

            [lib]
            name = "baz"
            crate_type = ["dylib"]
        "#,
        )
        .file("src/lib.rs", "pub fn baz() -> i32 { 1 }")
        .build();

    let target = cross_compile::alternate();
    assert_that(
        foo.cargo("build --target").arg(&target),
        execs(),
    );
}

#[test]
fn cross_tests() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            authors = []
            version = "0.0.0"

            [[bin]]
            name = "bar"
        "#,
        )
        .file(
            "src/bin/bar.rs",
            &format!(
                r#"
            #[allow(unused_extern_crates)]
            extern crate foo;
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
            #[test] fn test() {{ main() }}
        "#,
                cross_compile::alternate_arch()
            ),
        )
        .file(
            "src/lib.rs",
            &format!(
                r#"
            use std::env;
            pub fn foo() {{ assert_eq!(env::consts::ARCH, "{}"); }}
            #[test] fn test_foo() {{ foo() }}
        "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();
    assert_that(
        p.cargo("test --target").arg(&target),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] foo v0.0.0 ({foo})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/{triple}/debug/deps/foo-[..][EXE]
[RUNNING] target/{triple}/debug/deps/bar-[..][EXE]",
                foo = p.url(),
                triple = target
            ))
            .with_stdout_contains("test test_foo ... ok")
            .with_stdout_contains("test test ... ok"),
    );
}

#[test]
fn no_cross_doctests() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "src/lib.rs",
            r#"
            //! ```
            //! extern crate foo;
            //! assert!(true);
            //! ```
        "#,
        )
        .build();

    let host_output = format!(
        "\
[COMPILING] foo v0.0.1 ({foo})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/debug/deps/foo-[..][EXE]
[DOCTEST] foo
",
        foo = p.url()
    );

    println!("a");
    assert_that(
        p.cargo("test"),
        execs().with_stderr(&host_output),
    );

    println!("b");
    let target = cross_compile::host();
    assert_that(
        p.cargo("test --target").arg(&target),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ({foo})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/{triple}/debug/deps/foo-[..][EXE]
[DOCTEST] foo
",
            foo = p.url(),
            triple = target
        )),
    );

    println!("c");
    let target = cross_compile::alternate();
    assert_that(
        p.cargo("test --target").arg(&target),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.1 ({foo})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/{triple}/debug/deps/foo-[..][EXE]
",
            foo = p.url(),
            triple = target
        )),
    );
}

#[test]
fn simple_cargo_run() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "src/main.rs",
            &format!(
                r#"
            use std::env;
            fn main() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    let target = cross_compile::alternate();
    assert_that(
        p.cargo("run --target").arg(&target),
        execs(),
    );
}

#[test]
fn cross_with_a_build_script() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = 'build.rs'
        "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
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
        "#,
                target
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs().with_stderr(&format!(
            "\
[COMPILING] foo v0.0.0 (file://[..])
[RUNNING] `rustc [..] build.rs [..] --out-dir {dir}/target/debug/build/foo-[..]`
[RUNNING] `{dir}/target/debug/build/foo-[..]/build-script-build`
[RUNNING] `rustc [..] src/main.rs [..] --target {target} [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            target = target,
            dir = p.root().display()
        )),
    );
}

#[test]
fn build_script_needed_for_host_and_target() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = 'build.rs'

            [dependencies.d1]
            path = "d1"
            [build-dependencies.d2]
            path = "d2"
        "#,
        )
        .file(
            "build.rs",
            r#"
            #[allow(unused_extern_crates)]
            extern crate d2;
            fn main() { d2::d2(); }
        "#,
        )
        .file(
            "src/main.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate d1;
            fn main() { d1::d1(); }
        ",
        )
        .file(
            "d1/Cargo.toml",
            r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []
            build = 'build.rs'
        "#,
        )
        .file("d1/src/lib.rs", "pub fn d1() {}")
        .file(
            "d1/build.rs",
            r#"
            use std::env;
            fn main() {
                let target = env::var("TARGET").unwrap();
                println!("cargo:rustc-flags=-L /path/to/{}", target);
            }
        "#,
        )
        .file(
            "d2/Cargo.toml",
            r#"
            [package]
            name = "d2"
            version = "0.0.0"
            authors = []

            [dependencies.d1]
            path = "../d1"
        "#,
        )
        .file(
            "d2/src/lib.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate d1;
            pub fn d2() { d1::d1(); }
        ",
        )
        .build();

    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs()
            .with_stderr_contains(&format!(
                "[COMPILING] d1 v0.0.0 ({url}/d1)",
                url = p.url()
            ))
            .with_stderr_contains(&format!("[RUNNING] `rustc [..] d1/build.rs [..] --out-dir {dir}/target/debug/build/d1-[..]`",
    dir = p.root().display()))
            .with_stderr_contains(&format!(
                "[RUNNING] `{dir}/target/debug/build/d1-[..]/build-script-build`",
                dir = p.root().display()
            ))
            .with_stderr_contains(
                "[RUNNING] `rustc [..] d1/src/lib.rs [..]`",
            )
            .with_stderr_contains(&format!(
                "[COMPILING] d2 v0.0.0 ({url}/d2)",
                url = p.url()
            ))
            .with_stderr_contains(&format!(
                "\
                 [RUNNING] `rustc [..] d2/src/lib.rs [..] \
                 -L /path/to/{host}`",
                host = host
            ))
            .with_stderr_contains(&format!(
                "[COMPILING] foo v0.0.0 ({url})",
                url = p.url()
            ))
            .with_stderr_contains(&format!("\
[RUNNING] `rustc [..] build.rs [..] --out-dir {dir}/target/debug/build/foo-[..] \
           -L /path/to/{host}`", dir = p.root().display(), host = host))
            .with_stderr_contains(&format!(
                "\
                 [RUNNING] `rustc [..] src/main.rs [..] --target {target} [..] \
                 -L /path/to/{target}`",
                target = target
            )),
    );
}

#[test]
fn build_deps_for_the_right_arch() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies.d2]
            path = "d2"
        "#,
        )
        .file("src/main.rs", "extern crate d2; fn main() {}")
        .file("d1/Cargo.toml",  &basic_manifest("d1", "0.0.0"))
        .file("d1/src/lib.rs", "pub fn d1() {}")
        .file(
            "d2/Cargo.toml",
            r#"
            [package]
            name = "d2"
            version = "0.0.0"
            authors = []
            build = "build.rs"

            [build-dependencies.d1]
            path = "../d1"
        "#,
        )
        .file("d2/build.rs", "extern crate d1; fn main() {}")
        .file("d2/src/lib.rs", "")
        .build();

    let target = cross_compile::alternate();
    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs(),
    );
}

#[test]
fn build_script_only_host() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"

            [build-dependencies.d1]
            path = "d1"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "extern crate d1; fn main() {}")
        .file(
            "d1/Cargo.toml",
            r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []
            build = "build.rs"
        "#,
        )
        .file("d1/src/lib.rs", "pub fn d1() {}")
        .file(
            "d1/build.rs",
            r#"
            use std::env;

            fn main() {
                assert!(env::var("OUT_DIR").unwrap().replace("\\", "/")
                                           .contains("target/debug/build/d1-"),
                        "bad: {:?}", env::var("OUT_DIR"));
            }
        "#,
        )
        .build();

    let target = cross_compile::alternate();
    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs(),
    );
}

#[test]
fn plugin_build_script_right_arch() {
    if cross_compile::disabled() {
        return;
    }
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [lib]
            name = "foo"
            plugin = true
        "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v --target")
            .arg(cross_compile::alternate()),
        execs().with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[..]/build-script-build`
[RUNNING] `rustc [..] src/lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn build_script_with_platform_specific_dependencies() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [build-dependencies.d1]
            path = "d1"
        "#,
        )
        .file(
            "build.rs",
            "
            #[allow(unused_extern_crates)]
            extern crate d1;
            fn main() {}
        ",
        )
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            &format!(
                r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []

            [target.{}.dependencies]
            d2 = {{ path = "../d2" }}
        "#,
                host
            ),
        )
        .file("d1/src/lib.rs", "#[allow(unused_extern_crates)] extern crate d2;")
        .file("d2/Cargo.toml",  &basic_manifest("d2", "0.0.0"))
        .file("d2/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs().with_stderr(&format!(
            "\
[COMPILING] d2 v0.0.0 ([..])
[RUNNING] `rustc [..] d2/src/lib.rs [..]`
[COMPILING] d1 v0.0.0 ([..])
[RUNNING] `rustc [..] d1/src/lib.rs [..]`
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `{dir}/target/debug/build/foo-[..]/build-script-build`
[RUNNING] `rustc [..] src/lib.rs [..] --target {target} [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.root().display(),
            target = target
        )),
    );
}

#[test]
fn platform_specific_dependencies_do_not_leak() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [dependencies.d1]
            path = "d1"

            [build-dependencies.d1]
            path = "d1"
        "#,
        )
        .file("build.rs", "extern crate d1; fn main() {}")
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            &format!(
                r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []

            [target.{}.dependencies]
            d2 = {{ path = "../d2" }}
        "#,
                host
            ),
        )
        .file("d1/src/lib.rs", "extern crate d2;")
        .file("d1/Cargo.toml",  &basic_manifest("d1", "0.0.0"))
        .file("d2/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs()
            .with_status(101)
            .with_stderr_contains("[..] can't find crate for `d2`[..]"),
    );
}

#[test]
fn platform_specific_variables_reflected_in_build_scripts() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            build = "build.rs"

            [target.{host}.dependencies]
            d1 = {{ path = "d1" }}

            [target.{target}.dependencies]
            d2 = {{ path = "d2" }}
        "#,
                host = host,
                target = target
            ),
        )
        .file(
            "build.rs",
            &format!(
                r#"
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
        "#,
                host = host,
                target = target
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "d1/Cargo.toml",
            r#"
            [package]
            name = "d1"
            version = "0.0.0"
            authors = []
            links = "d1"
            build = "build.rs"
        "#,
        )
        .file("d1/build.rs", r#"fn main() { println!("cargo:val=1") }"#)
        .file("d1/src/lib.rs", "")
        .file(
            "d2/Cargo.toml",
            r#"
            [package]
            name = "d2"
            version = "0.0.0"
            authors = []
            links = "d2"
            build = "build.rs"
        "#,
        )
        .file("d2/build.rs", r#"fn main() { println!("cargo:val=1") }"#)
        .file("d2/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build -v"), execs());
    assert_that(
        p.cargo("build -v --target").arg(&target),
        execs(),
    );
}

#[test]
fn cross_test_dylib() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
            crate_type = ["dylib"]

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            extern crate bar as the_bar;

            pub fn bar() { the_bar::baz(); }

            #[test]
            fn foo() { bar(); }
        "#,
        )
        .file(
            "tests/test.rs",
            r#"
            extern crate foo as the_foo;

            #[test]
            fn foo() { the_foo::bar(); }
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            crate_type = ["dylib"]
        "#,
        )
        .file(
            "bar/src/lib.rs",
            &format!(
                r#"
             use std::env;
             pub fn baz() {{
                assert_eq!(env::consts::ARCH, "{}");
            }}
        "#,
                cross_compile::alternate_arch()
            ),
        )
        .build();

    assert_that(
        p.cargo("test --target").arg(&target),
        execs()
            .with_stderr(&format!(
                "\
[COMPILING] bar v0.0.1 ({dir}/bar)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target/{arch}/debug/deps/foo-[..][EXE]
[RUNNING] target/{arch}/debug/deps/test-[..][EXE]",
                dir = p.url(),
                arch = cross_compile::alternate()
            ))
            .with_stdout_contains_n("test foo ... ok", 2),
    );
}
