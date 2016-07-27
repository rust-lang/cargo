extern crate cargotest;
extern crate hamcrest;

use std::env;
use std::path::MAIN_SEPARATOR as SEP;

use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn profile_overrides() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.dev]
            opt-level = 1
            debug = false
            rpath = true
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level=1 \
        -C debug-assertions=on \
        -C rpath \
        --out-dir [..] \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
[FINISHED] debug [optimized] target(s) in [..]
", sep = SEP,
dir = p.root().display(),
url = p.url(),
)));
}

#[test]
fn top_level_overrides_deps() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.release]
            opt-level = 1
            debug = true

            [dependencies.foo]
            path = "foo"
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]

            name = "foo"
            version = "0.0.0"
            authors = []

            [profile.release]
            opt-level = 0
            debug = false

            [lib]
            name = "foo"
            crate_type = ["dylib", "rlib"]
        "#)
        .file("foo/src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v").arg("--release"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] foo v0.0.0 ({url}/foo)
[RUNNING] `rustc foo{sep}src{sep}lib.rs --crate-name foo \
        --crate-type dylib --crate-type rlib -C prefer-dynamic \
        -C opt-level=1 \
        -g \
        --out-dir {dir}{sep}target{sep}release{sep}deps \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}release{sep}deps`
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level=1 \
        -g \
        --out-dir [..] \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}release{sep}deps \
        --extern foo={dir}{sep}target{sep}release{sep}deps{sep}\
                     {prefix}foo[..]{suffix} \
        --extern foo={dir}{sep}target{sep}release{sep}deps{sep}libfoo.rlib`
[FINISHED] release [optimized + debuginfo] target(s) in [..]
",
                    dir = p.root().display(),
                    url = p.url(),
                    sep = SEP,
                    prefix = env::consts::DLL_PREFIX,
                    suffix = env::consts::DLL_SUFFIX)));
}
