//! Tests for dep-info files. This includes the dep-info file Cargo creates in
//! the output directory, and the ones stored in the fingerprint.

use cargo_test_support::compare::assert_match_exact;
use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::Package;
use cargo_test_support::{
    basic_bin_manifest, basic_manifest, is_nightly, main_file, project, rustc_host, Project,
};
use filetime::FileTime;
use std::convert::TryInto;
use std::fs;
use std::path::Path;
use std::str;

// Helper for testing dep-info files in the fingerprint dir.
#[track_caller]
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
    let dep_info = &mut &dep_info[..];
    let deps = (0..read_usize(dep_info))
        .map(|_| {
            (
                read_u8(dep_info),
                str::from_utf8(read_bytes(dep_info)).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    test_cb(&info_path, &deps);

    fn read_usize(bytes: &mut &[u8]) -> usize {
        let ret = &bytes[..4];
        *bytes = &bytes[4..];

        u32::from_le_bytes(ret.try_into().unwrap()) as usize
    }

    fn read_u8(bytes: &mut &[u8]) -> u8 {
        let ret = bytes[0];
        *bytes = &bytes[1..];
        ret
    }

    fn read_bytes<'a>(bytes: &mut &'a [u8]) -> &'a [u8] {
        let n = read_usize(bytes);
        let ret = &bytes[..n];
        *bytes = &bytes[n..];
        ret
    }
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

#[ignore]
#[cargo_test]
fn dep_path_inside_target_has_correct_path() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("a"))
        .file(&format!("target/{}/debug/blah", rustc_host()), "")
        .file(
            "src/main.rs",
            &format!(
                r#"
                    fn main() {{
                        let x = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/target/{}/debug/blah"));
                    }}
                "#,
                rustc_host()
            ),
        )
        .build();

    p.cargo("build").run();

    let depinfo_path = &p.bin("a").with_extension("d");

    assert!(depinfo_path.is_file(), "{:?}", depinfo_path);

    let depinfo = p.read_file(depinfo_path.to_str().unwrap());

    let bin_path = p.bin("a");
    let target_debug_blah = Path::new("target")
        .join(rustc_host())
        .join("debug")
        .join("blah");
    if !depinfo.lines().any(|line| {
        line.starts_with(&format!("{}:", bin_path.display()))
            && line.contains(target_debug_blah.to_str().unwrap())
    }) {
        panic!(
            "Could not find {:?}: {:?} in {:?}",
            bin_path, target_debug_blah, depinfo_path
        );
    }
}

#[cargo_test]
fn no_rewrite_if_no_change() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build").run();
    let dep_info = p
        .root()
        .join("target")
        .join(rustc_host())
        .join("debug/libfoo.d");
    let metadata1 = dep_info.metadata().unwrap();
    p.cargo("build").run();
    let metadata2 = dep_info.metadata().unwrap();

    assert_eq!(
        FileTime::from_last_modification_time(&metadata1),
        FileTime::from_last_modification_time(&metadata2),
    );
}

#[ignore]
#[cargo_test]
fn relative_depinfo_paths_ws() {
    if !is_nightly() {
        // -Z binary-dep-depinfo is unstable (https://github.com/rust-lang/rust/issues/63012)
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
        &format!("target/{}/debug/.fingerprint/pm-*/dep-lib-pm", rustc_host()),
        &[(0, "src/lib.rs"), (1, "debug/deps/libpmdep-*.rlib")],
    );

    assert_deps_contains(
        &p,
        &format!("target/{}/debug/.fingerprint/foo-*/dep-bin-foo", host),
        &[
            (0, "src/main.rs"),
            (
                1,
                &format!(
                    "debug/deps/{}pm-*.{}",
                    paths::get_lib_prefix("proc-macro"),
                    paths::get_lib_extension("proc-macro")
                ),
            ),
            (1, &format!("{}/debug/deps/libbar-*.rlib", host)),
            (1, &format!("{}/debug/deps/libregdep-*.rlib", host)),
        ],
    );

    assert_deps_contains(
        &p,
        &format!(
            "target/{}/debug/.fingerprint/foo-*/dep-build-script-build-script-build",
            rustc_host()
        ),
        &[(0, "build.rs"), (1, "debug/deps/libbdep-*.rlib")],
    );

    // Make sure it stays fresh.
    p.cargo("build -Z binary-dep-depinfo --target")
        .arg(&host)
        .masquerade_as_nightly_cargo()
        .with_stderr("[FINISHED] dev [..]")
        .run();
}

#[ignore]
#[cargo_test]
fn relative_depinfo_paths_no_ws() {
    if !is_nightly() {
        // -Z binary-dep-depinfo is unstable (https://github.com/rust-lang/rust/issues/63012)
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
        &format!("target/{}/debug/.fingerprint/pm-*/dep-lib-pm", rustc_host()),
        &[(0, "src/lib.rs"), (1, "debug/deps/libpmdep-*.rlib")],
    );

    assert_deps_contains(
        &p,
        &format!(
            "target/{}/debug/.fingerprint/foo-*/dep-bin-foo",
            rustc_host()
        ),
        &[
            (0, "src/main.rs"),
            (
                1,
                &format!(
                    "debug/deps/{}pm-*.{}",
                    paths::get_lib_prefix("proc-macro"),
                    paths::get_lib_extension("proc-macro")
                ),
            ),
            (1, "debug/deps/libbar-*.rlib"),
            (1, "debug/deps/libregdep-*.rlib"),
        ],
    );

    assert_deps_contains(
        &p,
        &format!(
            "target/{}/debug/.fingerprint/foo-*/dep-build-script-build-script-build",
            rustc_host()
        ),
        &[(0, "build.rs"), (1, "debug/deps/libbdep-*.rlib")],
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
        &format!(
            "target/{}/debug/.fingerprint/regdep-*/dep-lib-regdep",
            rustc_host()
        ),
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

#[ignore]
#[cargo_test]
fn canonical_path() {
    if !is_nightly() {
        // -Z binary-dep-depinfo is unstable (https://github.com/rust-lang/rust/issues/63012)
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
    p.symlink(real, &format!("target/{}", rustc_host()));

    p.cargo("build -Z binary-dep-depinfo")
        .masquerade_as_nightly_cargo()
        .run();

    assert_deps_contains(
        &p,
        &format!(
            "target/{}/debug/.fingerprint/foo-*/dep-lib-foo",
            rustc_host()
        ),
        &[(0, "src/lib.rs"), (1, "debug/deps/libregdep-*.rmeta")],
    );
}

#[cargo_test]
fn non_local_build_script() {
    // Non-local build script information is not included.
    Package::new("bar", "1.0.0")
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
                bar = "1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    let contents = p.read_file(
        p.root()
            .join("target")
            .join(rustc_host())
            .join("debug/foo.d")
            .to_str()
            .unwrap(),
    );
    assert_match_exact(
        &format!(
            "[ROOT]/foo/target/{}/debug/foo[EXE]: [ROOT]/foo/src/main.rs",
            rustc_host()
        ),
        &contents,
    );
}
