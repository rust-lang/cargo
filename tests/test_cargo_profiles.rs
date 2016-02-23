use std::env;
use std::path::MAIN_SEPARATOR as SEP;

use support::registry::Package;
use support::{COMPILING, RUNNING, UPDATING, DOWNLOADING};
use support::{project, execs};
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
                execs().with_status(0).with_stdout(&format!("\
{compiling} test v0.0.0 ({url})
{running} `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level=1 \
        -C debug-assertions=on \
        -C rpath \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
",
running = RUNNING, compiling = COMPILING, sep = SEP,
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
                execs().with_status(0).with_stdout(&format!("\
{compiling} foo v0.0.0 ({url})
{running} `rustc foo{sep}src{sep}lib.rs --crate-name foo \
        --crate-type dylib --crate-type rlib -C prefer-dynamic \
        -C opt-level=1 \
        -g \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target{sep}release{sep}deps \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}release{sep}deps \
        -L dependency={dir}{sep}target{sep}release{sep}deps`
{compiling} test v0.0.0 ({url})
{running} `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level=1 \
        -g \
        --out-dir {dir}{sep}target{sep}release \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}release \
        -L dependency={dir}{sep}target{sep}release{sep}deps \
        --extern foo={dir}{sep}target{sep}release{sep}deps{sep}\
                     {prefix}foo-[..]{suffix} \
        --extern foo={dir}{sep}target{sep}release{sep}deps{sep}libfoo-[..].rlib`
",
                    running = RUNNING,
                    compiling = COMPILING,
                    dir = p.root().display(),
                    url = p.url(),
                    sep = SEP,
                    prefix = env::consts::DLL_PREFIX,
                    suffix = env::consts::DLL_SUFFIX)));
});


test!(dependencies_profile_in_dev {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            dependencies-profile="release"

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.release]
            opt-level = 3

            [dependencies]
            baz = "*"
        "#)
        .file("src/lib.rs", "extern crate baz;");
    Package::new("baz", "0.0.1").publish();

    assert_that(p.cargo_process("build").arg("-v"),
        execs().with_status(0).with_stdout(&format!("\
{updating} registry [..]
{downloading} baz v0.0.1 ([..])
{compiling} baz v0.0.1 ([..])
{running} `rustc [..]lib.rs --crate-name baz --crate-type lib \
        -C opt-level=3 \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir [..]deps \
        --emit=dep-info,link \
        -L dependency=[..]deps \
        -L dependency=[..]deps \
        --cap-lints allow`
{compiling} test v0.0.0 ([..])
{running} `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -g \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps \
        --extern baz=[..].rlib`",
            updating = UPDATING,
            downloading = DOWNLOADING,
            running = RUNNING,
            compiling = COMPILING,
            sep = SEP,
            dir = p.root().display())
        ));
});


test!(dependencies_profile_in_release {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            dependencies-profile="dev"

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.dev]
            opt-level = 1

            [dependencies]
            baz = "*"
        "#)
        .file("src/lib.rs", "extern crate baz;");
    Package::new("baz", "0.0.1").publish();

    assert_that(p.cargo_process("build").arg("--release").arg("-v"),
        execs().with_status(0).with_stdout(&format!("\
{updating} registry [..]
{downloading} baz v0.0.1 ([..])
{compiling} baz v0.0.1 ([..])
{running} `rustc [..]lib.rs --crate-name baz --crate-type lib \
        -C opt-level=1 \
        -g \
        -C debug-assertions=on \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir [..]deps \
        --emit=dep-info,link \
        -L dependency=[..]deps \
        -L dependency=[..]deps \
        --cap-lints allow`
{compiling} test v0.0.0 ([..])
{running} `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level=3 \
        --out-dir {dir}{sep}target{sep}release \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}release \
        -L dependency={dir}{sep}target{sep}release{sep}deps \
        --extern baz=[..].rlib`",
            updating = UPDATING,
            downloading = DOWNLOADING,
            running = RUNNING,
            compiling = COMPILING,
            sep = SEP,
            dir = p.root().display())
        ));
});
