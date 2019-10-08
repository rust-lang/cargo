use filetime::FileTime;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::SystemTime;

use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::Package;
use cargo_test_support::sleep_ms;
use cargo_test_support::{basic_manifest, is_coarse_mtime, project};

#[cargo_test]
fn modifying_and_moving() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build").with_stdout("").run();
    p.root().move_into_the_past();
    p.root().join("target").move_into_the_past();

    File::create(&p.root().join("src/a.rs"))
        .unwrap()
        .write_all(b"#[allow(unused)]fn main() {}")
        .unwrap();
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    fs::rename(&p.root().join("src/a.rs"), &p.root().join("src/b.rs")).unwrap();
    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[..]file not found[..]")
        .run();
}

#[cargo_test]
fn modify_only_some_files() {
    let p = project()
        .file("src/lib.rs", "mod a;")
        .file("src/a.rs", "")
        .file("src/main.rs", "mod b; fn main() {}")
        .file("src/b.rs", "")
        .file("tests/test.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("test").run();
    sleep_ms(1000);

    assert!(p.bin("foo").is_file());

    let lib = p.root().join("src/lib.rs");
    let bin = p.root().join("src/b.rs");

    File::create(&lib)
        .unwrap()
        .write_all(b"invalid rust code")
        .unwrap();
    File::create(&bin)
        .unwrap()
        .write_all(b"#[allow(unused)]fn foo() {}")
        .unwrap();
    lib.move_into_the_past();

    // Make sure the binary is rebuilt, not the lib
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn rebuild_sub_package_then_while_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"

            [dependencies.a]
            path = "a"
            [dependencies.b]
            path = "b"
        "#,
        )
        .file("src/lib.rs", "extern crate a; extern crate b;")
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            authors = []
            version = "0.0.1"
            [dependencies.b]
            path = "../b"
        "#,
        )
        .file("a/src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    File::create(&p.root().join("b/src/lib.rs"))
        .unwrap()
        .write_all(br#"pub fn b() {}"#)
        .unwrap();

    p.cargo("build -pb").run();

    File::create(&p.root().join("src/lib.rs"))
        .unwrap()
        .write_all(br#"extern crate a; extern crate b; pub fn toplevel() {}"#)
        .unwrap();

    p.cargo("build").run();
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

            [features]
            foo = []
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build --features foo")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    /* Targets should be cached from the first build */

    p.cargo("build")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();

    p.cargo("build").with_stdout("").run();

    p.cargo("build --features foo")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
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

            [profile.dev]
            panic = "abort"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("test")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]debug[..]deps[..]foo-[..][EXE]
[DOCTEST] foo
",
        )
        .run();

    /* Targets should be cached from the first build */

    p.cargo("build")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();

    p.cargo("test foo")
        .with_stderr(
            "\
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]debug[..]deps[..]foo-[..][EXE]
",
        )
        .run();
}

