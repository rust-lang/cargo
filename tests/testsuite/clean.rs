//! Tests for the `cargo clean` command.

use cargo_test_support::compare::assert_e2e;
use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{
    basic_bin_manifest, basic_manifest, git, main_file, project, project_in, rustc_host,
};
use glob::GlobError;
use std::env;
use std::path::{Path, PathBuf};

#[cargo_test]
fn cargo_clean_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build").run();
    assert!(p.build_dir().is_dir());

    p.cargo("clean").run();
    assert!(!p.build_dir().is_dir());
}

#[cargo_test]
fn different_dir() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("src/bar/a.rs", "")
        .build();

    p.cargo("build").run();
    assert!(p.build_dir().is_dir());

    p.cargo("clean")
        .cwd("src")
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    assert!(!p.build_dir().is_dir());
}

#[cargo_test]
fn clean_multiple_packages() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.d1]
                    path = "d1"
                [dependencies.d2]
                    path = "d2"

                [[bin]]
                    name = "foo"
            "#,
        )
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file("d2/Cargo.toml", &basic_bin_manifest("d2"))
        .file("d2/src/main.rs", "fn main() { println!(\"d2\"); }")
        .build();

    p.cargo("build -p d1 -p d2 -p foo").run();

    let d1_path = &p
        .build_dir()
        .join("debug")
        .join(format!("d1{}", env::consts::EXE_SUFFIX));
    let d2_path = &p
        .build_dir()
        .join("debug")
        .join(format!("d2{}", env::consts::EXE_SUFFIX));

    assert!(p.bin("foo").is_file());
    assert!(d1_path.is_file());
    assert!(d2_path.is_file());

    p.cargo("clean -p d1 -p d2")
        .cwd("src")
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    assert!(p.bin("foo").is_file());
    assert!(!d1_path.is_file());
    assert!(!d2_path.is_file());
}

#[cargo_test]
fn clean_multiple_packages_in_glob_char_path() {
    let p = project_in("[d1]")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();
    let foo_path = &p.build_dir().join("debug").join("deps");

    #[cfg(not(target_env = "msvc"))]
    let file_glob = "foo-*";

    #[cfg(target_env = "msvc")]
    let file_glob = "foo.pdb";

    // Assert that build artifacts are produced
    p.cargo("build").run();
    assert_ne!(get_build_artifacts(foo_path, file_glob).len(), 0);

    // Assert that build artifacts are destroyed
    p.cargo("clean -p foo").run();
    assert_eq!(get_build_artifacts(foo_path, file_glob).len(), 0);
}

fn get_build_artifacts(path: &PathBuf, file_glob: &str) -> Vec<Result<PathBuf, GlobError>> {
    let pattern = path.to_str().expect("expected utf-8 path");
    let pattern = glob::Pattern::escape(pattern);

    let path = PathBuf::from(pattern).join(file_glob);
    let path = path.to_str().expect("expected utf-8 path");
    glob::glob(path)
        .expect("expected glob to run")
        .into_iter()
        .collect::<Vec<Result<PathBuf, GlobError>>>()
}

#[cargo_test]
fn clean_p_only_cleans_specified_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = [
                    "foo",
                    "foo_core",
                    "foo-base",
                ]
            "#,
        )
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "//! foo")
        .file("foo_core/Cargo.toml", &basic_manifest("foo_core", "0.1.0"))
        .file("foo_core/src/lib.rs", "//! foo_core")
        .file("foo-base/Cargo.toml", &basic_manifest("foo-base", "0.1.0"))
        .file("foo-base/src/lib.rs", "//! foo-base")
        .build();

    let fingerprint_path = &p.build_dir().join("debug").join(".fingerprint");

    p.cargo("build -p foo -p foo_core -p foo-base").run();

    let mut fingerprint_names = get_fingerprints_without_hashes(fingerprint_path);

    // Artifacts present for all after building
    assert!(fingerprint_names.iter().any(|e| e == "foo"));
    let num_foo_core_artifacts = fingerprint_names
        .iter()
        .filter(|&e| e == "foo_core")
        .count();
    assert_ne!(num_foo_core_artifacts, 0);
    let num_foo_base_artifacts = fingerprint_names
        .iter()
        .filter(|&e| e == "foo-base")
        .count();
    assert_ne!(num_foo_base_artifacts, 0);

    p.cargo("clean -p foo").run();

    fingerprint_names = get_fingerprints_without_hashes(fingerprint_path);

    // Cleaning `foo` leaves artifacts for the others
    assert!(!fingerprint_names.iter().any(|e| e == "foo"));
    assert_eq!(
        fingerprint_names
            .iter()
            .filter(|&e| e == "foo_core")
            .count(),
        num_foo_core_artifacts,
    );
    assert_eq!(
        fingerprint_names
            .iter()
            .filter(|&e| e == "foo-base")
            .count(),
        num_foo_core_artifacts,
    );
}

