//! Tests for fingerprinting (rebuild detection).
//!
//! This must be kept in-sync with `freshness_checksum.rs`

use std::fs::{self, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::SystemTime;

use crate::prelude::*;
use cargo_test_support::paths;
use cargo_test_support::registry::Package;
use cargo_test_support::{
    basic_lib_manifest, basic_manifest, is_coarse_mtime, project, sleep_ms, str,
};
use filetime::FileTime;

#[cargo_test]
fn modifying_and_moving() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.root().move_into_the_past();
    p.root().join("target").move_into_the_past();

    p.change_file("src/a.rs", "#[allow(unused)]fn main() {}");
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the file `src/a.rs` has changed ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    fs::rename(&p.root().join("src/a.rs"), &p.root().join("src/b.rs")).unwrap();
    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
error[E0583]: file not found for module `a`
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("test").run();
    sleep_ms(1000);

    assert!(p.bin("foo").is_file());

    let lib = p.root().join("src/lib.rs");
    p.change_file("src/lib.rs", "invalid rust code");
    p.change_file("src/b.rs", "#[allow(unused)]fn foo() {}");
    lib.move_into_the_past();

    // Make sure the binary is rebuilt, not the lib
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the file `src/b.rs` has changed ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"

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
                edition = "2015"
                [dependencies.b]
                path = "../b"
            "#,
        )
        .file("a/src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to highest compatible versions
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    if is_coarse_mtime() {
        sleep_ms(1000);
    }
    p.change_file("b/src/lib.rs", "pub fn b() {}");

    p.cargo("build -pb -v")
        .with_stderr_data(str![[r#"
[DIRTY] b v0.0.1 ([ROOT]/foo/b): the file `b/src/lib.rs` has changed ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[RUNNING] `rustc --crate-name b [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.change_file(
        "src/lib.rs",
        "extern crate a; extern crate b; pub fn toplevel() {}",
    );

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[FRESH] b v0.0.1 ([ROOT]/foo/b)
[DIRTY] a v0.0.1 ([ROOT]/foo/a): the dependency `b` was rebuilt ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[RUNNING] `rustc --crate-name a [..]
[DIRTY] foo v0.0.1 ([ROOT]/foo): the dependency `b` was rebuilt ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rebuild_tests_if_lib_changes() {
    let p = project()
        .file("src/lib.rs", "pub fn foo() {}")
        .file("tests/foo-test.rs", "extern crate foo;")
        .build();

    p.cargo("build").run();
    p.cargo("test").run();

    sleep_ms(1000);
    p.change_file("src/lib.rs", "");

    p.cargo("build").run();
    p.cargo("test -v --test foo-test")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the dependency `foo` was rebuilt ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo_test [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/foo_test-[HASH][EXE]`

"#]])
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
                edition = "2015"
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
        .with_stdout_data(str![])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
        .with_stdout_data(str![])
        .with_stderr_data(str![[r#"
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
                authors = []

                [workspace]

                [dependencies]
                foo = { path = "foo" }
            "#,
        )
        .file("src/_unused.rs", "")
        .file("build.rs", "fn main() {}")
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.0.1"))
        .file("foo/src/lib.rs", "")
        .file("foo/build.rs", "fn main() {}")
        .build();

    // make sure the most recently modified file is `src/lib.rs`, not
    // `Cargo.toml`, to expose a historical bug where we forgot to strip the
    // `Cargo.toml` path from looking for the package root.
    cargo_test_support::sleep_ms(100);
    fs::write(p.root().join("src/lib.rs"), "").unwrap();

    p.cargo("build").run();
    let mut new = p.root();
    new.pop();
    new.push("bar");
    fs::rename(p.root(), &new).unwrap();

    p.cargo("build")
        .cwd(&new)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
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
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build -Z mtime-on-use")
        .masquerade_as_nightly_cargo(&["mtime-on-use"])
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // This does not make new files, but it does update the mtime of the dependency.
    p.cargo("build -p bar -Z mtime-on-use")
        .masquerade_as_nightly_cargo(&["mtime-on-use"])
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // This should not recompile!
    p.cargo("build -Z mtime-on-use")
        .masquerade_as_nightly_cargo(&["mtime-on-use"])
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
    let mut cleaned = false;
    dir.push("build");
    let outdated = |f: io::Result<fs::DirEntry>| {
        filetime::FileTime::from_last_modification_time(&f.unwrap().metadata().unwrap())
            <= timestamp
    };
    for build_unit in fs::read_dir(&dir).unwrap() {
        let build_unit = build_unit.unwrap();
        for hash_dir in fs::read_dir(&build_unit.path()).unwrap() {
            let hash_dir = hash_dir.unwrap();
            let fingerprint = hash_dir.path().join("fingerprint");
            if fs::read_dir(fingerprint).unwrap().all(outdated) {
                fs::remove_dir_all(hash_dir.path()).unwrap();
                println!("remove: {:?}", hash_dir.path());
                // a real cleaner would remove the big files in deps and build as well
                // but fingerprint is sufficient for our tests
                cleaned = true;
            }
        }
    }
    assert!(
        cleaned,
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
                edition = "2015"

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
        .masquerade_as_nightly_cargo(&["mtime-on-use"])
        .run();
    p.cargo("build -Z mtime-on-use --features a")
        .masquerade_as_nightly_cargo(&["mtime-on-use"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .masquerade_as_nightly_cargo(&["mtime-on-use"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    fingerprint_cleaner(p.target_debug_dir(), timestamp);
    // This should not recompile!
    p.cargo("build -Z mtime-on-use --features a")
        .masquerade_as_nightly_cargo(&["mtime-on-use"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // But this should be cleaned and so need a rebuild
    p.cargo("build -Z mtime-on-use")
        .masquerade_as_nightly_cargo(&["mtime-on-use"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"

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

    p.change_file("reg1new/src/lib.rs", "// modified");
    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    p.cargo("build -v").with_stderr_data(str![[r#"
[DIRTY] registry1 v0.1.0 ([ROOT]/foo/reg1new): the file `reg1new/src/lib.rs` has changed ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] registry1 v0.1.0 ([ROOT]/foo/reg1new)
[RUNNING] `rustc --crate-name registry1 [..]
[DIRTY] registry2 v0.1.0: the dependency `registry1` was rebuilt
[COMPILING] registry2 v0.1.0
[RUNNING] `rustc --crate-name registry2 [..]
[DIRTY] foo v0.0.1 ([ROOT]/foo): the dependency `registry2` was rebuilt
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();

    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[FRESH] registry1 v0.1.0 ([ROOT]/foo/reg1new)
[FRESH] registry2 v0.1.0
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[COMPILING] proc_macro_dep v0.1.0 ([ROOT]/foo/proc_macro_dep)
[COMPILING] root v0.1.0 ([ROOT]/foo/root)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build -v").with_stderr_data(str![[r#"
[FRESH] proc_macro_dep v0.1.0 ([ROOT]/foo/proc_macro_dep)
[DIRTY] root v0.1.0 ([ROOT]/foo/root): the file `root/src/lib.rs` has changed ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] root v0.1.0 ([ROOT]/foo/root)
[RUNNING] `rustc --crate-name root [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();

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
                    println!("cargo::rustc-link-lib=slib");
                    println!("cargo::rustc-link-search={}", out_dir.display());
                }
            "#,
        )
        .file("slib.rs", &slib(2))
        .build();

    p.cargo("build").run();

    // 2 != 1
    p.cargo("test --lib")
        .with_status(101)
        .with_stdout_data("...\n[..]doit assert failure[..]\n...")
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
                    println!("cargo::rerun-if-changed=build.rs");
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
        .with_stderr_data("...\n[..]Crash![..]\n...")
        .with_status(101)
        .run();
    // There was a bug where this second call would be "fresh".
    p.cargo("build")
        .with_stderr_data("...\n[..]Crash![..]\n...")
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
                println!("cargo::rerun-if-env-changed=SOMEVAR");
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
                println!("cargo::rerun-if-changed=build.rs");
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
            edition = "2015"

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
            .with_stderr_data(
                str![[r#"
[FRESH] pathdep [..]
[FRESH] regdep [..]
[FRESH] regdep_env [..]
[FRESH] regdep_old_style [..]
[FRESH] regdep_rerun [..]
[FRESH] foo [..]
[FINISHED] [..]

"#]]
                .unordered(),
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
            .with_stderr_data(
                str![[r#"
[FRESH] regdep [..]
[FRESH] pathdep [..]
[FRESH] regdep_env [..]
[DIRTY] foo v0.1.0 ([ROOT]/foo): the precalculated components changed
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FRESH] regdep_rerun [..]
[FRESH] regdep_old_style [..]
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/build_script_build`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
                .unordered(),
            )
            .run();
    }
}

#[cargo_test]
fn rename_with_path_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = 'a' }
            "#,
        )
        .file("src/lib.rs", "extern crate a; pub fn foo() { a::foo(); }")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                b = { path = 'b' }
            "#,
        )
        .file("a/src/lib.rs", "extern crate b; pub fn foo() { b::foo() }")
        .file(
            "a/b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.5.0"
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn move_target_directory_with_path_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "a" }
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                edition = "2015"
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
                    println!("cargo::rerun-if-changed=build.rs");
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
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn verify_source_before_recompile() {
    Package::new("bar", "0.1.0")
        .file("src/lib.rs", "")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("vendor --respect-source-config").run();
    p.change_file(
        ".cargo/config.toml",
        r#"
            [source.crates-io]
            replace-with = 'vendor'

            [source.vendor]
            directory = 'vendor'
        "#,
    );
    // Sanity check: vendoring works correctly.
    p.cargo("check --verbose")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.1.0
[RUNNING] `rustc --crate-name bar [..] [ROOT]/foo/vendor/bar/src/lib.rs [..]
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // Now modify vendored crate.
    p.change_file(
        "vendor/bar/src/lib.rs",
        r#"compile_error!("You shall not pass!");"#,
    );
    // Should ignore modified sources without any recompile.
    p.cargo("check --verbose")
        .with_stderr_data(str![[r#"
[FRESH] bar v0.1.0
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Add a `RUSTFLAGS` to trigger a recompile.
    //
    // Cargo should refuse to build because of checksum verification failure.
    // Cargo shouldn't recompile dependency `bar`.
    p.cargo("check --verbose")
        .env("RUSTFLAGS", "-W warnings")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the listed checksum of `[ROOT]/foo/vendor/bar/src/lib.rs` has changed:
expected: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
actual:   66e843918c1d4ea8231af814f9f958958808249d4407de01114acb730ecd9bdf

directory sources are not intended to be edited, if modifications are required then it is recommended that `[patch]` is used with a forked copy of the source

"#]])
        .run();
}

#[cargo_test]
fn skip_mtime_check_in_selected_cargo_home_subdirs() {
    let p = project()
        .at("cargo_home/registry/foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .build();
    let project_root = p.root();
    let cargo_home = project_root.parent().unwrap().parent().unwrap();
    p.cargo("check -v")
        .env("CARGO_HOME", &cargo_home)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/cargo_home/registry/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.change_file("src/lib.rs", "illegal syntax");
    p.cargo("check -v")
        .env("CARGO_HOME", &cargo_home)
        .with_stderr_data(str![[r#"
[FRESH] foo v0.5.0 ([ROOT]/cargo_home/registry/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn use_mtime_cache_in_cargo_home() {
    let p = project()
        .at("cargo_home/foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .build();
    let project_root = p.root();
    let cargo_home = project_root.parent().unwrap();
    p.cargo("check -v")
        .env("CARGO_HOME", &cargo_home)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/cargo_home/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..] src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.change_file("src/lib.rs", "illegal syntax");
    p.cargo("check -v")
        .env("CARGO_HOME", &cargo_home)
        .with_status(101)
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.5.0 ([ROOT]/cargo_home/foo): the file `src/lib.rs` has changed ([TIME_DIFF_AFTER_LAST_BUILD])
[CHECKING] foo v0.5.0 ([ROOT]/cargo_home/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]
...
[ERROR] could not compile `foo` (lib) due to 1 previous error
...
"#]])
        .run();
}

#[cargo_test]
fn incremental_build_script_execution_got_new_mtime_and_cargo_check() {
    // See https://github.com/rust-lang/cargo/issues/16104
    let p = project()
        .file("src/lib.rs", "")
        .file("touch-me", "")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo::rerun-if-changed=touch-me") }"#,
        )
        .build();

    p.cargo("check")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    if is_coarse_mtime() {
        sleep_ms(1000);
    }

    p.change_file("touch-me", "oops");

    // The first one is expected to rerun build script
    p.cargo("check -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the file `touch-me` has changed ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/build_script_build`
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // subsequent cargo check gets stuck...
    p.cargo("check -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn symlink_to_package() {
    // Illustrates what happens when the path is a symlink, and the symlink
    // target changes.
    if !cargo_test_support::symlink_supported() {
        return;
    }
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    bar::bar();
                }
            "#,
        )
        .file("bar1/Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file(
            "bar1/src/lib.rs",
            r#"
                pub fn bar() {
                    println!("one");
                }
            "#,
        )
        .file("bar2/Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file(
            "bar2/src/lib.rs",
            r#"
                pub fn bar() {
                    println!("two");
                }
            "#,
        )
        .symlink_dir("bar1", "bar")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[CHECKING] bar v1.0.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    let bar_path = p.root().join("bar");
    cargo_util::paths::remove_file(&bar_path).unwrap();
    p.symlink("bar2", "bar");
    // FIXME: This is not rebuilding when it should.
    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
