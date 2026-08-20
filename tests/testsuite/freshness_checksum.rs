//! Tests for checksum-based fingerprinting (rebuild detection).

use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::net::TcpListener;
use std::thread;

use crate::prelude::*;
use cargo_test_support::assert_deps_contains;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, project, rustc_host, str};

#[cargo_test]
fn non_nightly_fails() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("build -Zchecksum-freshness")
        .with_stderr_data(str![[r#"
[ERROR] the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.

"#]])
        .with_status(101)
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
fn checksum_actually_uses_checksum() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("check -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.root().move_into_the_future();

    p.cargo("check -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
fn checksum_build_compatible_with_mtime_build() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("check -Zchecksum-freshness")
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
    p.cargo("check -Zchecksum-freshness")
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

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
fn same_size_different_content() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("check -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.change_file("src/main.rs", "mod a;fn main() { }");

    p.cargo("check -v -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the file `src/main.rs` has changed (checksum didn't match, blake3=26aa07e1adab787246f9d333be65d2eb78dd5fd0fee834ba7a769098b4b651bc != blake3=fc1a42e376d9c148227c13de41b77143f6b5b8132d2b204b63cdbc9326848894)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(
    nightly,
    reason = "-Zbinary-dep-depinfo is unstable, also requires -Zchecksum-hash-algorithm"
)]
fn binary_depinfo_correctly_encoded() {
    Package::new("regdep", "0.1.0")
        .file("src/lib.rs", "pub fn f() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            regdep = "0.1"
            bar = {path = "./bar"}
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                regdep::f();
                bar::f();
            }
            "#,
        )
        /*********** Path Dependency `bar` ***********/
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn f() {}")
        .build();

    let host = rustc_host();
    p.cargo("build -Zbinary-dep-depinfo -Zchecksum-freshness --target")
        .arg(&host)
        .masquerade_as_nightly_cargo(&["binary-dep-depinfo", "checksum-freshness"])
        .with_stderr_data(str![[r#"
...
[COMPILING] foo v0.1.0 ([ROOT]/foo)
...

"#]])
        .run();

    assert_deps_contains(
        &p,
        &format!("target/{}/debug/build/foo/*/fingerprint/dep-bin-foo", host),
        &[
            (0, "src/main.rs"),
            (1, &format!("{}/debug/build/bar/*/out/libbar-*.rlib", host)),
            (
                1,
                &format!("{}/debug/build/regdep/*/out/libregdep-*.rlib", host),
            ),
        ],
    );

    // Make sure it stays fresh.
    p.cargo("build -Zbinary-dep-depinfo -Zchecksum-freshness --target")
        .arg(&host)
        .masquerade_as_nightly_cargo(&["binary-dep-depinfo", "checksum-freshness"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
fn modifying_and_moving() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.root().move_into_the_past();
    p.root().join("target").move_into_the_past();

    p.change_file("src/a.rs", "#[allow(unused)]fn main() {}");
    p.cargo("build -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): file size changed (0 != 28) for `src/a.rs`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    fs::rename(&p.root().join("src/a.rs"), &p.root().join("src/b.rs")).unwrap();
    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
error[E0583]: file not found for module `a`
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to highest compatible versions
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[COMPILING] a v0.0.1 ([ROOT]/foo/a)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.change_file("b/src/lib.rs", "pub fn b() {}");

    p.cargo("build -Zchecksum-freshness -pb -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[DIRTY] b v0.0.1 ([ROOT]/foo/b): file size changed (0 != 13) for `b/src/lib.rs`
[COMPILING] b v0.0.1 ([ROOT]/foo/b)
[RUNNING] `rustc --crate-name b [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.change_file(
        "src/lib.rs",
        "extern crate a; extern crate b; pub fn toplevel() {}",
    );

    p.cargo("build -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
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

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
fn rebuild_tests_if_lib_changes() {
    let p = project()
        .file("src/lib.rs", "pub fn foo() {}")
        .file("tests/foo-test.rs", "extern crate foo;")
        .build();

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();
    p.cargo("test -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();

    p.change_file("src/lib.rs", "");

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();
    p.cargo("test -Zchecksum-freshness -v --test foo-test")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.0.1 ([ROOT]/foo): the dependency `foo` was rebuilt ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo_test [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/debug/build/foo/[HASH]/out/foo_test-[HASH][EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();

    p.root().move_into_the_past();

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stdout_data(str![])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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
    fs::write(p.root().join("src/lib.rs"), "").unwrap();

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();
    let mut new = p.root();
    new.pop();
    new.push("bar");
    fs::rename(p.root(), &new).unwrap();

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .cwd(&new)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();

    p.change_file("reg1new/src/lib.rs", "// modified");

    p.cargo("build -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[DIRTY] registry1 v0.1.0 ([ROOT]/foo/reg1new): file size changed (0 != 11) for `reg1new/src/lib.rs`
[COMPILING] registry1 v0.1.0 ([ROOT]/foo/reg1new)
[RUNNING] `rustc --crate-name registry1 [..]
[DIRTY] registry2 v0.1.0: the dependency `registry1` was rebuilt
[COMPILING] registry2 v0.1.0
[RUNNING] `rustc --crate-name registry2 [..]
[DIRTY] foo v0.0.1 ([ROOT]/foo): the dependency `registry2` was rebuilt
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[FRESH] registry1 v0.1.0 ([ROOT]/foo/reg1new)
[FRESH] registry2 v0.1.0
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[COMPILING] proc_macro_dep v0.1.0 ([ROOT]/foo/proc_macro_dep)
[COMPILING] root v0.1.0 ([ROOT]/foo/root)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data(str![[r#"
[FRESH] proc_macro_dep v0.1.0 ([ROOT]/foo/proc_macro_dep)
[DIRTY] root v0.1.0 ([ROOT]/foo/root): file size changed (150 != 162) for `root/src/lib.rs`
[COMPILING] root v0.1.0 ([ROOT]/foo/root)
[RUNNING] `rustc --crate-name root [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    t.join().ok().unwrap();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();

    // 2 != 1
    p.cargo("test -Zchecksum-freshness --lib")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_status(101)
        .with_stdout_data("...\n[..]doit assert failure[..]\n...")
        .run();

    // Fix the mistake.
    p.change_file("slib.rs", &slib(1));

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();
    // This should recompile with the new static lib, and the test should pass.
    p.cargo("test -Zchecksum-freshness --lib")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();
    p.change_file("helper.rs", r#"pub fn doit() {panic!("Crash!");}"#);
    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data("...\n[..]Crash![..]\n...")
        .with_status(101)
        .run();
    // There was a bug where this second call would be "fresh".
    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .with_stderr_data("...\n[..]Crash![..]\n...")
        .with_status(101)
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();

    // Now rename the root directory and rerun `cargo run`. Not only should we
    // not build anything but we also shouldn't crash.
    let mut new = p.root();
    new.pop();
    new.push("foo2");

    fs::rename(p.root(), &new).unwrap();

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .cwd(&new)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .run();

    let new_target = p.root().join("target2");
    fs::rename(p.root().join("target"), &new_target).unwrap();

    p.cargo("build -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .env("CARGO_TARGET_DIR", &new_target)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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
    p.cargo("check -Zchecksum-freshness --verbose")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
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
    p.cargo("check -Zchecksum-freshness --verbose")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
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
    p.cargo("check -Zchecksum-freshness --verbose")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
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

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
fn skip_checksum_check_in_selected_cargo_home_subdirs() {
    let p = project()
        .at("cargo_home/registry/foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .build();
    let project_root = p.root();
    let cargo_home = project_root.parent().unwrap().parent().unwrap();
    p.cargo("check -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .env("CARGO_HOME", &cargo_home)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/cargo_home/registry/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.change_file("src/lib.rs", "illegal syntax");
    p.cargo("check -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .env("CARGO_HOME", &cargo_home)
        .with_stderr_data(str![[r#"
[FRESH] foo v0.5.0 ([ROOT]/cargo_home/registry/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
fn use_checksum_cache_in_cargo_home() {
    let p = project()
        .at("cargo_home/foo")
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .build();
    let project_root = p.root();
    let cargo_home = project_root.parent().unwrap();
    p.cargo("check -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .env("CARGO_HOME", &cargo_home)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/cargo_home/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..] src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.change_file("src/lib.rs", "illegal syntax");
    p.cargo("check -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .env("CARGO_HOME", &cargo_home)
        .with_status(101)
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.5.0 ([ROOT]/cargo_home/foo): file size changed (0 != 14) for `src/lib.rs`
[CHECKING] foo v0.5.0 ([ROOT]/cargo_home/foo)
[RUNNING] `rustc --crate-name foo [..] src/lib.rs [..]
...
[ERROR] could not compile `foo` (lib) due to 1 previous error
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires -Zchecksum-hash-algorithm")]
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

    p.cargo("check -Zchecksum-freshness")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.change_file("touch-me", "oops");

    // The first one is expected to rerun build script
    p.cargo("check -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
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
    p.cargo("check -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -Zchecksum-freshness -v")
        .masquerade_as_nightly_cargo(&["checksum-freshness"])
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