fn get_fingerprints_without_hashes(fingerprint_path: &Path) -> Vec<String> {
    std::fs::read_dir(fingerprint_path)
        .expect("Build dir should be readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let name = entry.file_name();
            let name = name
                .into_string()
                .expect("fingerprint name should be UTF-8");
            name.rsplit_once('-')
                .expect("Name should contain at least one hyphen")
                .0
                .to_owned()
        })
        .collect()
}

#[cargo_test]
fn clean_release() {
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
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build --release").run();

    p.cargo("clean -p foo").run();
    p.cargo("build --release")
        .with_stderr_data(str![[r#"
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean -p foo --release").run();
    p.cargo("build --release")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build").run();

    p.cargo("clean").arg("--release").run();
    assert!(p.build_dir().is_dir());
    assert!(p.build_dir().join("debug").is_dir());
    assert!(!p.build_dir().join("release").is_dir());
}

#[cargo_test]
fn clean_doc() {
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
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("doc").run();

    let doc_path = &p.build_dir().join("doc");

    assert!(doc_path.is_dir());

    p.cargo("clean --doc")
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();

    assert!(!doc_path.is_dir());
    assert!(p.build_dir().is_dir());
}

#[cargo_test]
fn build_script() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                use std::path::PathBuf;
                use std::env;

                fn main() {
                    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
                    if env::var("FIRST").is_ok() {
                        std::fs::File::create(out.join("out")).unwrap();
                    } else {
                        assert!(!out.join("out").exists());
                    }
                }
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").env("FIRST", "1").run();
    p.cargo("clean -p foo").run();
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] build.rs [..]`
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc [..] src/main.rs [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn clean_git() {
    let git = git::new("dep", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep", "0.5.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    dep = {{ git = '{}' }}
                "#,
                git.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    p.cargo("clean -p dep")
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    p.cargo("build").run();
}

#[cargo_test]
fn registry() {
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
                bar = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.1.0").publish();

    p.cargo("build").run();
    p.cargo("clean -p bar")
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    p.cargo("build").run();
}

#[cargo_test]
fn clean_verbose() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.1.0").publish();

    p.cargo("build").run();
    let mut expected = String::from(
        "\
[REMOVING] [ROOT]/foo/target/debug/.fingerprint/bar-[HASH]
[REMOVING] [ROOT]/foo/target/debug/deps/libbar-[HASH].rlib
[REMOVING] [ROOT]/foo/target/debug/deps/bar-[HASH].d
[REMOVING] [ROOT]/foo/target/debug/deps/libbar-[HASH].rmeta
",
    );
    if cfg!(target_os = "macos") {
        // Rust 1.69 has changed so that split-debuginfo=unpacked includes unpacked for rlibs.
        for _ in p.glob("target/debug/deps/bar-*.o") {
            expected.push_str("[REMOVING] [ROOT]/foo/target/debug/deps/bar-[HASH][..].o\n");
        }
    }
    expected.push_str("[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total\n");
    p.cargo("clean -p bar --verbose")
        .with_stderr_data(&expected.unordered())
        .run();
    p.cargo("build").run();
}

#[cargo_test]
fn clean_remove_rlib_rmeta() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();
    assert!(p.target_debug_dir().join("libfoo.rlib").exists());
    let rmeta = p.glob("target/debug/deps/*.rmeta").next().unwrap().unwrap();
    assert!(rmeta.exists());
    p.cargo("clean -p foo").run();
    assert!(!p.target_debug_dir().join("libfoo.rlib").exists());
    assert!(!rmeta.exists());
}

#[cargo_test]
fn package_cleans_all_the_things() {
    // -p cleans everything
    // Use dashes everywhere to make sure dash/underscore stuff is handled.
    for crate_type in &["rlib", "dylib", "cdylib", "staticlib", "proc-macro"] {
        // Try each crate type individually since the behavior changes when
        // they are combined.
        let p = project()
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "foo-bar"
                    version = "0.1.0"
                    edition = "2015"

                    [lib]
                    crate-type = ["{}"]
                    "#,
                    crate_type
                ),
            )
            .file("src/lib.rs", "")
            .build();
        p.cargo("build").run();
        p.cargo("clean -p foo-bar").run();
        assert_all_clean(&p.build_dir());
    }
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo-bar"
            version = "0.1.0"
            edition = "2018"

            [lib]
            crate-type = ["rlib", "dylib", "staticlib"]

            [[example]]
            name = "foo-ex-rlib"
            crate-type = ["rlib"]
            test = true

            [[example]]
            name = "foo-ex-cdylib"
            crate-type = ["cdylib"]
            test = true

            [[example]]
            name = "foo-ex-bin"
            test = true
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/lib/some-main.rs", "fn main() {}")
        .file("src/bin/other-main.rs", "fn main() {}")
        .file("examples/foo-ex-rlib.rs", "")
        .file("examples/foo-ex-cdylib.rs", "")
        .file("examples/foo-ex-bin.rs", "fn main() {}")
        .file("tests/foo-test.rs", "")
        .file("benches/foo-bench.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build --all-targets")
        .env("CARGO_INCREMENTAL", "1")
        .run();
    p.cargo("test --all-targets")
        .env("CARGO_INCREMENTAL", "1")
        .run();
    p.cargo("check --all-targets")
        .env("CARGO_INCREMENTAL", "1")
        .run();
    p.cargo("clean -p foo-bar").run();
    assert_all_clean(&p.build_dir());

    // Try some targets.
    p.cargo("build --all-targets --target")
        .arg(rustc_host())
        .run();
    p.cargo("clean -p foo-bar --target").arg(rustc_host()).run();
    assert_all_clean(&p.build_dir());
}

// Ensures that all files for the package have been deleted.
#[track_caller]
fn assert_all_clean(build_dir: &Path) {
    let walker = walkdir::WalkDir::new(build_dir).into_iter();
    for entry in walker.filter_entry(|e| {
        let path = e.path();
        // This is a known limitation, clean can't differentiate between
        // the different build scripts from different packages.
        !(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("build_script_build")
            && path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                == "incremental")
    }) {
        let entry = entry.unwrap();
        let path = entry.path();
        if let ".rustc_info.json" | ".cargo-lock" | "CACHEDIR.TAG" =
            path.file_name().unwrap().to_str().unwrap()
        {
            continue;
        }
        if path.is_symlink() || path.is_file() {
            panic!("{:?} was not cleaned", path);
        }
    }
}

#[cargo_test]
fn clean_spec_version() {
    // clean -p foo where foo matches multiple versions
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar1 = {version="0.1", package="bar"}
            bar2 = {version="0.2", package="bar"}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();

    // Check suggestion for bad pkgid.
    p.cargo("clean -p baz")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `baz` did not match any packages

[HELP] a package with a similar name exists: `bar`

"#]])
        .run();

    p.cargo("clean -p bar:0.1.0")
        .with_stderr_data(str![[r#"
[WARNING] version qualifier in `-p bar:0.1.0` is ignored, cleaning all versions of `bar` found
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    let mut walker = walkdir::WalkDir::new(p.build_dir())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_str().unwrap();
            n.starts_with("bar") || n.starts_with("libbar")
        });
    if let Some(e) = walker.next() {
        panic!("{:?} was not cleaned", e.path());
    }
}

#[cargo_test]
fn clean_spec_partial_version() {
    // clean -p foo where foo matches multiple versions
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar1 = {version="0.1", package="bar"}
            bar2 = {version="0.2", package="bar"}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();

    // Check suggestion for bad pkgid.
    p.cargo("clean -p baz")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `baz` did not match any packages

[HELP] a package with a similar name exists: `bar`

"#]])
        .run();

    p.cargo("clean -p bar:0.1")
        .with_stderr_data(str![[r#"
[WARNING] version qualifier in `-p bar:0.1` is ignored, cleaning all versions of `bar` found
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    let mut walker = walkdir::WalkDir::new(p.build_dir())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_str().unwrap();
            n.starts_with("bar") || n.starts_with("libbar")
        });
    if let Some(e) = walker.next() {
        panic!("{:?} was not cleaned", e.path());
    }
}

#[cargo_test]
fn clean_spec_partial_version_ambiguous() {
    // clean -p foo where foo matches multiple versions
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar1 = {version="0.1", package="bar"}
            bar2 = {version="0.2", package="bar"}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();

    // Check suggestion for bad pkgid.
    p.cargo("clean -p baz")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `baz` did not match any packages

[HELP] a package with a similar name exists: `bar`

"#]])
        .run();

    p.cargo("clean -p bar:0")
        .with_stderr_data(str![[r#"
[WARNING] version qualifier in `-p bar:0` is ignored, cleaning all versions of `bar` found
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    let mut walker = walkdir::WalkDir::new(p.build_dir())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_str().unwrap();
            n.starts_with("bar") || n.starts_with("libbar")
        });
    if let Some(e) = walker.next() {
        panic!("{:?} was not cleaned", e.path());
    }
}

#[cargo_test]
fn clean_spec_reserved() {
    // Clean when a target (like a test) has a reserved name. In this case,
    // make sure `clean -p` doesn't delete the reserved directory `build` when
    // there is a test named `build`.
    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
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
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("tests/build.rs", "")
        .build();

    p.cargo("build --all-targets").run();
    assert!(p.target_debug_dir().join("build").is_dir());
    let build_test = p.glob("target/debug/deps/build-*").next().unwrap().unwrap();
    assert!(build_test.exists());
    // Tests are never "uplifted".
    assert!(p.glob("target/debug/build-*").next().is_none());

    p.cargo("clean -p foo").run();
    // Should not delete this.
    assert!(p.target_debug_dir().join("build").is_dir());

    // This should not rebuild bar.
    p.cargo("build -v --all-targets")
        .with_stderr_data(str![[r#"
[FRESH] bar v1.0.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn clean_dry_run() {
    // Basic `clean --dry-run` test.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Start with no files.
    p.cargo("clean --dry-run")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[SUMMARY] 0 files
[WARNING] no files deleted due to --dry-run

"#]])
        .run();
    p.cargo("check").run();
    let before = p.build_dir().ls_r();
    p.cargo("clean --dry-run")
        .with_stderr_data(str![[r#"
[SUMMARY] [FILE_NUM] files, [FILE_SIZE]B total
[WARNING] no files deleted due to --dry-run

"#]])
        .run();
    // Verify it didn't delete anything.
    let after = p.build_dir().ls_r();
    assert_eq!(before, after);
    let mut expected = itertools::join(before.iter().map(|p| p.to_str().unwrap()), "\n");
    expected.push_str("\n");
    let expected = snapbox::filter::normalize_paths(&expected);
    let expected = assert_e2e().redactions().redact(&expected);
    eprintln!("{expected}");
    // Verify the verbose output.
    p.cargo("clean --dry-run -v")
        .with_stdout_data(expected.unordered())
        .with_stderr_data(str![[r#"
[SUMMARY] [FILE_NUM] files, [FILE_SIZE]B total
[WARNING] no files deleted due to --dry-run

"#]])
        .run();
}

#[cargo_test]
fn doc_with_package_selection() {
    // --doc with -p
    let p = project().file("src/lib.rs", "").build();
    p.cargo("clean --doc -p foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] --doc cannot be used with -p

"#]])
        .run();
}

#[cargo_test]
fn quiet_does_not_show_summary() {
    // Checks that --quiet works with `cargo clean`, since there was a
    // subtle issue with how the flag is defined as a global flag.
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();
    p.cargo("clean --quiet --dry-run")
        .with_stdout_data("")
        .with_stderr_data("")
        .run();
    // Verify exact same command without -q would actually display something.
    p.cargo("clean --dry-run")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[SUMMARY] [FILE_NUM] files, [FILE_SIZE]B total
[WARNING] no files deleted due to --dry-run

"#]])
        .run();
}
