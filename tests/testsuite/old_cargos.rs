//! Tests for checking behavior of old cargos.
//!
//! These tests are ignored because it is intended to be run on a developer
//! system with a bunch of toolchains installed. This requires `rustup` to be
//! installed. It will iterate over installed toolchains, and run some tests
//! over each one, producing a report at the end. As of this writing, I have
//! tested 1.0 to 1.51. Run this with:
//!
//! ```console
//! cargo test --test testsuite -- old_cargos --nocapture --ignored
//! ```

use cargo::CargoResult;
use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::{self, Dependency, Package};
use cargo_test_support::{cargo_exe, execs, paths, process, project, rustc_host};
use cargo_util::{ProcessBuilder, ProcessError};
use semver::Version;
use std::fs;

fn tc_process(cmd: &str, toolchain: &str) -> ProcessBuilder {
    let mut p = if toolchain == "this" {
        if cmd == "cargo" {
            process(&cargo_exe())
        } else {
            process(cmd)
        }
    } else {
        let mut cmd = process(cmd);
        cmd.arg(format!("+{}", toolchain));
        cmd
    };
    // Reset PATH since `process` modifies it to remove rustup.
    p.env("PATH", std::env::var_os("PATH").unwrap());
    p
}

/// Returns a sorted list of all toolchains.
///
/// The returned value includes the parsed version, and the rustup toolchain
/// name as a string.
fn collect_all_toolchains() -> Vec<(Version, String)> {
    let rustc_version = |tc| {
        let mut cmd = tc_process("rustc", tc);
        cmd.arg("-V");
        let output = cmd.exec_with_output().expect("rustc installed");
        let version = std::str::from_utf8(&output.stdout).unwrap();
        let parts: Vec<_> = version.split_whitespace().collect();
        assert_eq!(parts[0], "rustc");
        assert!(parts[1].starts_with("1."));
        Version::parse(parts[1]).expect("valid version")
    };

    // Provide a way to override the list.
    if let Ok(tcs) = std::env::var("OLD_CARGO") {
        return tcs
            .split(',')
            .map(|tc| (rustc_version(tc), tc.to_string()))
            .collect();
    }

    let host = rustc_host();
    // I tend to have lots of toolchains installed, but I don't want to test
    // all of them (like dated nightlies, or toolchains for non-host targets).
    let valid_names = &[
        format!("stable-{}", host),
        format!("beta-{}", host),
        format!("nightly-{}", host),
    ];

    let output = ProcessBuilder::new("rustup")
        .args(&["toolchain", "list"])
        .exec_with_output()
        .expect("rustup should be installed");
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let mut toolchains: Vec<_> = stdout
        .lines()
        .map(|line| {
            // Some lines say things like (default), just get the version.
            line.split_whitespace().next().expect("non-empty line")
        })
        .filter(|line| {
            line.ends_with(&host)
                && (line.starts_with("1.") || valid_names.iter().any(|name| name == line))
        })
        .map(|line| (rustc_version(line), line.to_string()))
        .collect();

    toolchains.sort_by(|a, b| a.0.cmp(&b.0));
    toolchains
}

/// Returns whether the default toolchain is the stable version.
fn default_toolchain_is_stable() -> bool {
    let default = tc_process("rustc", "this").arg("-V").exec_with_output();
    let stable = tc_process("rustc", "stable").arg("-V").exec_with_output();
    match (default, stable) {
        (Ok(d), Ok(s)) => d.stdout == s.stdout,
        _ => false,
    }
}