#[cargo_test]
fn changing_bin_paths_common_target_features_caches_targets() {
    // Make sure dep_cache crate is built once per feature
    let p = project()
        .no_manifest()
        .file(
            ".cargo/config",
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
        .with_stdout("ftest off")
        .with_stderr(
            "\
[..]Compiling dep_crate v0.0.1 ([..])
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/a[EXE]`
",
        )
        .run();
    p.cargo("clean -p a").cwd("a").run();
    p.cargo("run")
        .cwd("a")
        .with_stdout("ftest off")
        .with_stderr(
            "\
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/a[EXE]`
",
        )
        .run();

    /* Build and rebuild b/. Ensure dep_crate only builds once */
    p.cargo("run")
        .cwd("b")
        .with_stdout("ftest on")
        .with_stderr(
            "\
[..]Compiling dep_crate v0.0.1 ([..])
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/b[EXE]`
",
        )
        .run();
    p.cargo("clean -p b").cwd("b").run();
    p.cargo("run")
        .cwd("b")
        .with_stdout("ftest on")
        .with_stderr(
            "\
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/b[EXE]`
",
        )
        .run();

    /* Build a/ package again. If we cache different feature dep builds correctly,
     * this should not cause a rebuild of dep_crate */
    p.cargo("clean -p a").cwd("a").run();
    p.cargo("run")
        .cwd("a")
        .with_stdout("ftest off")
        .with_stderr(
            "\
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/a[EXE]`
",
        )
        .run();

    /* Build b/ package again. If we cache different feature dep builds correctly,
     * this should not cause a rebuild */
    p.cargo("clean -p b").cwd("b").run();
    p.cargo("run")
        .cwd("b")
        .with_stdout("ftest on")
        .with_stderr(
            "\
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/b[EXE]`
",
        )
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
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.rename_run("foo", "off1").with_stdout("feature off").run();

    p.cargo("build --features foo")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.rename_run("foo", "on1").with_stdout("feature on").run();

    /* Targets should be cached from the first build */

    let mut e = p.cargo("build");
    // MSVC does not include hash in binary filename, so it gets recompiled.
    if cfg!(target_env = "msvc") {
        e.with_stderr("[COMPILING] foo[..]\n[FINISHED] dev[..]");
    } else {
        e.with_stderr("[FINISHED] dev[..]");
    }
    e.run();
    p.rename_run("foo", "off2").with_stdout("feature off").run();

    let mut e = p.cargo("build --features foo");
    if cfg!(target_env = "msvc") {
        e.with_stderr("[COMPILING] foo[..]\n[FINISHED] dev[..]");
    } else {
        e.with_stderr("[FINISHED] dev[..]");
    }
    e.run();
    p.rename_run("foo", "on2").with_stdout("feature on").run();
}

#[cargo_test]
fn rebuild_tests_if_lib_changes() {
    let p = project()
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "tests/foo.rs",
            r#"
            extern crate foo;
            #[test]
            fn test() { foo::foo(); }
        "#,
        )
        .build();

    p.cargo("build").run();
    p.cargo("test").run();

    sleep_ms(1000);
    File::create(&p.root().join("src/lib.rs")).unwrap();

    p.cargo("build -v").run();
    p.cargo("test -v")
        .with_status(101)
        .with_stderr_contains("[..]cannot find function `foo`[..]")
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
        .with_stderr(
            "\
[COMPILING] c v0.0.1 ([..])
[COMPILING] b v0.0.1 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
",
        )
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
            authors = []
            build = "build.rs"
        "#,
        )
        .file(
            "a/build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-changed=build.rs");
            }
        "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build").with_stdout("").run();
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
            authors = []
            [dependencies]
            d = { path = "../d" }
        "#,
        )
        .file("c/src/lib.rs", "")
        .file("d/Cargo.toml", &basic_manifest("d", "0.0.1"))
        .file("d/src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            target-dir = "./target"
        "#,
        )
        .build();

    p.cargo("build")
        .cwd("a1")
        .with_stderr(&format!(
            "\
[COMPILING] d v0.0.1 ({dir}/d)
[COMPILING] c v0.0.1 ({dir}/c)
[COMPILING] b v0.0.1 ({dir}/b)
[COMPILING] a1 v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url().to_file_path().unwrap().to_str().unwrap()
        ))
        .run();
    p.cargo("build")
        .cwd("a2")
        .with_stderr(
            "\
[COMPILING] a2 v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn no_rebuild_if_build_artifacts_move_backwards_in_time() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    p.root().move_into_the_past();

    p.cargo("build")
        .with_stdout("")
        .with_stderr("[FINISHED] [..]")
        .run();
}

