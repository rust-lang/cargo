//! Tests for dep-info files. This includes the dep-info file Cargo creates in
//! the output directory, and the ones stored in the fingerprint.

use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::Package;
use cargo_test_support::{
    basic_bin_manifest, basic_manifest, is_nightly, main_file, project, rustc_host, Project,
};
use filetime::FileTime;
use std::fs;
use std::path::Path;

// Helper for testing dep-info files in the fingerprint dir.
fn assert_deps(project: &Project, fingerprint: &str, test_cb: impl Fn(&Path, &[(u8, &str)])) {
    let mut files = project
        .glob(fingerprint)
        .map(|f| f.expect("unwrap glob result"))
        // Filter out `.json` entries.
        .filter(|f| f.extension().is_none());
    let info_path = files
        .next()
        .unwrap_or_else(|| panic!("expected 1 dep-info file at {}, found 0", fingerprint));
    assert!(files.next().is_none(), "expected only 1 dep-info file");
    let dep_info = fs::read(&info_path).unwrap();
    let deps: Vec<(u8, &str)> = dep_info
        .split(|&x| x == 0)
        .filter(|x| !x.is_empty())
        .map(|p| {
            (
                p[0],
                std::str::from_utf8(&p[1..]).expect("expected valid path"),
            )
        })
        .collect();
    test_cb(&info_path, &deps);
}

fn assert_deps_contains(project: &Project, fingerprint: &str, expected: &[(u8, &str)]) {
    assert_deps(project, fingerprint, |info_path, entries| {
        for (e_kind, e_path) in expected {
            let pattern = glob::Pattern::new(e_path).unwrap();
            let count = entries
                .iter()
                .filter(|(kind, path)| kind == e_kind && pattern.matches(path))
                .count();
            if count != 1 {
                panic!(
                    "Expected 1 match of {} {} in {:?}, got {}:\n{:#?}",
                    e_kind, e_path, info_path, count, entries
                );
            }
        }
    })
}

#[cargo_test]
fn build_dep_info() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build").run();

    let depinfo_bin_path = &p.bin("foo").with_extension("d");

    assert!(depinfo_bin_path.is_file());

    let depinfo = p.read_file(depinfo_bin_path.to_str().unwrap());

    let bin_path = p.bin("foo");
    let src_path = p.root().join("src").join("foo.rs");
    if !depinfo.lines().any(|line| {
        line.starts_with(&format!("{}:", bin_path.display()))
            && line.contains(src_path.to_str().unwrap())
    }) {
        panic!(
            "Could not find {:?}: {:?} in {:?}",
            bin_path, src_path, depinfo_bin_path
        );
    }
}

#[cargo_test]
fn build_dep_info_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["lib"]
        "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "lib").with_extension("d").is_file());
}

#[cargo_test]
fn build_dep_info_rlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["rlib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "rlib").with_extension("d").is_file());
}

#[cargo_test]
fn build_dep_info_dylib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["dylib"]
        "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "dylib").with_extension("d").is_file());
}

#[cargo_test]
fn no_rewrite_if_no_change() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build").run();
    let dep_info = p.root().join("target/debug/libfoo.d");
    let metadata1 = dep_info.metadata().unwrap();
    p.cargo("build").run();
    let metadata2 = dep_info.metadata().unwrap();

    assert_eq!(
        FileTime::from_last_modification_time(&metadata1),
        FileTime::from_last_modification_time(&metadata2),
    );
}

#[cargo_test]
fn relative_depinfo_paths_ws() {
    if !is_nightly() {
        // See https://github.com/rust-lang/rust/issues/63012
        return;
    }

    // Test relative dep-info paths in a workspace with --target with
    // proc-macros and other dependency kinds.
    Package::new("regdep", "0.1.0")
        .file("src/lib.rs", "pub fn f() {}")
        .publish();
    Package::new("pmdep", "0.1.0")
        .file("src/lib.rs", "pub fn f() {}")
        .publish();
    Package::new("bdep", "0.1.0")
        .file("src/lib.rs", "pub fn f() {}")
        .publish();

    let p = project()
        /*********** Workspace ***********/
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["foo"]
            "#,
        )
        /*********** Main Project ***********/
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            pm = {path = "../pm"}
            bar = {path = "../bar"}
            regdep = "0.1"

            [build-dependencies]
            bdep = "0.1"
            bar = {path = "../bar"}
            "#,
        )
        .file(
            "foo/src/main.rs",
            r#"
            pm::noop!{}

            fn main() {
                bar::f();
                regdep::f();
            }
            "#,
        )
        .file("foo/build.rs", "fn main() { bdep::f(); }")
        /*********** Proc Macro ***********/
        .file(
            "pm/Cargo.toml",
            r#"
            [package]
            name = "pm"
            version = "0.1.0"
            edition = "2018"

            [lib]
            proc-macro = true

            [dependencies]
            pmdep = "0.1"
            "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro]
            pub fn noop(_item: TokenStream) -> TokenStream {
                pmdep::f();
                "".parse().unwrap()
            }
            "#,
        )
        /*********** Path Dependency `bar` ***********/
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn f() {}")
        .build();

    let host = rustc_host();
    p.cargo("build -Z binary-dep-depinfo --target")
        .arg(&host)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();

    assert_deps_contains(
        &p,
        "target/debug/.fingerprint/pm-*/dep-lib-pm-*",
        &[(1, "src/lib.rs"), (2, "debug/deps/libpmdep-*.rlib")],
    );

    assert_deps_contains(
        &p,
        &format!("target/{}/debug/.fingerprint/foo-*/dep-bin-foo*", host),
        &[
            (1, "src/main.rs"),
            (
                2,
                &format!(
                    "debug/deps/{}pm-*.{}",
                    paths::get_lib_prefix("proc-macro"),
                    paths::get_lib_extension("proc-macro")
                ),
            ),
            (2, &format!("{}/debug/deps/libbar-*.rlib", host)),
            (2, &format!("{}/debug/deps/libregdep-*.rlib", host)),
        ],
    );

    assert_deps_contains(
        &p,
        "target/debug/.fingerprint/foo-*/dep-build-script-build_script_build-*",
        &[(1, "build.rs"), (2, "debug/deps/libbdep-*.rlib")],
    );

    // Make sure it stays fresh.
    p.cargo("build -Z binary-dep-depinfo --target")
        .arg(&host)
        .masquerade_as_nightly_cargo()
        .with_stderr("[FINISHED] dev [..]")
        .run();
}