// This is a test for exercising the behavior of older versions of cargo with
// the new feature syntax.
//
// The test involves a few dependencies with different feature requirements:
//
// * `bar` 1.0.0 is the base version that does not use the new syntax.
// * `bar` 1.0.1 has a feature with the new syntax, but the feature is unused.
//   The optional dependency `new-baz-dep` should not be activated.
// * `bar` 1.0.2 has a dependency on `baz` that *requires* the new feature
//   syntax.
#[ignore = "must be run manually, requires old cargo installations"]
#[cargo_test]
fn new_features() {
    let registry = registry::init();
    if std::process::Command::new("rustup").output().is_err() {
        panic!("old_cargos requires rustup to be installed");
    }
    Package::new("new-baz-dep", "1.0.0").publish();

    Package::new("baz", "1.0.0").publish();
    let baz101_cksum = Package::new("baz", "1.0.1")
        .add_dep(Dependency::new("new-baz-dep", "1.0").optional(true))
        .feature("new-feat", &["dep:new-baz-dep"])
        .publish();

    let bar100_cksum = Package::new("bar", "1.0.0")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat", &["baz"])
        .publish();
    let bar101_cksum = Package::new("bar", "1.0.1")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("feat", &["dep:baz"])
        .publish();
    let bar102_cksum = Package::new("bar", "1.0.2")
        .add_dep(Dependency::new("baz", "1.0").enable_features(&["new-feat"]))
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
        .file("src/lib.rs", "")
        .build();

    let lock_bar_to = |toolchain_version: &Version, bar_version| {
        let lock = if toolchain_version < &Version::new(1, 12, 0) {
            let url = registry.index_url();
            match bar_version {
                100 => format!(
                    r#"
                        [root]
                        name = "foo"
                        version = "0.1.0"
                        dependencies = [
                         "bar 1.0.0 (registry+{url})",
                        ]

                        [[package]]
                        name = "bar"
                        version = "1.0.0"
                        source = "registry+{url}"
                    "#,
                    url = url
                ),
                101 => format!(
                    r#"
                        [root]
                        name = "foo"
                        version = "0.1.0"
                        dependencies = [
                         "bar 1.0.1 (registry+{url})",
                        ]

                        [[package]]
                        name = "bar"
                        version = "1.0.1"
                        source = "registry+{url}"
                    "#,
                    url = url
                ),
                102 => format!(
                    r#"
                        [root]
                        name = "foo"
                        version = "0.1.0"
                        dependencies = [
                         "bar 1.0.2 (registry+{url})",
                        ]

                        [[package]]
                        name = "bar"
                        version = "1.0.2"
                        source = "registry+{url}"
                        dependencies = [
                         "baz 1.0.1 (registry+{url})",
                        ]

                        [[package]]
                        name = "baz"
                        version = "1.0.1"
                        source = "registry+{url}"
                    "#,
                    url = url
                ),
                _ => panic!("unexpected version"),
            }
        } else {
            match bar_version {
                100 => format!(
                    r#"
                        [root]
                        name = "foo"
                        version = "0.1.0"
                        dependencies = [
                         "bar 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
                        ]

                        [[package]]
                        name = "bar"
                        version = "1.0.0"
                        source = "registry+https://github.com/rust-lang/crates.io-index"

                        [metadata]
                        "checksum bar 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)" = "{}"
                    "#,
                    bar100_cksum
                ),
                101 => format!(
                    r#"
                        [root]
                        name = "foo"
                        version = "0.1.0"
                        dependencies = [
                         "bar 1.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                        ]

                        [[package]]
                        name = "bar"
                        version = "1.0.1"
                        source = "registry+https://github.com/rust-lang/crates.io-index"

                        [metadata]
                        "checksum bar 1.0.1 (registry+https://github.com/rust-lang/crates.io-index)" = "{}"
                    "#,
                    bar101_cksum
                ),
                102 => format!(
                    r#"
                        [root]
                        name = "foo"
                        version = "0.1.0"
                        dependencies = [
                         "bar 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)",
                        ]

                        [[package]]
                        name = "bar"
                        version = "1.0.2"
                        source = "registry+https://github.com/rust-lang/crates.io-index"
                        dependencies = [
                         "baz 1.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                        ]

                        [[package]]
                        name = "baz"
                        version = "1.0.1"
                        source = "registry+https://github.com/rust-lang/crates.io-index"

                        [metadata]
                        "checksum bar 1.0.2 (registry+https://github.com/rust-lang/crates.io-index)" = "{bar102_cksum}"
                        "checksum baz 1.0.1 (registry+https://github.com/rust-lang/crates.io-index)" = "{baz101_cksum}"
                    "#,
                    bar102_cksum = bar102_cksum,
                    baz101_cksum = baz101_cksum
                ),
                _ => panic!("unexpected version"),
            }
        };
        p.change_file("Cargo.lock", &lock);
    };

    let toolchains = collect_all_toolchains();

    let config_path = paths::home().join(".cargo/config");
    let lock_path = p.root().join("Cargo.lock");

    struct ToolchainBehavior {
        bar: Option<Version>,
        baz: Option<Version>,
        new_baz_dep: Option<Version>,
    }

    // Collect errors to print at the end. One entry per toolchain, a list of
    // strings to print.
    let mut unexpected_results: Vec<Vec<String>> = Vec::new();

    for (version, toolchain) in &toolchains {
        let mut tc_result = Vec::new();
        // Write a config appropriate for this version.
        if version < &Version::new(1, 12, 0) {
            fs::write(
                &config_path,
                format!(
                    r#"
                        [registry]
                        index = "{}"
                    "#,
                    registry.index_url()
                ),
            )
            .unwrap();
        } else {
            fs::write(
                &config_path,
                format!(
                    "
                        [source.crates-io]
                        registry = 'https://wut'  # only needed by 1.12
                        replace-with = 'dummy-registry'

                        [source.dummy-registry]
                        registry = '{}'
                    ",
                    registry.index_url()
                ),
            )
            .unwrap();
        }

        // Fetches the version of a package in the lock file.
        let pkg_version = |pkg| -> Option<Version> {
            let output = tc_process("cargo", toolchain)
                .args(&["pkgid", pkg])
                .cwd(p.root())
                .exec_with_output()
                .ok()?;
            let stdout = std::str::from_utf8(&output.stdout).unwrap();
            let version = stdout
                .trim()
                .rsplitn(2, ':')
                .next()
                .expect("version after colon");
            Some(Version::parse(version).expect("parseable version"))
        };

        // Runs `cargo build` and returns the versions selected in the lock.
        let run_cargo = || -> CargoResult<ToolchainBehavior> {
            match tc_process("cargo", toolchain)
                .args(&["build", "--verbose"])
                .cwd(p.root())
                .exec_with_output()
            {
                Ok(_output) => {
                    eprintln!("{} ok", toolchain);
                    let bar = pkg_version("bar");
                    let baz = pkg_version("baz");
                    let new_baz_dep = pkg_version("new-baz-dep");
                    Ok(ToolchainBehavior {
                        bar,
                        baz,
                        new_baz_dep,
                    })
                }
                Err(e) => {
                    eprintln!("{} err {}", toolchain, e);
                    Err(e)
                }
            }
        };

        macro_rules! check_lock {
            ($tc_result:ident, $pkg:expr, $which:expr, $actual:expr, None) => {
                check_lock!(= $tc_result, $pkg, $which, $actual, None);
            };
            ($tc_result:ident, $pkg:expr, $which:expr, $actual:expr, $expected:expr) => {
                check_lock!(= $tc_result, $pkg, $which, $actual, Some(Version::parse($expected).unwrap()));
            };
            (= $tc_result:ident, $pkg:expr, $which:expr, $actual:expr, $expected:expr) => {
                let exp: Option<Version> = $expected;
                if $actual != $expected {
                    $tc_result.push(format!(
                        "{} for {} saw {:?} but expected {:?}",
                        $which, $pkg, $actual, exp
                    ));
                }
            };
        }

        let check_err_contains = |tc_result: &mut Vec<_>, err: anyhow::Error, contents| {
            if let Some(ProcessError {
                stderr: Some(stderr),
                ..
            }) = err.downcast_ref::<ProcessError>()
            {
                let stderr = std::str::from_utf8(stderr).unwrap();
                if !stderr.contains(contents) {
                    tc_result.push(format!(
                        "{} expected to see error contents:\n{}\nbut saw:\n{}",
                        toolchain, contents, stderr
                    ));
                }
            } else {
                panic!("{} unexpected error {}", toolchain, err);
            }
        };

        // Unlocked behavior.
        let which = "unlocked";
        lock_path.rm_rf();
        p.build_dir().rm_rf();
        match run_cargo() {
            Ok(behavior) => {
                if version < &Version::new(1, 51, 0) {
                    check_lock!(tc_result, "bar", which, behavior.bar, "1.0.2");
                    check_lock!(tc_result, "baz", which, behavior.baz, "1.0.1");
                    check_lock!(tc_result, "new-baz-dep", which, behavior.new_baz_dep, None);
                } else if version >= &Version::new(1, 51, 0) && version <= &Version::new(1, 59, 0) {
                    check_lock!(tc_result, "bar", which, behavior.bar, "1.0.0");
                    check_lock!(tc_result, "baz", which, behavior.baz, None);
                    check_lock!(tc_result, "new-baz-dep", which, behavior.new_baz_dep, None);
                }
                // Starting with 1.60, namespaced-features has been stabilized.
                else {
                    check_lock!(tc_result, "bar", which, behavior.bar, "1.0.2");
                    check_lock!(tc_result, "baz", which, behavior.baz, "1.0.1");
                    check_lock!(
                        tc_result,
                        "new-baz-dep",
                        which,
                        behavior.new_baz_dep,
                        "1.0.0"
                    );
                }
            }
            Err(e) => {
                tc_result.push(format!("unlocked build failed: {}", e));
            }
        }

        let which = "locked bar 1.0.0";
        lock_bar_to(version, 100);
        match run_cargo() {
            Ok(behavior) => {
                check_lock!(tc_result, "bar", which, behavior.bar, "1.0.0");
                check_lock!(tc_result, "baz", which, behavior.baz, None);
                check_lock!(tc_result, "new-baz-dep", which, behavior.new_baz_dep, None);
            }
            Err(e) => {
                tc_result.push(format!("bar 1.0.0 locked build failed: {}", e));
            }
        }

        let which = "locked bar 1.0.1";
        lock_bar_to(version, 101);
        match run_cargo() {
            Ok(behavior) => {
                check_lock!(tc_result, "bar", which, behavior.bar, "1.0.1");
                check_lock!(tc_result, "baz", which, behavior.baz, None);
                check_lock!(tc_result, "new-baz-dep", which, behavior.new_baz_dep, None);
            }
            Err(e) => {
                // When version >= 1.51 and <= 1.59,
                // 1.0.1 can't be used without -Znamespaced-features
                // It gets filtered out of the index.
                check_err_contains(
                    &mut tc_result,
                    e,
                    "candidate versions found which didn't match: 1.0.2, 1.0.0",
                );
            }
        }

        let which = "locked bar 1.0.2";
        lock_bar_to(version, 102);
        match run_cargo() {
            Ok(behavior) => {
                if version <= &Version::new(1, 59, 0) {
                    check_lock!(tc_result, "bar", which, behavior.bar, "1.0.2");
                    check_lock!(tc_result, "baz", which, behavior.baz, "1.0.1");
                    check_lock!(tc_result, "new-baz-dep", which, behavior.new_baz_dep, None);
                }
                // Starting with 1.60, namespaced-features has been stabilized.
                else {
                    check_lock!(tc_result, "bar", which, behavior.bar, "1.0.2");
                    check_lock!(tc_result, "baz", which, behavior.baz, "1.0.1");
                    check_lock!(
                        tc_result,
                        "new-baz-dep",
                        which,
                        behavior.new_baz_dep,
                        "1.0.0"
                    );
                }
            }
            Err(e) => {
                // When version >= 1.51 and <= 1.59,
                // baz can't lock to 1.0.1, it requires -Znamespaced-features
                check_err_contains(
                    &mut tc_result,
                    e,
                    "candidate versions found which didn't match: 1.0.0",
                );
            }
        }

        unexpected_results.push(tc_result);
    }

    // Generate a report.
    let mut has_err = false;
    for ((tc_vers, tc_name), errs) in toolchains.iter().zip(unexpected_results) {
        if errs.is_empty() {
            continue;
        }
        eprintln!("error: toolchain {} (version {}):", tc_name, tc_vers);
        for err in errs {
            eprintln!("  {}", err);
        }
        has_err = true;
    }
    if has_err {
        panic!("at least one toolchain did not run as expected");
    }
}

#[cargo_test]
#[ignore = "must be run manually, requires old cargo installations"]
fn index_cache_rebuild() {
    // Checks that the index cache gets rebuilt.
    //
    // 1.48 will not cache entries with features with the same name as a
    // dependency. If the cache does not get rebuilt, then running with
    // `-Znamespaced-features` would prevent the new cargo from seeing those
    // entries. The index cache version was changed to prevent this from
    // happening, and switching between versions should work correctly
    // (although it will thrash the cash, that's better than not working
    // correctly.
    Package::new("baz", "1.0.0").publish();
    Package::new("bar", "1.0.0").publish();
    Package::new("bar", "1.0.1")
        .add_dep(Dependency::new("baz", "1.0").optional(true))
        .feature("baz", &["dep:baz"])
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
        .file("src/lib.rs", "")
        .build();

    // This version of Cargo errors on index entries that have overlapping
    // feature names, so 1.0.1 will be missing.
    execs()
        .with_process_builder(tc_process("cargo", "1.48.0"))
        .arg("check")
        .cwd(p.root())
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 [..]
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    fs::remove_file(p.root().join("Cargo.lock")).unwrap();

    // This should rebuild the cache and use 1.0.1.
    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.1 [..]
[CHECKING] bar v1.0.1
[CHECKING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    fs::remove_file(p.root().join("Cargo.lock")).unwrap();

    // Verify 1.48 can still resolve, and is at 1.0.0.
    execs()
        .with_process_builder(tc_process("cargo", "1.48.0"))
        .arg("tree")
        .cwd(p.root())
        .with_stdout(
            "\
foo v0.1.0 [..]
└── bar v1.0.0
",
        )
        .run();
}

#[cargo_test]
#[ignore = "must be run manually, requires old cargo installations"]
fn avoids_split_debuginfo_collision() {
    // Test needs two different toolchains.
    // If the default toolchain is stable, then it won't work.
    if default_toolchain_is_stable() {
        return;
    }
    // Checks for a bug where .o files were being incorrectly shared between
    // different toolchains using incremental and split-debuginfo on macOS.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [profile.dev]
                split-debuginfo = "unpacked"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    execs()
        .with_process_builder(tc_process("cargo", "stable"))
        .arg("build")
        .env("CARGO_INCREMENTAL", "1")
        .cwd(p.root())
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("build")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();

    execs()
        .with_process_builder(tc_process("cargo", "stable"))
        .arg("build")
        .env("CARGO_INCREMENTAL", "1")
        .cwd(p.root())
        .with_stderr(
            "\
[FINISHED] [..]
",
        )
        .run();
}
