use std::os;
use std::path;

use support::{project, execs};
use support::{COMPILING, RUNNING};
use hamcrest::assert_that;

fn setup() {
}

test!(profile_overrides {
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
                execs().with_status(0).with_stdout(format!("\
{compiling} test v0.0.0 ({url})
{running} `rustc {dir}{sep}src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level=1 \
        --cfg ndebug \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        -C rpath \
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

test!(top_level_overrides_deps {
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
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
{running} `rustc {dir}{sep}foo{sep}src{sep}lib.rs --crate-name foo \
        --crate-type dylib --crate-type rlib -C prefer-dynamic \
        -C opt-level=1 \
        -g \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target{sep}release{sep}deps \
        --emit=dep-info,link \
        -L {dir}{sep}target{sep}release{sep}deps \
        -L {dir}{sep}target{sep}release{sep}deps`
{compiling} test v0.0.0 ({url})
{running} `rustc {dir}{sep}src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level=1 \
        -g \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target{sep}release \
        --emit=dep-info,link \
        -L {dir}{sep}target{sep}release \
        -L {dir}{sep}target{sep}release{sep}deps \
        --extern foo={dir}{sep}target{sep}release{sep}deps/\
                     {prefix}foo-[..]{suffix} \
        --extern foo={dir}{sep}target{sep}release{sep}deps/libfoo-[..].rlib`
",
                    running = RUNNING,
                    compiling = COMPILING,
                    dir = p.root().display(),
                    url = p.url(),
                    sep = path::SEP,
                    prefix = os::consts::DLL_PREFIX,
                    suffix = os::consts::DLL_SUFFIX).as_slice()));
});