#[cargo_test]
fn relative_depinfo_paths_no_ws() {
    if !is_nightly() {
        // See https://github.com/rust-lang/rust/issues/63012
        return;
    }

    // Test relative dep-info paths without a workspace with proc-macros and
    // other dependency kinds.
    Package::new("regdep", "0.1.0")
        .file("src/lib.rs", "pub fn f() {}")
        .publish();
    Package::new("pmdep", "0.1.0")
        .file("src/lib.rs", "pub fn f() {}")
        .publish();
    Package::new("bdep", "0.1.0")
        .file("src/lib.rs", "pub fn f() {}")
        .publish();

    let p = project()
        /*********** Main Project ***********/
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            pm = {path = "pm"}
            bar = {path = "bar"}
            regdep = "0.1"

            [build-dependencies]
            bdep = "0.1"
            bar = {path = "bar"}
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            pm::noop!{}

            fn main() {
                bar::f();
                regdep::f();
            }
            "#,
        )
        .file("build.rs", "fn main() { bdep::f(); }")
        /*********** Proc Macro ***********/
        .file(
            "pm/Cargo.toml",
            r#"
            [package]
            name = "pm"
            version = "0.1.0"
            edition = "2018"

            [lib]
            proc-macro = true

            [dependencies]
            pmdep = "0.1"
            "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro]
            pub fn noop(_item: TokenStream) -> TokenStream {
                pmdep::f();
                "".parse().unwrap()
            }
            "#,
        )
        /*********** Path Dependency `bar` ***********/
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn f() {}")
        .build();

    p.cargo("build -Z binary-dep-depinfo")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();

    assert_deps_contains(
        &p,
        "target/debug/.fingerprint/pm-*/dep-lib-pm-*",
        &[(1, "src/lib.rs"), (2, "debug/deps/libpmdep-*.rlib")],
    );

    assert_deps_contains(
        &p,
        "target/debug/.fingerprint/foo-*/dep-bin-foo*",
        &[
            (1, "src/main.rs"),
            (
                2,
                &format!(
                    "debug/deps/{}pm-*.{}",
                    paths::get_lib_prefix("proc-macro"),
                    paths::get_lib_extension("proc-macro")
                ),
            ),
            (2, "debug/deps/libbar-*.rlib"),
            (2, "debug/deps/libregdep-*.rlib"),
        ],
    );

    assert_deps_contains(
        &p,
        "target/debug/.fingerprint/foo-*/dep-build-script-build_script_build-*",
        &[(1, "build.rs"), (2, "debug/deps/libbdep-*.rlib")],
    );

    // Make sure it stays fresh.
    p.cargo("build -Z binary-dep-depinfo")
        .masquerade_as_nightly_cargo()
        .with_stderr("[FINISHED] dev [..]")
        .run();
}

#[cargo_test]
fn reg_dep_source_not_tracked() {
    // Make sure source files in dep-info file are not tracked for registry dependencies.
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

            [dependencies]
            regdep = "0.1"
            "#,
        )
        .file("src/lib.rs", "pub fn f() { regdep::f(); }")
        .build();

    p.cargo("build").run();

    assert_deps(
        &p,
        "target/debug/.fingerprint/regdep-*/dep-lib-regdep-*",
        |info_path, entries| {
            for (kind, path) in entries {
                if *kind == 1 {
                    panic!(
                        "Did not expect package root relative path type: {:?} in {:?}",
                        path, info_path
                    );
                }
            }
        },
    );
}

#[cargo_test]
fn canonical_path() {
    if !is_nightly() {
        // See https://github.com/rust-lang/rust/issues/63012
        return;
    }
    if !cargo_test_support::symlink_supported() {
        return;
    }
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

            [dependencies]
            regdep = "0.1"
            "#,
        )
        .file("src/lib.rs", "pub fn f() { regdep::f(); }")
        .build();

    let real = p.root().join("real_target");
    real.mkdir_p();
    p.symlink(real, "target");

    p.cargo("build -Z binary-dep-depinfo")
        .masquerade_as_nightly_cargo()
        .run();

    assert_deps_contains(
        &p,
        "target/debug/.fingerprint/foo-*/dep-lib-foo-*",
        &[(1, "src/lib.rs"), (2, "debug/deps/libregdep-*.rmeta")],
    );
}