#[cargo_test]
fn rebuild_if_build_artifacts_move_forward_in_time() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    p.root().move_into_the_future();

    p.cargo("build")
        .env("CARGO_LOG", "")
        .with_stdout("")
        .with_stderr(
            "\
[COMPILING] a v0.0.1 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
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
        .with_stdout("old desc")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/foo[EXE]`
",
        )
        .run();

    File::create(&p.root().join("Cargo.toml"))
        .unwrap()
        .write_all(
            br#"
        [package]
        name = "foo"
        description = "new desc"
        version = "0.0.1"
        authors = []
    "#,
        )
        .unwrap();

    p.cargo("run")
        .with_stdout("new desc")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/foo[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn no_rebuild_when_rename_dir() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = { path = "foo" }
        "#,
        )
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.0.1"))
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    let mut new = p.root();
    new.pop();
    new.push("bar");
    fs::rename(p.root(), &new).unwrap();

    p.cargo("build")
        .cwd(&new)
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
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
                authors = []

                [dependencies]
                registry3 = { version = "*", optional = true }
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
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
                authors = []

                [dependencies]
                registry2 = "*"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
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
    p.cargo("build").with_stderr("[FINISHED] [..]\n").run();
    p.cargo("build -p bar")
        .with_stderr("[FINISHED] [..]\n")
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
        .with_stderr(
            "\
[COMPILING] baz v0.1.1 ([..])
[RUNNING] `rustc[..] --test [..]`
[FINISHED] [..]
",
        )
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
        .with_stderr(
            "\
[FRESH] shared [..]
[FRESH] foo [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn changing_rustflags_is_cached() {
    let p = project().file("src/lib.rs", "").build();

    // This isn't ever cached, we always have to recompile
    for _ in 0..2 {
        p.cargo("build")
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
            )
            .run();
        p.cargo("build")
            .env("RUSTFLAGS", "-C linker=cc")
            .with_stderr(
                "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
            )
            .run();
    }
}

#[cargo_test]
fn update_dependency_mtime_does_not_rebuild() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = { path = "bar" }
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build -Z mtime-on-use")
        .masquerade_as_nightly_cargo()
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
    // This does not make new files, but it does update the mtime of the dependency.
    p.cargo("build -p bar -Z mtime-on-use")
        .masquerade_as_nightly_cargo()
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
    // This should not recompile!
    p.cargo("build -Z mtime-on-use")
        .masquerade_as_nightly_cargo()
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
}

fn fingerprint_cleaner(mut dir: PathBuf, timestamp: filetime::FileTime) {
    // Cargo is experimenting with letting outside projects develop some
    // limited forms of GC for target_dir. This is one of the forms.
    // Specifically, Cargo is updating the mtime of a file in
    // target/profile/.fingerprint each time it uses the fingerprint.
    // So a cleaner can remove files associated with a fingerprint
    // if all the files in the fingerprint's folder are older then a time stamp without
    // effecting any builds that happened since that time stamp.
    let mut cleand = false;
    dir.push(".fingerprint");
    for fing in fs::read_dir(&dir).unwrap() {
        let fing = fing.unwrap();

        let outdated = |f: io::Result<fs::DirEntry>| {
            filetime::FileTime::from_last_modification_time(&f.unwrap().metadata().unwrap())
                <= timestamp
        };
        if fs::read_dir(fing.path()).unwrap().all(outdated) {
            fs::remove_dir_all(fing.path()).unwrap();
            println!("remove: {:?}", fing.path());
            // a real cleaner would remove the big files in deps and build as well
            // but fingerprint is sufficient for our tests
            cleand = true;
        } else {
        }
    }
    assert!(
        cleand,
        "called fingerprint_cleaner, but there was nothing to remove"
    );
}

#[cargo_test]
fn fingerprint_cleaner_does_not_rebuild() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = { path = "bar" }

            [features]
            a = []
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build -Z mtime-on-use")
        .masquerade_as_nightly_cargo()
        .run();
    p.cargo("build -Z mtime-on-use --features a")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
    if is_coarse_mtime() {
        sleep_ms(1000);
    }
    let timestamp = filetime::FileTime::from_system_time(SystemTime::now());
    if is_coarse_mtime() {
        sleep_ms(1000);
    }
    // This does not make new files, but it does update the mtime.
    p.cargo("build -Z mtime-on-use --features a")
        .masquerade_as_nightly_cargo()
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
    fingerprint_cleaner(p.target_debug_dir(), timestamp);
    // This should not recompile!
    p.cargo("build -Z mtime-on-use --features a")
        .masquerade_as_nightly_cargo()
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
    // But this should be cleaned and so need a rebuild
    p.cargo("build -Z mtime-on-use")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
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
        .with_stderr(
            "\
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name build_script_build [..]
[RUNNING] [..]build-script-build`
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--test[..]
[FINISHED] [..]
",
        )
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
        .with_stderr_unordered(
            "\
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C debuginfo=2 [..]
[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--crate-type lib --emit=[..]link -C panic=abort -C debuginfo=2 [..]
[COMPILING] somepm [..]
[RUNNING] `rustc --crate-name somepm [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo src/lib.rs [..]-C panic=abort[..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn bust_patched_dep() {
    Package::new("registry1", "0.1.0").publish();
    Package::new("registry2", "0.1.0")
        .dep("registry1", "0.1.0")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            registry2 = "0.1.0"

            [patch.crates-io]
            registry1 = { path = "reg1new" }
        "#,
        )
        .file("src/lib.rs", "")
        .file("reg1new/Cargo.toml", &basic_manifest("registry1", "0.1.0"))
        .file("reg1new/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    File::create(&p.root().join("reg1new/src/lib.rs")).unwrap();
    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] registry1 v0.1.0 ([..])
[COMPILING] registry2 v0.1.0
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();

    p.cargo("build -v")
        .with_stderr(
            "\
[FRESH] registry1 v0.1.0 ([..])
[FRESH] registry2 v0.1.0
[FRESH] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn rebuild_on_mid_build_file_modification() {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["root", "proc_macro_dep"]
        "#,
        )
        .file(
            "root/Cargo.toml",
            r#"
            [package]
            name = "root"
            version = "0.1.0"
            authors = []

            [dependencies]
            proc_macro_dep = { path = "../proc_macro_dep" }
        "#,
        )
        .file(
            "root/src/lib.rs",
            r#"
            #[macro_use]
            extern crate proc_macro_dep;

            #[derive(Noop)]
            pub struct X;
        "#,
        )
        .file(
            "proc_macro_dep/Cargo.toml",
            r#"
            [package]
            name = "proc_macro_dep"
            version = "0.1.0"
            authors = []

            [lib]
            proc-macro = true
        "#,
        )
        .file(
            "proc_macro_dep/src/lib.rs",
            &format!(
                r#"
                extern crate proc_macro;

                use std::io::Read;
                use std::net::TcpStream;
                use proc_macro::TokenStream;

                #[proc_macro_derive(Noop)]
                pub fn noop(_input: TokenStream) -> TokenStream {{
                    let mut stream = TcpStream::connect("{}").unwrap();
                    let mut v = Vec::new();
                    stream.read_to_end(&mut v).unwrap();
                    "".parse().unwrap()
                }}
            "#,
                addr
            ),
        )
        .build();
    let root = p.root();

    let t = thread::spawn(move || {
        let socket = server.accept().unwrap().0;
        sleep_ms(1000);
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(root.join("root/src/lib.rs"))
            .unwrap();
        writeln!(file, "// modified").expect("Failed to append to root sources");
        drop(file);
        drop(socket);
        drop(server.accept().unwrap());
    });

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] proc_macro_dep v0.1.0 ([..]/proc_macro_dep)
[COMPILING] root v0.1.0 ([..]/root)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] root v0.1.0 ([..]/root)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    t.join().ok().unwrap();
}

#[cargo_test]
fn dirty_both_lib_and_test() {
    // This tests that all artifacts that depend on the results of a build
    // script will get rebuilt when the build script reruns, even for separate
    // commands. It does the following:
    //
    // 1. Project "foo" has a build script which will compile a small
    //    staticlib to link against. Normally this would use the `cc` crate,
    //    but here we just use rustc to avoid the `cc` dependency.
    // 2. Build the library.
    // 3. Build the unit test. The staticlib intentionally has a bad value.
    // 4. Rewrite the staticlib with the correct value.
    // 5. Build the library again.
    // 6. Build the unit test. This should recompile.

    let slib = |n| {
        format!(
            r#"
            #[no_mangle]
            pub extern "C" fn doit() -> i32 {{
                return {};
            }}
        "#,
            n
        )
    };

    let p = project()
        .file(
            "src/lib.rs",
            r#"
            extern "C" {
                fn doit() -> i32;
            }

            #[test]
            fn t1() {
                assert_eq!(unsafe { doit() }, 1, "doit assert failure");
            }
        "#,
        )
        .file(
            "build.rs",
            r#"
            use std::env;
            use std::path::PathBuf;
            use std::process::Command;

            fn main() {
                let rustc = env::var_os("RUSTC").unwrap();
                let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
                assert!(
                    Command::new(rustc)
                        .args(&[
                            "--crate-type=staticlib",
                            "--out-dir",
                            out_dir.to_str().unwrap(),
                            "slib.rs"
                        ])
                        .status()
                        .unwrap()
                        .success(),
                    "slib build failed"
                );
                println!("cargo:rustc-link-lib=slib");
                println!("cargo:rustc-link-search={}", out_dir.display());
            }
        "#,
        )
        .file("slib.rs", &slib(2))
        .build();

    p.cargo("build").run();

    // 2 != 1
    p.cargo("test --lib")
        .with_status(101)
        .with_stdout_contains("[..]doit assert failure[..]")
        .run();

    if is_coarse_mtime() {
        // #5918
        sleep_ms(1000);
    }
    // Fix the mistake.
    p.change_file("slib.rs", &slib(1));

    p.cargo("build").run();
    // This should recompile with the new static lib, and the test should pass.
    p.cargo("test --lib").run();
}

#[cargo_test]
fn script_fails_stay_dirty() {
    // Check if a script is aborted (such as hitting Ctrl-C) that it will re-run.
    // Steps:
    // 1. Build to establish fingerprints.
    // 2. Make a change that triggers the build script to re-run. Abort the
    //    script while it is running.
    // 3. Run the build again and make sure it re-runs the script.
    let p = project()
        .file(
            "build.rs",
            r#"
                mod helper;
                fn main() {
                    println!("cargo:rerun-if-changed=build.rs");
                    helper::doit();
                }
            "#,
        )
        .file("helper.rs", "pub fn doit() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();
    if is_coarse_mtime() {
        sleep_ms(1000);
    }
    p.change_file("helper.rs", r#"pub fn doit() {panic!("Crash!");}"#);
    p.cargo("build")
        .with_stderr_contains("[..]Crash![..]")
        .with_status(101)
        .run();
    // There was a bug where this second call would be "fresh".
    p.cargo("build")
        .with_stderr_contains("[..]Crash![..]")
        .with_status(101)
        .run();
}

#[cargo_test]
fn simulated_docker_deps_stay_cached() {
    // Test what happens in docker where the nanoseconds are zeroed out.
    Package::new("regdep", "1.0.0").publish();
    Package::new("regdep_old_style", "1.0.0")
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .publish();
    Package::new("regdep_env", "1.0.0")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-env-changed=SOMEVAR");
            }
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    Package::new("regdep_rerun", "1.0.0")
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-changed=build.rs");
            }
            "#,
        )
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            pathdep = { path = "pathdep" }
            regdep = "1.0"
            regdep_old_style = "1.0"
            regdep_env = "1.0"
            regdep_rerun = "1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate pathdep;
            extern crate regdep;
            extern crate regdep_old_style;
            extern crate regdep_env;
            extern crate regdep_rerun;
            ",
        )
        .file("build.rs", "fn main() {}")
        .file("pathdep/Cargo.toml", &basic_manifest("pathdep", "1.0.0"))
        .file("pathdep/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    let already_zero = {
        // This happens on HFS with 1-second timestamp resolution,
        // or other filesystems where it just so happens to write exactly on a
        // 1-second boundary.
        let metadata = fs::metadata(p.root().join("src/lib.rs")).unwrap();
        let mtime = FileTime::from_last_modification_time(&metadata);
        mtime.nanoseconds() == 0
    };

    // Recursively remove `nanoseconds` from every path.
    fn zeropath(path: &Path) {
        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let metadata = fs::metadata(entry.path()).unwrap();
            let mtime = metadata.modified().unwrap();
            let mtime_duration = mtime.duration_since(SystemTime::UNIX_EPOCH).unwrap();
            let trunc_mtime = FileTime::from_unix_time(mtime_duration.as_secs() as i64, 0);
            let atime = metadata.accessed().unwrap();
            let atime_duration = atime.duration_since(SystemTime::UNIX_EPOCH).unwrap();
            let trunc_atime = FileTime::from_unix_time(atime_duration.as_secs() as i64, 0);
            if let Err(e) = filetime::set_file_times(entry.path(), trunc_atime, trunc_mtime) {
                // Windows doesn't allow changing filetimes on some things
                // (directories, other random things I'm not sure why). Just
                // ignore them.
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    println!("PermissionDenied filetime on {:?}", entry.path());
                } else {
                    panic!("FileTime error on {:?}: {:?}", entry.path(), e);
                }
            }
        }
    }
    zeropath(&p.root());
    zeropath(&paths::home());

    if already_zero {
        println!("already zero");
        // If it was already truncated, then everything stays fresh.
        p.cargo("build -v")
            .with_stderr_unordered(
                "\
[FRESH] pathdep [..]
[FRESH] regdep [..]
[FRESH] regdep_env [..]
[FRESH] regdep_old_style [..]
[FRESH] regdep_rerun [..]
[FRESH] foo [..]
[FINISHED] [..]
",
            )
            .run();
    } else {
        println!("not already zero");
        // It is not ideal that `foo` gets recompiled, but that is the current
        // behavior. Currently mtimes are ignored for registry deps.
        //
        // Note that this behavior is due to the fact that `foo` has a build
        // script in "old" mode where it doesn't print `rerun-if-*`. In this
        // mode we use `Precalculated` to fingerprint a path dependency, where
        // `Precalculated` is an opaque string which has the most recent mtime
        // in it. It differs between builds because one has nsec=0 and the other
        // likely has a nonzero nsec. Hence, the rebuild.
        p.cargo("build -v")
            .with_stderr_unordered(
                "\
[FRESH] pathdep [..]
[FRESH] regdep [..]
[FRESH] regdep_env [..]
[FRESH] regdep_old_style [..]
[FRESH] regdep_rerun [..]
[COMPILING] foo [..]
[RUNNING] [..]/foo-[..]/build-script-build[..]
[RUNNING] `rustc --crate-name foo[..]
[FINISHED] [..]
",
            )
            .run();
    }
}

