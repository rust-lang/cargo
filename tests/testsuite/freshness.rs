use std::fs::{self, File};
use std::io::prelude::*;

use cargotest::sleep_ms;
use cargotest::support::paths::CargoPathExt;
use cargotest::support::registry::Package;
use cargotest::support::{execs, path2url, project};
use hamcrest::{assert_that, existing_file};

#[test]
fn modifying_and_moving() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            mod a; fn main() {}
        "#,
        )
        .file("src/a.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = path2url(p.root())
        )),
    );

    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
    p.root().move_into_the_past();
    p.root().join("target").move_into_the_past();

    File::create(&p.root().join("src/a.rs"))
        .unwrap()
        .write_all(b"#[allow(unused)]fn main() {}")
        .unwrap();
    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = path2url(p.root())
        )),
    );

    fs::rename(&p.root().join("src/a.rs"), &p.root().join("src/b.rs")).unwrap();
    assert_that(p.cargo("build"), execs().with_status(101));
}

#[test]
fn modify_only_some_files() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#,
        )
        .file("src/lib.rs", "mod a;")
        .file("src/a.rs", "")
        .file(
            "src/main.rs",
            r#"
            mod b;
            fn main() {}
        "#,
        )
        .file("src/b.rs", "")
        .file("tests/test.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = path2url(p.root())
        )),
    );
    assert_that(p.cargo("test"), execs().with_status(0));
    sleep_ms(1000);

    assert_that(&p.bin("foo"), existing_file());

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
    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(format!(
            "\
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = path2url(p.root())
        )),
    );
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn rebuild_sub_package_then_while_package() {
    let p = project("foo")
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
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            authors = []
            version = "0.0.1"
        "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));

    File::create(&p.root().join("b/src/lib.rs"))
        .unwrap()
        .write_all(
            br#"
        pub fn b() {}
    "#,
        )
        .unwrap();

    assert_that(p.cargo("build").arg("-pb"), execs().with_status(0));

    File::create(&p.root().join("src/lib.rs"))
        .unwrap()
        .write_all(
            br#"
        extern crate a;
        extern crate b;
        pub fn toplevel() {}
    "#,
        )
        .unwrap();

    assert_that(p.cargo("build"), execs().with_status(0));
}

#[test]
fn changing_lib_features_caches_targets() {
    let p = project("foo")
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

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );

    assert_that(
        p.cargo("build").arg("--features").arg("foo"),
        execs().with_status(0).with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );

    /* Targets should be cached from the first build */

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(0)
            .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"),
    );

    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));

    assert_that(
        p.cargo("build").arg("--features").arg("foo"),
        execs()
            .with_status(0)
            .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"),
    );
}

#[test]
fn changing_profiles_caches_targets() {
    let p = project("foo")
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

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );

    assert_that(
        p.cargo("test"),
        execs().with_status(0).with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]debug[..]deps[..]foo-[..][EXE]
[DOCTEST] foo
",
        ),
    );

    /* Targets should be cached from the first build */

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(0)
            .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"),
    );

    assert_that(
        p.cargo("test").arg("foo"),
        execs().with_status(0).with_stderr(
            "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[..]debug[..]deps[..]foo-[..][EXE]
[DOCTEST] foo
",
        ),
    );
}

