extern crate cargotest;
extern crate hamcrest;

use std::env;
use std::path::MAIN_SEPARATOR as SEP;

use cargotest::is_nightly;
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
        -C metadata=[..] \
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
fn opt_level_override_0() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.dev]
            opt-level = 0
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -g \
        -C metadata=[..] \
        --out-dir [..] \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
[FINISHED] [..] target(s) in [..]
", sep = SEP,
dir = p.root().display(),
url = p.url()
)));
}

fn check_opt_level_override(profile_level: &str, rustc_level: &str) {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", &format!(r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.dev]
            opt-level = {level}
        "#, level = profile_level))
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level={level} \
        -g \
        -C debug-assertions=on \
        -C metadata=[..] \
        --out-dir [..] \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
[FINISHED] [..] target(s) in [..]
", sep = SEP,
dir = p.root().display(),
url = p.url(),
level = rustc_level
)));
}

#[test]
fn opt_level_overrides() {
    if !is_nightly() { return }

    for &(profile_level, rustc_level) in &[
        ("1", "1"),
        ("2", "2"),
        ("3", "3"),
        ("\"s\"", "s"),
        ("\"z\"", "z"),
    ] {
        check_opt_level_override(profile_level, rustc_level)
    }
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
        -C metadata=[..] \
        --out-dir {dir}{sep}target{sep}release{sep}deps \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}release{sep}deps`
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name test --crate-type lib \
        -C opt-level=1 \
        -g \
        -C metadata=[..] \
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

#[test]
fn profile_in_non_root_manifest_triggers_a_warning() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = []

            [workspace]
            members = ["bar"]

            [profile.dev]
            debug = false
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
            authors = []
            workspace = ".."

            [profile.dev]
            opt-level = 1
        "#)
        .file("bar/src/main.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo_process("build").cwd(p.root().join("bar")).arg("-v"),
                execs().with_status(0).with_stderr("\
[WARNING] profiles for the non root package will be ignored, specify profiles at the workspace root:
package:   [..]
workspace: [..]
[COMPILING] bar v0.1.0 ([..])
[RUNNING] `rustc [..]`
[FINISHED] debug [unoptimized] target(s) in [..]"));
}

#[test]
fn profile_in_virtual_manifest_works() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["bar"]

            [profile.dev]
            opt-level = 1
            debug = false
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
            authors = []
            workspace = ".."
        "#)
        .file("bar/src/main.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo_process("build").cwd(p.root().join("bar")).arg("-v"),
                execs().with_status(0).with_stderr("\
[COMPILING] bar v0.1.0 ([..])
[RUNNING] `rustc [..]`
[FINISHED] debug [optimized] target(s) in [..]"));
}
