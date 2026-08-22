//! Tests for fingerprinting (rebuild detection).
//!
//! This is for general rebuild detection,
//! including config, manifest, and cli inputs.
//!
//! For source change detection, see [`freshness_mtime`] and [`freshness_checksum`].

use std::fs;
use std::io::prelude::*;
use std::net::TcpListener;
use std::process::Stdio;

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, project, rustc_host, rustc_host_env, str};

use super::death;

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
fn checksum_build_compatible_with_mtime_build() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("check")
        .env("CARGO_BUILD_FINGERPRINT", "content")
        .arg("-Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .env("CARGO_BUILD_FINGERPRINT", "content")
        .arg("-Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn changing_lib_features_caches_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"
                edition = "2015"

                [features]
                foo = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build --features foo")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    /* Targets should be cached from the first build */

    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build --features foo")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn changing_profiles_caches_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"
                edition = "2015"

                [profile.dev]
                panic = "abort"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/build/foo/[HASH]/out/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .run();

    /* Targets should be cached from the first build */

    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("test foo")
        .with_stderr_data(str![[r#"
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/build/foo/[HASH]/out/foo-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test]
fn changing_bin_paths_common_target_features_caches_targets() {
    // Make sure dep_cache crate is built once per feature
    let p = project()
        .no_manifest()
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                target-dir = "./target"
            "#,
        )
        .file(
            "dep_crate/Cargo.toml",
            r#"
                [package]
                name    = "dep_crate"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                ftest  = []
            "#,
        )
        .file(
            "dep_crate/src/lib.rs",
            r#"
                #[cfg(feature = "ftest")]
                pub fn yo() {
                    println!("ftest on")
                }
                #[cfg(not(feature = "ftest"))]
                pub fn yo() {
                    println!("ftest off")
                }
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name    = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                dep_crate = {path = "../dep_crate", features = []}
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "a/src/main.rs",
            r#"
                extern crate dep_crate;
                use dep_crate::yo;
                fn main() {
                    yo();
                }
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name    = "b"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                dep_crate = {path = "../dep_crate", features = ["ftest"]}
            "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "b/src/main.rs",
            r#"
                extern crate dep_crate;
                use dep_crate::yo;
                fn main() {
                    yo();
                }
            "#,
        )
        .build();

    /* Build and rebuild a/. Ensure dep_crate only builds once */
    p.cargo("run")
        .cwd("a")
        .with_stdout_data(str![[r#"
ftest off

"#]])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[COMPILING] dep_crate v0.0.1 ([ROOT]/foo/dep_crate)
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/./target/debug/a[EXE]`

"#]])
        .run();
    p.cargo("clean -p a").cwd("a").run();
    p.cargo("run")
        .cwd("a")
        .with_stdout_data(str![[r#"
ftest off

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/./target/debug/a[EXE]`

"#]])
        .run();

    /* Build and rebuild b/. Ensure dep_crate only builds once */
    p.cargo("run")
        .cwd("b")
        .with_stdout_data(str![[r#"
ftest on

"#]])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[COMPILING] dep_crate v0.0.1 ([ROOT]/foo/dep_crate)
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/./target/debug/b[EXE]`

"#]])
        .run();
    p.cargo("clean -p b").cwd("b").run();
    p.cargo("run")
        .cwd("b")
        .with_stdout_data(str![[r#"
ftest on

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/./target/debug/b[EXE]`

"#]])
        .run();

    /* Build a/ package again. If we cache different feature dep builds correctly,
     * this should not cause a rebuild of dep_crate */
    p.cargo("clean -p a").cwd("a").run();
    p.cargo("run")
        .cwd("a")
        .with_stdout_data(str![[r#"
ftest off

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/./target/debug/a[EXE]`

"#]])
        .run();

    /* Build b/ package again. If we cache different feature dep builds correctly,
     * this should not cause a rebuild */
    p.cargo("clean -p b").cwd("b").run();
    p.cargo("run")
        .cwd("b")
        .with_stdout_data(str![[r#"
ftest on

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/./target/debug/b[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn changing_bin_features_caches_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"
                edition = "2015"

                [features]
                foo = []
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let msg = if cfg!(feature = "foo") { "feature on" } else { "feature off" };
                    println!("{}", msg);
                }
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.rename_run("foo", "off1")
        .with_stdout_data(str![[r#"
feature off

"#]])
        .run();

    p.cargo("build --features foo")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.rename_run("foo", "on1")
        .with_stdout_data(str![[r#"
feature on

"#]])
        .run();

    /* Targets should be cached from the first build */
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.rename_run("foo", "off2")
        .with_stdout_data(str![[r#"
feature off

"#]])
        .run();

    p.cargo("build --features foo -v")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.rename_run("foo", "on2")
        .with_stdout_data(str![[r#"
feature on

"#]])
        .run();
}

#[cargo_test]
fn no_rebuild_transitive_target_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "a" }
                [dev-dependencies]
                b = { path = "b" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("tests/foo.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [target.foo.dependencies]
                c = { path = "../c" }
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                c = { path = "../c" }
            "#,
        )
        .file("b/src/lib.rs", "")
        .file("c/Cargo.toml", &basic_manifest("c", "0.0.1"))
        .file("c/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("test --no-run")
        .with_stderr_data(str![[r#"
[COMPILING] c v0.0.1 ([ROOT]/foo/c)
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] unittests src/lib.rs (target/debug/build/foo/[HASH]/out/foo-[HASH][EXE])
[EXECUTABLE] tests/foo.rs (target/debug/build/foo/[HASH]/out/foo-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test]
fn rerun_if_changed_in_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "a/build.rs",
            r#"
                fn main() {
                    println!("cargo::rerun-if-changed=build.rs");
                }
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn same_build_dir_cached_packages() {
    let p = project()
        .no_manifest()
        .file(
            "a1/Cargo.toml",
            r#"
                [package]
                name = "a1"
                version = "0.0.1"
                edition = "2015"
                authors = []
                [dependencies]
                b = { path = "../b" }
            "#,
        )
        .file("a1/src/lib.rs", "")
        .file(
            "a2/Cargo.toml",
            r#"
                [package]
                name = "a2"
                version = "0.0.1"
                edition = "2015"
                authors = []
                [dependencies]
                b = { path = "../b" }
            "#,
        )
        .file("a2/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.0.1"
                edition = "2015"
                authors = []
                [dependencies]
                c = { path = "../c" }
            "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "c/Cargo.toml",
            r#"
                [package]
                name = "c"
                version = "0.0.1"
                edition = "2015"
                authors = []
                [dependencies]
                d = { path = "../d" }
            "#,
        )
        .file("c/src/lib.rs", "")
        .file("d/Cargo.toml", &basic_manifest("d", "0.0.1"))
        .file("d/src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                target-dir = "./target"
            "#,
        )
        .build();

    p.cargo("build")
        .cwd("a1")
        .with_stderr_data(str![[r#"
[LOCKING] 3 packages to highest compatible versions
[COMPILING] d v0.0.1 ([ROOT]/foo/d)
[COMPILING] c v0.0.1 ([ROOT]/foo/c)
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[COMPILING] a1 v0.0.1 ([ROOT]/foo/a1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .cwd("a2")
        .with_stderr_data(str![[r#"
[LOCKING] 3 packages to highest compatible versions
[COMPILING] a2 v0.0.1 ([ROOT]/foo/a2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rebuild_if_environment_changes() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                description = "old desc"
                version = "0.0.1"
                edition = "2015"
                authors = []
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    println!("{}", env!("CARGO_PKG_DESCRIPTION"));
                }
            "#,
        )
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
old desc

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            description = "new desc"
            version = "0.0.1"
            edition = "2015"
            authors = []
        "#,
    );

    p.cargo("run -v")
        .with_stdout_data(str![[r#"
new desc

"#]])
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the environment variable CARGO_PKG_DESCRIPTION changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn unused_optional_dep() {
    Package::new("registry1", "0.1.0").publish();
    Package::new("registry2", "0.1.0").publish();
    Package::new("registry3", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "p"
                authors = []
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }
                baz = { path = "baz" }
                registry1 = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.1"
                edition = "2015"
                authors = []

                [dev-dependencies]
                registry2 = "*"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.1"
                edition = "2015"
                authors = []

                [dependencies]
                registry3 = { version = "*", optional = true }
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn path_dev_dep_registry_updates() {
    Package::new("registry1", "0.1.0").publish();
    Package::new("registry2", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "p"
                authors = []
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.1"
                edition = "2015"
                authors = []

                [dependencies]
                registry1 = "*"

                [dev-dependencies]
                baz = { path = "../baz"}
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.1"
                edition = "2015"
                authors = []

                [dependencies]
                registry2 = "*"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn change_panic_mode() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ['bar', 'baz']
                [profile.dev]
                panic = 'abort'
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.1"))
        .file("bar/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.1"
                edition = "2015"
                authors = []

                [lib]
                proc-macro = true

                [dependencies]
                bar = { path = '../bar' }
            "#,
        )
        .file("baz/src/lib.rs", "extern crate bar;")
        .build();

    p.cargo("build -p bar").run();
    p.cargo("build -p baz").run();
}

#[cargo_test]
fn dont_rebuild_based_on_plugins() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.1"
                edition = "2015"

                [workspace]
                members = ['baz']

                [dependencies]
                proc-macro-thing = { path = 'proc-macro-thing' }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "proc-macro-thing/Cargo.toml",
            r#"
                [package]
                name = "proc-macro-thing"
                version = "0.1.1"
                edition = "2015"

                [lib]
                proc-macro = true

                [dependencies]
                qux = { path = '../qux' }
            "#,
        )
        .file("proc-macro-thing/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.1"
                edition = "2015"

                [dependencies]
                qux = { path = '../qux' }
            "#,
        )
        .file("baz/src/main.rs", "fn main() {}")
        .file("qux/Cargo.toml", &basic_manifest("qux", "0.1.1"))
        .file("qux/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build -p baz").run();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build -p bar")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn reuse_workspace_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.1"
                edition = "2015"

                [workspace]

                [dependencies]
                baz = { path = 'baz' }
            "#,
        )
        .file("src/lib.rs", "")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.1"))
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("test -p baz -v --no-run")
        .with_stderr_data(str![[r#"
[COMPILING] baz v0.1.1 ([ROOT]/foo/baz)
[RUNNING] `rustc --crate-name baz [..]
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/debug/build/baz/[HASH]/out/baz-[HASH][EXE]`

"#]])
        .run();
}

#[cargo_test]
fn reuse_shared_build_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                shared = {path = "shared"}

                [workspace]
                members = ["shared", "bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("shared/Cargo.toml", &basic_manifest("shared", "0.0.1"))
        .file("shared/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"

                [build-dependencies]
                shared = { path = "../shared" }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file("bar/build.rs", "fn main() {}")
        .build();

    p.cargo("build --workspace").run();
    // This should not recompile!
    p.cargo("build -p foo -v")
        .with_stderr_data(str![[r#"
[FRESH] shared v0.0.1 ([ROOT]/foo/shared)
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn changing_rustflags_is_cached() {
    let p = project().file("src/lib.rs", "").build();

    // This isn't ever cached, we always have to recompile
    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build -v")
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build -v")
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn changing_rustc_extra_flags_is_cached() {
    let p = project().file("src/lib.rs", "").build();

    // This isn't ever cached, we always have to recompile
    p.cargo("rustc")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("rustc -v -- -C linker=cc")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("rustc -v")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("rustc -v -- -C linker=cc")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn reuse_panic_build_dep_test() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [build-dependencies]
                bar = { path = "bar" }

                [dev-dependencies]
                bar = { path = "bar" }

                [profile.dev]
                panic = "abort"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    // Check that `bar` is not built twice. It is only needed once (without `panic`).
    p.cargo("test --lib --no-run -v")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar [..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name build_script_build [..]
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/build_script_build`
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]--test[..]
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[EXECUTABLE] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/foo-[HASH][EXE]`

"#]])
        .run();
}

#[cargo_test]
fn reuse_panic_pm() {
    // foo(panic) -> bar(panic)
    // somepm(nopanic) -> bar(nopanic)
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }
                somepm = { path = "somepm" }

                [profile.dev]
                panic = "abort"
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .file(
            "somepm/Cargo.toml",
            r#"
                [package]
                name = "somepm"
                version = "0.0.1"
                edition = "2015"

                [lib]
                proc-macro = true

                [dependencies]
                bar = { path = "../bar" }
            "#,
        )
        .file("somepm/src/lib.rs", "extern crate bar;")
        .build();

    // bar is built once without panic (for proc-macro) and once with (for the
    // normal dependency).
    p.cargo("build -v")
        .with_stderr_data(
            str![[r#"
[LOCKING] 2 packages to highest compatible versions
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar [..] -C panic=abort [..]
[RUNNING] `rustc --crate-name bar [..]
[COMPILING] somepm v0.0.1 ([ROOT]/foo/somepm)
[RUNNING] `rustc --crate-name somepm [..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] -C panic=abort [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn metadata_change_invalidates() {
    // (key, value, value-updated, env-var-name)
    let scenarios = [
        (
            "description",
            r#""foo""#,
            r#""foo_updated""#,
            "CARGO_PKG_DESCRIPTION",
        ),
        (
            "homepage",
            r#""foo""#,
            r#""foo_updated""#,
            "CARGO_PKG_HOMEPAGE",
        ),
        (
            "repository",
            r#""foo""#,
            r#""foo_updated""#,
            "CARGO_PKG_REPOSITORY",
        ),
        (
            "license",
            r#""foo""#,
            r#""foo_updated""#,
            "CARGO_PKG_LICENSE",
        ),
        (
            "license-file",
            r#""foo""#,
            r#""foo_updated""#,
            "CARGO_PKG_LICENSE_FILE",
        ),
        (
            "authors",
            r#"["foo"]"#,
            r#"["foo_updated"]"#,
            "CARGO_PKG_AUTHORS",
        ),
        (
            "rust-version",
            r#""1.0.0""#,
            r#""1.0.1""#,
            "CARGO_PKG_RUST_VERSION",
        ),
        ("readme", r#""foo""#, r#""foo_updated""#, "CARGO_PKG_README"),
    ];
    let base_cargo_toml = r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                "#;

    let p = project().build();
    for (key, value, value_updated, env_var) in scenarios {
        p.change_file("Cargo.toml", base_cargo_toml);
        p.change_file(
            "src/main.rs",
            &format!(
                r#"
            fn main() {{
                let output = env!("{env_var}");
                println!("{{output}}");
            }}
            "#
            ),
        );

        // Compile the first time
        p.cargo("build").run();

        // Update the manifest, rebuild, and verify the build was invalided
        p.change_file("Cargo.toml", &format!("{base_cargo_toml}\n{key} = {value}"));
        p.cargo("build -v")
            .with_stderr_data(format!(
                r#"[DIRTY] foo v0.1.0 ([ROOT]/foo): the environment variable {env_var} changed
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
"#
            ))
            .run();

        // Remove references to the metadata and rebuild
        p.change_file(
            "src/main.rs",
            r#"
            fn main() {
                println!("foo");
            }
            "#,
        );
        p.cargo("build").run();

        // Update the manifest value and verify the build is NOT invalidated.
        p.change_file(
            "Cargo.toml",
            &format!("{base_cargo_toml}\n{key} = {value_updated}"),
        );

        p.cargo("build -v")
            .with_stderr_data(str![[r#"
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
            .run();
    }
}

#[cargo_test]
fn edition_change_invalidates() {
    const MANIFEST: &str = r#"
        [package]
        name = "foo"
        version = "0.1.0"
    "#;
    let p = project()
        .file("Cargo.toml", MANIFEST)
        .file("src/lib.rs", "")
        .build();
    p.cargo("build").run();
    p.change_file("Cargo.toml", &format!("{}edition = \"2018\"", MANIFEST));
    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.change_file(
        "Cargo.toml",
        &format!(
            r#"{}edition = "2018"
            [lib]
            edition = "2015"
            "#,
            MANIFEST
        ),
    );
    p.cargo("build")
        .with_stderr_data(str![[r#"
[WARNING] Cargo.toml: `edition` is set on library `foo` which is deprecated
[WARNING] `foo` (manifest) generated 1 warning
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[WARNING] Cargo.toml: `edition` is set on library `foo` which is deprecated
[WARNING] `foo` (manifest) generated 1 warning
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_eq!(
        p.glob("target/debug/build/foo/*/out/libfoo-*.rlib").count(),
        1
    );
}

#[cargo_test]
fn rerun_if_changes() {
    let p = project()
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rerun-if-env-changed=FOO");
                    if std::env::var("FOO").is_ok() {
                        println!("cargo::rerun-if-env-changed=BAR");
                    }
                }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build -v")
        .env("FOO", "1")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the env variable FOO changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/build_script_build`
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .env("FOO", "1")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build -v")
        .env("FOO", "1")
        .env("BAR", "1")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the env variable BAR changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/build_script_build`
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .env("FOO", "1")
        .env("BAR", "1")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build -v")
        .env("BAR", "2")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the env variable FOO changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/build_script_build`
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .env("BAR", "2")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn channel_shares_filenames() {
    // Test that different "nightly" releases use the same output filename.

    // Create separate rustc binaries to emulate running different toolchains.
    let nightly1 = format!(
        "\
rustc 1.44.0-nightly (38114ff16 2020-03-21)
binary: rustc
commit-hash: 38114ff16e7856f98b2b4be7ab4cd29b38bed59a
commit-date: 2020-03-21
host: {}
release: 1.44.0-nightly
LLVM version: 9.0
",
        rustc_host()
    );

    let nightly2 = format!(
        "\
rustc 1.44.0-nightly (a5b09d354 2020-03-31)
binary: rustc
commit-hash: a5b09d35473615e7142f5570f5c5fad0caf68bd2
commit-date: 2020-03-31
host: {}
release: 1.44.0-nightly
LLVM version: 9.0
",
        rustc_host()
    );

    let beta1 = format!(
        "\
rustc 1.43.0-beta.3 (4c587bbda 2020-03-25)
binary: rustc
commit-hash: 4c587bbda04ab55aaf56feab11dfdfe387a85d7a
commit-date: 2020-03-25
host: {}
release: 1.43.0-beta.3
LLVM version: 9.0
",
        rustc_host()
    );

    let beta2 = format!(
        "\
rustc 1.42.0-beta.5 (4e1c5f0e9 2020-02-28)
binary: rustc
commit-hash: 4e1c5f0e9769a588b91c977e3d81e140209ef3a2
commit-date: 2020-02-28
host: {}
release: 1.42.0-beta.5
LLVM version: 9.0
",
        rustc_host()
    );

    let stable1 = format!(
        "\
rustc 1.42.0 (b8cedc004 2020-03-09)
binary: rustc
commit-hash: b8cedc00407a4c56a3bda1ed605c6fc166655447
commit-date: 2020-03-09
host: {}
release: 1.42.0
LLVM version: 9.0
",
        rustc_host()
    );

    let stable2 = format!(
        "\
rustc 1.41.1 (f3e1a954d 2020-02-24)
binary: rustc
commit-hash: f3e1a954d2ead4e2fc197c7da7d71e6c61bad196
commit-date: 2020-02-24
host: {}
release: 1.41.1
LLVM version: 9.0
",
        rustc_host()
    );

    let compiler = project()
        .at("compiler")
        .file("Cargo.toml", &basic_manifest("compiler", "0.1.0"))
        .file(
            "src/main.rs",
            r#"
            fn main() {
                if std::env::args_os().any(|a| a == "-vV") {
                    print!("{}", env!("FUNKY_VERSION_TEST"));
                    return;
                }
                let mut cmd = std::process::Command::new("rustc");
                cmd.args(std::env::args_os().skip(1));
                assert!(cmd.status().unwrap().success());
            }
            "#,
        )
        .build();

    let makeit = |version, vv| {
        // Force a rebuild.
        compiler.target_debug_dir().join("deps").rm_rf();
        compiler.cargo("build").env("FUNKY_VERSION_TEST", vv).run();
        fs::rename(compiler.bin("compiler"), compiler.bin(version)).unwrap();
    };
    makeit("nightly1", nightly1);
    makeit("nightly2", nightly2);
    makeit("beta1", beta1);
    makeit("beta2", beta2);
    makeit("stable1", stable1);
    makeit("stable2", stable2);

    // Run `cargo check` with different rustc versions to observe its behavior.
    let p = project().file("src/lib.rs", "").build();

    // Runs `cargo check` and returns the rmeta filename created.
    // Checks that the freshness matches the given value.
    let check = |version, fresh| -> String {
        let output = p
            .cargo("check --message-format=json")
            .env("RUSTC", compiler.bin(version))
            .run();
        // Collect the filenames generated.
        let mut artifacts: Vec<_> = std::str::from_utf8(&output.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).unwrap();
                if value["reason"].as_str().unwrap() == "compiler-artifact" {
                    assert_eq!(value["fresh"].as_bool().unwrap(), fresh);
                    let filenames = value["filenames"].as_array().unwrap();
                    assert_eq!(filenames.len(), 1);
                    Some(filenames[0].to_string())
                } else {
                    None
                }
            })
            .collect();
        // Should only generate one rmeta file.
        assert_eq!(artifacts.len(), 1);
        artifacts.pop().unwrap()
    };

    let nightly1_name = check("nightly1", false);
    assert_eq!(check("nightly1", true), nightly1_name);
    assert_eq!(check("nightly2", false), nightly1_name); // same as before
    assert_eq!(check("nightly2", true), nightly1_name);
    // Should rebuild going back to nightly1.
    assert_eq!(check("nightly1", false), nightly1_name);

    let beta1_name = check("beta1", false);
    assert_ne!(beta1_name, nightly1_name);
    assert_eq!(check("beta1", true), beta1_name);
    assert_eq!(check("beta2", false), beta1_name); // same as before
    assert_eq!(check("beta2", true), beta1_name);
    // Should rebuild going back to beta1.
    assert_eq!(check("beta1", false), beta1_name);

    let stable1_name = check("stable1", false);
    assert_ne!(stable1_name, nightly1_name);
    assert_ne!(stable1_name, beta1_name);
    let stable2_name = check("stable2", false);
    assert_ne!(stable1_name, stable2_name);
    // Check everything is fresh.
    assert_eq!(check("stable1", true), stable1_name);
    assert_eq!(check("stable2", true), stable2_name);
    assert_eq!(check("beta1", true), beta1_name);
    assert_eq!(check("nightly1", true), nightly1_name);
}

#[cargo_test]
fn linking_interrupted() {
    // Interrupt during the linking phase shouldn't leave test executable as "fresh".

    // This is used to detect when linking starts, then to pause the linker so
    // that the test can kill cargo.
    let link_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let link_addr = link_listener.local_addr().unwrap();

    // This is used to detect when rustc exits.
    let rustc_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let rustc_addr = rustc_listener.local_addr().unwrap();

    // Create a linker that we can interrupt.
    let linker = project()
        .at("linker")
        .file("Cargo.toml", &basic_manifest("linker", "1.0.0"))
        .file(
            "src/main.rs",
            &r#"
            fn main() {
                // Figure out the output filename.
                let output = match std::env::args().find(|a| a.starts_with("/OUT:")) {
                    Some(s) => s[5..].to_string(),
                    None => {
                        let mut args = std::env::args();
                        loop {
                            if args.next().unwrap() == "-o" {
                                break;
                            }
                        }
                        args.next().unwrap()
                    }
                };
                std::fs::remove_file(&output).unwrap();
                std::fs::write(&output, "").unwrap();
                // Tell the test that we are ready to be interrupted.
                let mut socket = std::net::TcpStream::connect("__ADDR__").unwrap();
                // Wait for the test to kill us.
                std::thread::sleep(std::time::Duration::new(60, 0));
            }
            "#
            .replace("__ADDR__", &link_addr.to_string()),
        )
        .build();
    linker.cargo("build").run();

    // Create a wrapper around rustc that will tell us when rustc is finished.
    let rustc = project()
        .at("rustc-waiter")
        .file("Cargo.toml", &basic_manifest("rustc-waiter", "1.0.0"))
        .file(
            "src/main.rs",
            &r#"
            fn main() {
                let mut conn = None;
                // Check for a normal build (not -vV or --print).
                if std::env::args().any(|arg| arg == "t1") {
                    // Tell the test that rustc has started.
                    conn = Some(std::net::TcpStream::connect("__ADDR__").unwrap());
                }
                let status = std::process::Command::new("rustc")
                    .args(std::env::args().skip(1))
                    .status()
                    .expect("rustc to run");
                std::process::exit(status.code().unwrap_or(1));
            }
            "#
            .replace("__ADDR__", &rustc_addr.to_string()),
        )
        .build();
    rustc.cargo("build").run();

    // Build it once so that the fingerprint gets saved to disk.
    let p = project()
        .file("src/lib.rs", "")
        .file("tests/t1.rs", "")
        .build();
    p.cargo("test --test t1 --no-run").run();

    // Make a change, start a build, then interrupt it.
    p.change_file("src/lib.rs", "// modified");
    let linker_env = format!("CARGO_TARGET_{}_LINKER", rustc_host_env());
    // NOTE: This assumes that the paths to the linker or rustc are not in the
    // fingerprint. But maybe they should be?
    let mut cmd = p
        .cargo("test --test t1 --no-run")
        .env(&linker_env, linker.bin("linker"))
        .env("RUSTC", rustc.bin("rustc-waiter"))
        .build_command();
    let mut child = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("__CARGO_TEST_SETSID_PLEASE_DONT_USE_ELSEWHERE", "1")
        .spawn()
        .unwrap();
    // Wait for rustc to start.
    let mut rustc_conn = rustc_listener.accept().unwrap().0;
    // Wait for linking to start.
    drop(link_listener.accept().unwrap());

    // Interrupt the child.
    death::ctrl_c(&mut child);
    assert!(!child.wait().unwrap().success());
    // Wait for rustc to exit. If we don't wait, then the command below could
    // start while rustc is still being torn down.
    let mut buf = [0];
    drop(rustc_conn.read_exact(&mut buf));

    // Build again, shouldn't be fresh.
    p.cargo("test --test t1 -v")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the config settings changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[RUNNING] `rustc --crate-name t1 [..]
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/t1-[HASH][EXE]`

"#]])
        .run();
}

#[cargo_test]
#[cfg_attr(
    not(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc")),
    ignore
)]
fn lld_is_fresh() {
    // Check for bug when using lld linker that it remains fresh with dylib.
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [target.x86_64-pc-windows-msvc]
                linker = "rust-lld"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lib]
                crate-type = ["dylib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn env_in_code_causes_rebuild() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    println!("{:?}", option_env!("FOO"));
                    println!("{:?}", option_env!("FOO\nBAR"));
                }
            "#,
        )
        .build();

    p.cargo("build").env_remove("FOO").run();
    p.cargo("build")
        .env_remove("FOO")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build -v")
        .env("FOO", "bar")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.1.0 ([ROOT]/foo): the environment variable FOO changed
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .env("FOO", "bar")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build -v")
        .env("FOO", "baz")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.1.0 ([ROOT]/foo): the environment variable FOO changed
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .env("FOO", "baz")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build -v")
        .env_remove("FOO")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.1.0 ([ROOT]/foo): the environment variable FOO changed
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .env_remove("FOO")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let interesting = " #!$\nabc\r\\\t\u{8}\r\n";
    p.cargo("build").env("FOO", interesting).run();
    p.cargo("build")
        .env("FOO", interesting)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build").env("FOO\nBAR", interesting).run();
    p.cargo("build")
        .env("FOO\nBAR", interesting)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn env_build_script_no_rebuild() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo::rustc-env=FOO=bar");
                }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    println!("{:?}", env!("FOO"));
                }
            "#,
        )
        .build();

    p.cargo("build").run();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cargo_env_changes() {
    // Checks that changes to the env var CARGO in the dep-info file triggers
    // a rebuild.
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    println!("{:?}", env!("CARGO"));
                }
            "#,
        )
        .build();

    let cargo_exe = crate::utils::cargo_exe();
    let other_cargo_path = p.root().join(cargo_exe.file_name().unwrap());
    std::fs::hard_link(&cargo_exe, &other_cargo_path).unwrap();
    let other_cargo = || {
        let mut pb = cargo_test_support::process(&other_cargo_path);
        pb.cwd(p.root());
        cargo_test_support::execs().with_process_builder(pb)
    };

    p.cargo("check").run();
    other_cargo()
        .arg("check")
        .arg("-v")
        .with_stderr_data(str![[r#"
[DIRTY] foo v1.0.0 ([ROOT]/foo): the environment variable CARGO changed
[CHECKING] foo v1.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // And just to confirm that without using env! it doesn't rebuild.
    p.change_file("src/main.rs", "fn main() {}");
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v1.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    other_cargo()
        .arg("check")
        .arg("-v")
        .with_stderr_data(str![[r#"
[FRESH] foo v1.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn changing_linker() {
    // Changing linker should rebuild.
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("build").run();
    let linker_env = format!("CARGO_TARGET_{}_LINKER", rustc_host_env());
    p.cargo("build --verbose")
        .env(&linker_env, "nonexistent-linker")
        .with_status(101)
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the config settings changed
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] -C linker=nonexistent-linker[..]`
[ERROR] linker `nonexistent-linker` not found
...
"#]])
        .run();
}
