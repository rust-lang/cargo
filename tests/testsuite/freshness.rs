use std::fs::{self, File};
use std::io::prelude::*;

use crate::support::paths::CargoPathExt;
use crate::support::registry::Package;
use crate::support::sleep_ms;
use crate::support::{basic_manifest, project};

#[test]
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
        ).run();

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
        ).run();

    fs::rename(&p.root().join("src/a.rs"), &p.root().join("src/b.rs")).unwrap();
    p.cargo("build").with_status(101).run();
}

#[test]
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
        ).run();
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
        ).run();
    assert!(p.bin("foo").is_file());
}

#[test]
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
        ).file("src/lib.rs", "extern crate a; extern crate b;")
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
        ).file("a/src/lib.rs", "extern crate b;")
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

#[test]
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
        ).file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();

    p.cargo("build --features foo")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();

    /* Targets should be cached from the first build */

    p.cargo("build")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();

    p.cargo("build").with_stdout("").run();

    p.cargo("build --features foo")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
}

#[test]
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
        ).file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();

    p.cargo("test")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]debug[..]deps[..]foo-[..][EXE]
[DOCTEST] foo
",
        ).run();

    /* Targets should be cached from the first build */

    p.cargo("build")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();

    p.cargo("test foo")
        .with_stderr(
            "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]debug[..]deps[..]foo-[..][EXE]
[DOCTEST] foo
",
        ).run();
}

#[test]
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
        ).file(
            "dep_crate/Cargo.toml",
            r#"
            [package]
            name    = "dep_crate"
            version = "0.0.1"
            authors = []

            [features]
            ftest  = []
        "#,
        ).file(
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
        ).file(
            "a/Cargo.toml",
            r#"
            [package]
            name    = "a"
            version = "0.0.1"
            authors = []

            [dependencies]
            dep_crate = {path = "../dep_crate", features = []}
        "#,
        ).file("a/src/lib.rs", "")
        .file(
            "a/src/main.rs",
            r#"
            extern crate dep_crate;
            use dep_crate::yo;
            fn main() {
                yo();
            }
        "#,
        ).file(
            "b/Cargo.toml",
            r#"
            [package]
            name    = "b"
            version = "0.0.1"
            authors = []

            [dependencies]
            dep_crate = {path = "../dep_crate", features = ["ftest"]}
        "#,
        ).file("b/src/lib.rs", "")
        .file(
            "b/src/main.rs",
            r#"
            extern crate dep_crate;
            use dep_crate::yo;
            fn main() {
                yo();
            }
        "#,
        ).build();

    /* Build and rebuild a/. Ensure dep_crate only builds once */
    p.cargo("run")
        .cwd(p.root().join("a"))
        .with_stdout("ftest off")
        .with_stderr(
            "\
[..]Compiling dep_crate v0.0.1 ([..])
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/a[EXE]`
",
        ).run();
    p.cargo("clean -p a").cwd(p.root().join("a")).run();
    p.cargo("run")
        .cwd(p.root().join("a"))
        .with_stdout("ftest off")
        .with_stderr(
            "\
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/a[EXE]`
",
        ).run();

    /* Build and rebuild b/. Ensure dep_crate only builds once */
    p.cargo("run")
        .cwd(p.root().join("b"))
        .with_stdout("ftest on")
        .with_stderr(
            "\
[..]Compiling dep_crate v0.0.1 ([..])
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/b[EXE]`
",
        ).run();
    p.cargo("clean -p b").cwd(p.root().join("b")).run();
    p.cargo("run")
        .cwd(p.root().join("b"))
        .with_stdout("ftest on")
        .with_stderr(
            "\
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/b[EXE]`
",
        ).run();

    /* Build a/ package again. If we cache different feature dep builds correctly,
     * this should not cause a rebuild of dep_crate */
    p.cargo("clean -p a").cwd(p.root().join("a")).run();
    p.cargo("run")
        .cwd(p.root().join("a"))
        .with_stdout("ftest off")
        .with_stderr(
            "\
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/a[EXE]`
",
        ).run();

    /* Build b/ package again. If we cache different feature dep builds correctly,
     * this should not cause a rebuild */
    p.cargo("clean -p b").cwd(p.root().join("b")).run();
    p.cargo("run")
        .cwd(p.root().join("b"))
        .with_stdout("ftest on")
        .with_stderr(
            "\
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target/debug/b[EXE]`
",
        ).run();
}