#[test]
fn changing_bin_paths_common_target_features_caches_targets() {
    // Make sure dep_cache crate is built once per feature
    let p = project("foo")
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
    assert_that(
        p.cargo("run").cwd(p.root().join("a")),
        execs().with_status(0).with_stdout("ftest off").with_stderr(
            "\
[..]Compiling dep_crate v0.0.1 ([..])
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target[/]debug[/]a[EXE]`
",
        ),
    );
    assert_that(
        p.cargo("clean").arg("-p").arg("a").cwd(p.root().join("a")),
        execs().with_status(0),
    );
    assert_that(
        p.cargo("run").cwd(p.root().join("a")),
        execs().with_status(0).with_stdout("ftest off").with_stderr(
            "\
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target[/]debug[/]a[EXE]`
",
        ),
    );

    /* Build and rebuild b/. Ensure dep_crate only builds once */
    assert_that(
        p.cargo("run").cwd(p.root().join("b")),
        execs().with_status(0).with_stdout("ftest on").with_stderr(
            "\
[..]Compiling dep_crate v0.0.1 ([..])
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target[/]debug[/]b[EXE]`
",
        ),
    );
    assert_that(
        p.cargo("clean").arg("-p").arg("b").cwd(p.root().join("b")),
        execs().with_status(0),
    );
    assert_that(
        p.cargo("run").cwd(p.root().join("b")),
        execs().with_status(0).with_stdout("ftest on").with_stderr(
            "\
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target[/]debug[/]b[EXE]`
",
        ),
    );

    /* Build a/ package again. If we cache different feature dep builds correctly,
     * this should not cause a rebuild of dep_crate */
    assert_that(
        p.cargo("clean").arg("-p").arg("a").cwd(p.root().join("a")),
        execs().with_status(0),
    );
    assert_that(
        p.cargo("run").cwd(p.root().join("a")),
        execs().with_status(0).with_stdout("ftest off").with_stderr(
            "\
[..]Compiling a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target[/]debug[/]a[EXE]`
",
        ),
    );

    /* Build b/ package again. If we cache different feature dep builds correctly,
     * this should not cause a rebuild */
    assert_that(
        p.cargo("clean").arg("-p").arg("b").cwd(p.root().join("b")),
        execs().with_status(0),
    );
    assert_that(
        p.cargo("run").cwd(p.root().join("b")),
        execs().with_status(0).with_stdout("ftest on").with_stderr(
            "\
[..]Compiling b v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[..]target[/]debug[/]b[EXE]`
",
        ),
    );
}

#[test]
fn changing_bin_features_caches_targets() {
    let p = project("foo")
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

    // Windows has a problem with replacing a binary that was just executed.
    // Unlinking it will succeed, but then attempting to immediately replace
    // it will sometimes fail with "Already Exists".
    // See https://github.com/rust-lang/cargo/issues/5481
    let foo_proc = |name: &str| {
        let src = p.bin("foo");
        let dst = p.bin(name);
        fs::copy(&src, &dst).expect("Failed to copy foo");
        p.process(dst)
    };

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        foo_proc("off1"),
        execs().with_status(0).with_stdout("feature off"),
    );

    assert_that(
        p.cargo("build").arg("--features").arg("foo"),
        execs().with_status(0).with_stderr(
            "\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        foo_proc("on1"),
        execs().with_status(0).with_stdout("feature on"),
    );

    /* Targets should be cached from the first build */

    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr(
            "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        foo_proc("off2"),
        execs().with_status(0).with_stdout("feature off"),
    );

    assert_that(
        p.cargo("build").arg("--features").arg("foo"),
        execs().with_status(0).with_stderr(
            "\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        foo_proc("on2"),
        execs().with_status(0).with_stdout("feature on"),
    );
}

#[test]
fn rebuild_tests_if_lib_changes() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#,
        )
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

    assert_that(p.cargo("build"), execs().with_status(0));
    assert_that(p.cargo("test"), execs().with_status(0));

    sleep_ms(1000);
    File::create(&p.root().join("src/lib.rs")).unwrap();

    assert_that(p.cargo("build").arg("-v"), execs().with_status(0));
    assert_that(p.cargo("test").arg("-v"), execs().with_status(101));
}