#[cargo_test]
fn metadata_change_invalidates() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();

    for attr in &[
        "authors = [\"foo\"]",
        "description = \"desc\"",
        "homepage = \"https://example.com\"",
        "repository =\"https://example.com\"",
    ] {
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(p.root().join("Cargo.toml"))
            .unwrap();
        writeln!(file, "{}", attr).unwrap();
        p.cargo("build")
            .with_stderr_contains("[COMPILING] foo [..]")
            .run();
    }
    p.cargo("build -v")
        .with_stderr_contains("[FRESH] foo[..]")
        .run();
    assert_eq!(p.glob("target/debug/deps/libfoo-*.rlib").count(), 1);
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
        .with_stderr_contains("[COMPILING] foo [..]")
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
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();
    p.cargo("build -v")
        .with_stderr_contains("[FRESH] foo[..]")
        .run();
    assert_eq!(p.glob("target/debug/deps/libfoo-*.rlib").count(), 1);
}

#[cargo_test]
fn rename_with_path_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = { path = 'a' }
            "#,
        )
        .file("src/lib.rs", "extern crate a; pub fn foo() { a::foo(); }")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []

                [dependencies]
                b = { path = 'b' }
            "#,
        )
        .file("a/src/lib.rs", "extern crate b; pub fn foo() { b::foo() }")
        .file(
            "a/b/Cargo.toml",
            r#"
                [project]
                name = "b"
                version = "0.5.0"
                authors = []
            "#,
        )
        .file("a/b/src/lib.rs", "pub fn foo() { }");
    let p = p.build();

    p.cargo("build").run();

    // Now rename the root directory and rerun `cargo run`. Not only should we
    // not build anything but we also shouldn't crash.
    let mut new = p.root();
    new.pop();
    new.push("foo2");

    fs::rename(p.root(), &new).unwrap();

    p.cargo("build")
        .cwd(&new)
        .with_stderr("[FINISHED] [..]")
        .run();
}