#[test]
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
        ).file(
            "src/main.rs",
            r#"
            fn main() {
                let msg = if cfg!(feature = "foo") { "feature on" } else { "feature off" };
                println!("{}", msg);
            }
        "#,
        ).build();

    // Windows has a problem with replacing a binary that was just executed.
    // Unlinking it will succeed, but then attempting to immediately replace
    // it will sometimes fail with "Already Exists".
    // See https://github.com/rust-lang/cargo/issues/5481
    let foo_proc = |name: &str| {
        let src = p.bin("foo");
        let dst = p.bin(name);
        fs::rename(&src, &dst).expect("Failed to rename foo");
        p.process(dst)
    };

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
    foo_proc("off1").with_stdout("feature off").run();

    p.cargo("build --features foo")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
    foo_proc("on1").with_stdout("feature on").run();

    /* Targets should be cached from the first build */

    p.cargo("build")
        .with_stderr(
            "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
    foo_proc("off2").with_stdout("feature off").run();

    p.cargo("build --features foo")
        .with_stderr(
            "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
    foo_proc("on2").with_stdout("feature on").run();
}

#[test]
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
        ).build();

    p.cargo("build").run();
    p.cargo("test").run();

    sleep_ms(1000);
    File::create(&p.root().join("src/lib.rs")).unwrap();

    p.cargo("build -v").run();
    p.cargo("test -v").with_status(101).run();
}

#[test]
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
        ).file("src/lib.rs", "")
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
        ).file("a/src/lib.rs", "")
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
        ).file("b/src/lib.rs", "")
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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
}

#[test]
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
        ).file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
            build = "build.rs"
        "#,
        ).file(
            "a/build.rs",
            r#"
            fn main() {
                println!("cargo:rerun-if-changed=build.rs");
            }
        "#,
        ).file("a/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build").with_stdout("").run();
}

#[test]
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
        ).file("a1/src/lib.rs", "")
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
        ).file("a2/src/lib.rs", "")
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
        ).file("b/src/lib.rs", "")
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
        ).file("c/src/lib.rs", "")
        .file("d/Cargo.toml", &basic_manifest("d", "0.0.1"))
        .file("d/src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            target-dir = "./target"
        "#,
        ).build();

    p.cargo("build")
        .cwd(p.root().join("a1"))
        .with_stderr(&format!(
            "\
[COMPILING] d v0.0.1 ({dir}/d)
[COMPILING] c v0.0.1 ({dir}/c)
[COMPILING] b v0.0.1 ({dir}/b)
[COMPILING] a1 v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url().to_file_path().unwrap().to_str().unwrap()
        )).run();
    p.cargo("build")
        .cwd(p.root().join("a2"))
        .with_stderr(
            "\
[COMPILING] a2 v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
}

#[test]
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
        ).file("src/lib.rs", "")
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

#[test]
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
        ).file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    p.root().move_into_the_future();

    p.cargo("build")
        .env("RUST_LOG", "")
        .with_stdout("")
        .with_stderr(
            "\
[COMPILING] a v0.0.1 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        ).run();
}

#[test]
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
        ).file(
            "src/main.rs",
            r#"
            fn main() {
                println!("{}", env!("CARGO_PKG_DESCRIPTION"));
            }
        "#,
        ).build();

    p.cargo("run")
        .with_stdout("old desc")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/foo[EXE]`
",
        ).run();

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
        ).unwrap();

    p.cargo("run")
        .with_stdout("new desc")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/foo[EXE]`
",
        ).run();
}

#[test]
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
        ).file("src/lib.rs", "")
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

#[test]
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
        ).file("src/lib.rs", "")
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
        ).file("bar/src/lib.rs", "")
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
        ).file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[test]
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
        ).file("src/lib.rs", "")
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
        ).file("bar/src/lib.rs", "")
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
        ).file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[test]
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
        ).file("src/lib.rs", "")
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
        ).file("baz/src/lib.rs", "extern crate bar;")
        .build();

    p.cargo("build -p bar").run();
    p.cargo("build -p baz").run();
}

#[test]
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
        ).file("src/lib.rs", "")
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
        ).file("proc-macro-thing/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.1"

                [dependencies]
                qux = { path = '../qux' }
            "#,
        ).file("baz/src/main.rs", "fn main() {}")
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

#[test]
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
        ).file("src/lib.rs", "")
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
        ).run();
}

#[test]
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

    p.cargo("build --all").run();
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

#[test]
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

#[test]
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
[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--crate-type lib --emit=dep-info,link -C debuginfo=2 [..]
[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--crate-type lib --emit=dep-info,link -C panic=abort -C debuginfo=2 [..]
[COMPILING] somepm [..]
[RUNNING] `rustc --crate-name somepm [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo src/lib.rs [..]-C panic=abort[..]
[FINISHED] [..]
",
        )
        .run();
}