#[test]
fn no_rebuild_transitive_target_deps() {
    let p = project("foo")
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
        .file(
            "c/Cargo.toml",
            r#"
            [package]
            name = "c"
            version = "0.0.1"
            authors = []
        "#,
        )
        .file("c/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));
    assert_that(
        p.cargo("test").arg("--no-run"),
        execs().with_status(0).with_stderr(
            "\
[COMPILING] c v0.0.1 ([..])
[COMPILING] b v0.0.1 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn rerun_if_changed_in_dep() {
    let p = project("foo")
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

    assert_that(p.cargo("build"), execs().with_status(0));
    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
}

#[test]
fn same_build_dir_cached_packages() {
    let p = project("foo")
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
        .file(
            "d/Cargo.toml",
            r#"
            [package]
            name = "d"
            version = "0.0.1"
            authors = []
        "#,
        )
        .file("d/src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
            [build]
            target-dir = "./target"
        "#,
        )
        .build();

    assert_that(
        p.cargo("build").cwd(p.root().join("a1")),
        execs().with_status(0).with_stderr(&format!(
            "\
[COMPILING] d v0.0.1 ({dir}/d)
[COMPILING] c v0.0.1 ({dir}/c)
[COMPILING] b v0.0.1 ({dir}/b)
[COMPILING] a1 v0.0.1 ({dir}/a1)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
    assert_that(
        p.cargo("build").cwd(p.root().join("a2")),
        execs().with_status(0).with_stderr(&format!(
            "\
[COMPILING] a2 v0.0.1 ({dir}/a2)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.url()
        )),
    );
}

#[test]
fn no_rebuild_if_build_artifacts_move_backwards_in_time() {
    let p = project("backwards_in_time")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "backwards_in_time"
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
        "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));

    p.root().move_into_the_past();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(0)
            .with_stdout("")
            .with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn rebuild_if_build_artifacts_move_forward_in_time() {
    let p = project("forwards_in_time")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "forwards_in_time"
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
        "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));

    p.root().move_into_the_future();

    assert_that(
        p.cargo("build").env("RUST_LOG", ""),
        execs().with_status(0).with_stdout("").with_stderr(
            "\
[COMPILING] a v0.0.1 ([..])
[COMPILING] forwards_in_time v0.0.1 ([..])
[FINISHED] [..]
",
        ),
    );
}

#[test]
fn rebuild_if_environment_changes() {
    let p = project("env_change")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "env_change"
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

    assert_that(
        p.cargo("run"),
        execs()
            .with_status(0)
            .with_stdout("old desc")
            .with_stderr(&format!(
                "\
[COMPILING] env_change v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target[/]debug[/]env_change[EXE]`
",
                dir = p.url()
            )),
    );

    File::create(&p.root().join("Cargo.toml"))
        .unwrap()
        .write_all(
            br#"
        [package]
        name = "env_change"
        description = "new desc"
        version = "0.0.1"
        authors = []
    "#,
        )
        .unwrap();

    assert_that(
        p.cargo("run"),
        execs()
            .with_status(0)
            .with_stdout("new desc")
            .with_stderr(&format!(
                "\
[COMPILING] env_change v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target[/]debug[/]env_change[EXE]`
",
                dir = p.url()
            )),
    );
}

#[test]
fn no_rebuild_when_rename_dir() {
    let p = project("foo")
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
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));
    let mut new = p.root();
    new.pop();
    new.push("bar");
    fs::rename(p.root(), &new).unwrap();

    assert_that(
        p.cargo("build").cwd(&new),
        execs()
            .with_status(0)
            .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"),
    );
}

#[test]
fn unused_optional_dep() {
    Package::new("registry1", "0.1.0").publish();
    Package::new("registry2", "0.1.0").publish();
    Package::new("registry3", "0.1.0").publish();

    let p = project("p")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "p"
                authors = []
                version = "0.1.0"

                [dependencies]
                foo = { path = "foo" }
                bar = { path = "bar" }
                registry1 = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.1"
                authors = []

                [dev-dependencies]
                registry2 = "*"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.1"
                authors = []

                [dependencies]
                registry3 = { version = "*", optional = true }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));
    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn path_dev_dep_registry_updates() {
    Package::new("registry1", "0.1.0").publish();
    Package::new("registry2", "0.1.0").publish();

    let p = project("p")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "p"
                authors = []
                version = "0.1.0"

                [dependencies]
                foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.1"
                authors = []

                [dependencies]
                registry1 = "*"

                [dev-dependencies]
                bar = { path = "../bar"}
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.1"
                authors = []

                [dependencies]
                registry2 = "*"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));
    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn change_panic_mode() {
    let p = project("p")
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ['foo', 'bar']
                [profile.dev]
                panic = 'abort'
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.1"
                authors = []
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.1"
                authors = []

                [lib]
                proc-macro = true

                [dependencies]
                foo = { path = '../foo' }
            "#,
        )
        .file("bar/src/lib.rs", "extern crate foo;")
        .build();

    assert_that(p.cargo("build -p foo"), execs().with_status(0));
    assert_that(p.cargo("build -p bar"), execs().with_status(0));
}

#[test]
fn dont_rebuild_based_on_plugins() {
    let p = project("p")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.1"

                [workspace]
                members = ['bar']

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
                baz = { path = '../baz' }
            "#,
        )
        .file("proc-macro-thing/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.1"

                [dependencies]
                baz = { path = '../baz' }
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.1"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));
    assert_that(p.cargo("build -p bar"), execs().with_status(0));
    assert_that(
        p.cargo("build"),
        execs().with_status(0).with_stderr("[FINISHED] [..]\n"),
    );
    assert_that(
        p.cargo("build -p bar"),
        execs().with_status(0).with_stderr("[FINISHED] [..]\n"),
    );
}