#[cargo_test]
fn move_target_directory_with_path_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = { path = "a" }
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
            "#,
        )
        .file("src/lib.rs", "extern crate a; pub use a::print_msg;")
        .file(
            "a/build.rs",
            r###"
            use std::env;
            use std::fs;
            use std::path::Path;

            fn main() {
                println!("cargo:rerun-if-changed=build.rs");
                let out_dir = env::var("OUT_DIR").unwrap();
                let dest_path = Path::new(&out_dir).join("hello.rs");
                fs::write(&dest_path, r#"
                    pub fn message() -> &'static str {
                        "Hello, World!"
                    }
                "#).unwrap();
            }
        "###,
        )
        .file(
            "a/src/lib.rs",
            r#"
            include!(concat!(env!("OUT_DIR"), "/hello.rs"));
            pub fn print_msg() { message(); }
            "#,
        );
    let p = p.build();

    let mut parent = p.root();
    parent.pop();

    p.cargo("build").run();

    let new_target = p.root().join("target2");
    fs::rename(p.root().join("target"), &new_target).unwrap();

    p.cargo("build")
        .env("CARGO_TARGET_DIR", &new_target)
        .with_stderr("[FINISHED] [..]")
        .run();
}

#[cargo_test]
fn rerun_if_changes() {
    let p = project()
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rerun-if-env-changed=FOO");
                    if std::env::var("FOO").is_ok() {
                        println!("cargo:rerun-if-env-changed=BAR");
                    }
                }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();

    p.cargo("build -v")
        .env("FOO", "1")
        .with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `[..]build-script-build`
[RUNNING] `rustc [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build")
        .env("FOO", "1")
        .with_stderr("[FINISHED] [..]")
        .run();

    p.cargo("build -v")
        .env("FOO", "1")
        .env("BAR", "1")
        .with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `[..]build-script-build`
[RUNNING] `rustc [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build")
        .env("FOO", "1")
        .env("BAR", "1")
        .with_stderr("[FINISHED] [..]")
        .run();

    p.cargo("build -v")
        .env("BAR", "2")
        .with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `[..]build-script-build`
[RUNNING] `rustc [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build")
        .env("BAR", "2")
        .with_stderr("[FINISHED] [..]")
        .run();
}
