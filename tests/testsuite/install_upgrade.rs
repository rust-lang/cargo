//! Tests for `cargo install` where it upgrades a package if it is out-of-date.

use cargo::core::PackageId;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cargo_test_support::install::{cargo_home, exe};
use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::{self, Package};
use cargo_test_support::{
    basic_manifest, cargo_process, cross_compile, execs, git, process, project, Execs,
};

fn pkg_maybe_yanked(name: &str, vers: &str, yanked: bool) {
    Package::new(name, vers)
        .yanked(yanked)
        .file(
            "src/main.rs",
            r#"fn main() { println!("{}", env!("CARGO_PKG_VERSION")) }"#,
        )
        .publish();
}

// Helper for publishing a package.
fn pkg(name: &str, vers: &str) {
    pkg_maybe_yanked(name, vers, false)
}

fn v1_path() -> PathBuf {
    cargo_home().join(".crates.toml")
}

fn v2_path() -> PathBuf {
    cargo_home().join(".crates2.json")
}

fn load_crates1() -> toml::Value {
    toml::from_str(&fs::read_to_string(v1_path()).unwrap()).unwrap()
}

fn load_crates2() -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(v2_path()).unwrap()).unwrap()
}

fn installed_exe(name: &str) -> PathBuf {
    cargo_home().join("bin").join(exe(name))
}

/// Helper for executing binaries installed by cargo.
fn installed_process(name: &str) -> Execs {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    thread_local!(static UNIQUE_ID: usize = NEXT_ID.fetch_add(1, Ordering::SeqCst));

    // This copies the executable to a unique name so that it may be safely
    // replaced on Windows.  See Project::rename_run for details.
    let src = installed_exe(name);
    let dst = installed_exe(&UNIQUE_ID.with(|my_id| format!("{}-{}", name, my_id)));
    // Note: Cannot use copy. On Linux, file descriptors may be left open to
    // the executable as other tests in other threads are constantly spawning
    // new processes (see https://github.com/rust-lang/cargo/pull/5557 for
    // more).
    fs::rename(&src, &dst)
        .unwrap_or_else(|e| panic!("Failed to rename `{:?}` to `{:?}`: {}", src, dst, e));
    // Leave behind a fake file so that reinstall duplicate check works.
    fs::write(src, "").unwrap();
    let p = process(dst);
    execs().with_process_builder(p)
}

/// Check that the given package name/version has the following bins listed in
/// the trackers. Also verifies that both trackers are in sync and valid.
/// Pass in an empty `bins` list to assert that the package is *not* installed.
fn validate_trackers(name: &str, version: &str, bins: &[&str]) {
    let v1 = load_crates1();
    let v1_table = v1.get("v1").unwrap().as_table().unwrap();
    let v2 = load_crates2();
    let v2_table = v2["installs"].as_object().unwrap();
    assert_eq!(v1_table.len(), v2_table.len());
    // Convert `bins` to a BTreeSet.
    let bins: BTreeSet<String> = bins
        .iter()
        .map(|b| format!("{}{}", b, env::consts::EXE_SUFFIX))
        .collect();
    // Check every entry matches between v1 and v2.
    for (pkg_id_str, v1_bins) in v1_table {
        let pkg_id: PackageId = toml::Value::from(pkg_id_str.to_string())
            .try_into()
            .unwrap();
        let v1_bins: BTreeSet<String> = v1_bins
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b.as_str().unwrap().to_string())
            .collect();
        if pkg_id.name().as_str() == name && pkg_id.version().to_string() == version {
            if bins.is_empty() {
                panic!(
                    "Expected {} to not be installed, but found: {:?}",
                    name, v1_bins
                );
            } else {
                assert_eq!(bins, v1_bins);
            }
        }
        let pkg_id_value = serde_json::to_value(&pkg_id).unwrap();
        let pkg_id_str = pkg_id_value.as_str().unwrap();
        let v2_info = v2_table
            .get(pkg_id_str)
            .expect("v2 missing v1 pkg")
            .as_object()
            .unwrap();
        let v2_bins = v2_info["bins"].as_array().unwrap();
        let v2_bins: BTreeSet<String> = v2_bins
            .iter()
            .map(|b| b.as_str().unwrap().to_string())
            .collect();
        assert_eq!(v1_bins, v2_bins);
    }
}

#[cargo_test]
fn registry_upgrade() {
    // Installing and upgrading from a registry.
    pkg("foo", "1.0.0");
    cargo_process("install foo")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v1.0.0 (registry [..])
[INSTALLING] foo v1.0.0
[COMPILING] foo v1.0.0
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [CWD]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v1.0.0` (executable `foo[EXE]`)
[WARNING] be sure to add [..]
",
        )
        .run();
    installed_process("foo").with_stdout("1.0.0").run();
    validate_trackers("foo", "1.0.0", &["foo"]);

    cargo_process("install foo")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[IGNORED] package `foo v1.0.0` is already installed[..]
[WARNING] be sure to add [..]
",
        )
        .run();

    pkg("foo", "1.0.1");

    cargo_process("install foo")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v1.0.1 (registry [..])
[INSTALLING] foo v1.0.1
[COMPILING] foo v1.0.1
[FINISHED] release [optimized] target(s) in [..]
[REPLACING] [CWD]/home/.cargo/bin/foo[EXE]
[REPLACED] package `foo v1.0.0` with `foo v1.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add [..]
",
        )
        .run();

    installed_process("foo").with_stdout("1.0.1").run();
    validate_trackers("foo", "1.0.1", &["foo"]);

    cargo_process("install foo --version=1.0.0")
        .with_stderr_contains("[COMPILING] foo v1.0.0")
        .run();
    installed_process("foo").with_stdout("1.0.0").run();
    validate_trackers("foo", "1.0.0", &["foo"]);

    cargo_process("install foo --version=^1.0")
        .with_stderr_contains("[COMPILING] foo v1.0.1")
        .run();
    installed_process("foo").with_stdout("1.0.1").run();
    validate_trackers("foo", "1.0.1", &["foo"]);

    cargo_process("install foo --version=^1.0")
        .with_stderr_contains("[IGNORED] package `foo v1.0.1` is already installed[..]")
        .run();
}

#[cargo_test]
fn uninstall() {
    // Basic uninstall test.
    pkg("foo", "1.0.0");
    cargo_process("install foo").run();
    cargo_process("uninstall foo").run();
    let data = load_crates2();
    assert_eq!(data["installs"].as_object().unwrap().len(), 0);
    let v1_table = load_crates1();
    assert_eq!(v1_table.get("v1").unwrap().as_table().unwrap().len(), 0);
}

#[cargo_test]
fn upgrade_force() {
    pkg("foo", "1.0.0");
    cargo_process("install foo").run();
    cargo_process("install foo --force")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[INSTALLING] foo v1.0.0
[COMPILING] foo v1.0.0
[FINISHED] release [optimized] target(s) in [..]
[REPLACING] [..]/.cargo/bin/foo[EXE]
[REPLACED] package `foo v1.0.0` with `foo v1.0.0` (executable `foo[EXE]`)
[WARNING] be sure to add `[..]/.cargo/bin` to your PATH [..]
",
        )
        .run();
    validate_trackers("foo", "1.0.0", &["foo"]);
}

#[cargo_test]
fn ambiguous_version_no_longer_allowed() {
    // Non-semver-requirement is not allowed for `--version`.
    pkg("foo", "1.0.0");
    cargo_process("install foo --version=1.0")
        .with_stderr(
            "\
[ERROR] invalid value '1.0' for '--version <VERSION>': cannot parse '1.0' as a SemVer version

  tip: if you want to specify SemVer range, add an explicit qualifier, like '^1.0'

For more information, try '--help'.
",
        )
        .with_status(1)
        .run();
}

#[cargo_test]
fn path_is_always_dirty() {
    // --path should always reinstall.
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("install --path .").run();
    p.cargo("install --path .")
        .with_stderr_contains("[REPLACING] [..]/foo[EXE]")
        .run();
}

#[cargo_test]
fn fails_for_conflicts_unknown() {
    // If an untracked file is in the way, it should fail.
    pkg("foo", "1.0.0");
    let exe = installed_exe("foo");
    exe.parent().unwrap().mkdir_p();
    fs::write(exe, "").unwrap();
    cargo_process("install foo")
        .with_stderr_contains("[ERROR] binary `foo[EXE]` already exists in destination")
        .with_status(101)
        .run();
}

#[cargo_test]
fn fails_for_conflicts_known() {
    // If the same binary exists in another package, it should fail.
    pkg("foo", "1.0.0");
    Package::new("bar", "1.0.0")
        .file("src/bin/foo.rs", "fn main() {}")
        .publish();
    cargo_process("install foo").run();
    cargo_process("install bar")
        .with_stderr_contains(
            "[ERROR] binary `foo[EXE]` already exists in destination as part of `foo v1.0.0`",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn supports_multiple_binary_names() {
    // Can individually install with --bin or --example
    Package::new("foo", "1.0.0")
        .file("src/main.rs", r#"fn main() { println!("foo"); }"#)
        .file("src/bin/a.rs", r#"fn main() { println!("a"); }"#)
        .file("examples/ex1.rs", r#"fn main() { println!("ex1"); }"#)
        .publish();
    cargo_process("install foo --bin foo").run();
    installed_process("foo").with_stdout("foo").run();
    assert!(!installed_exe("a").exists());
    assert!(!installed_exe("ex1").exists());
    validate_trackers("foo", "1.0.0", &["foo"]);
    cargo_process("install foo --bin a").run();
    installed_process("a").with_stdout("a").run();
    assert!(!installed_exe("ex1").exists());
    validate_trackers("foo", "1.0.0", &["a", "foo"]);
    cargo_process("install foo --example ex1").run();
    installed_process("ex1").with_stdout("ex1").run();
    validate_trackers("foo", "1.0.0", &["a", "ex1", "foo"]);
    cargo_process("uninstall foo --bin foo").run();
    assert!(!installed_exe("foo").exists());
    assert!(installed_exe("ex1").exists());
    validate_trackers("foo", "1.0.0", &["a", "ex1"]);
    cargo_process("uninstall foo").run();
    assert!(!installed_exe("ex1").exists());
    assert!(!installed_exe("a").exists());
}

#[cargo_test]
fn v1_already_installed_fresh() {
    // Install with v1, then try to install again with v2.
    pkg("foo", "1.0.0");
    cargo_process("install foo").run();
    cargo_process("install foo")
        .with_stderr_contains("[IGNORED] package `foo v1.0.0` is already installed[..]")
        .run();
}

#[cargo_test]
fn v1_already_installed_dirty() {
    // Install with v1, then install a new version with v2.
    pkg("foo", "1.0.0");
    cargo_process("install foo").run();
    pkg("foo", "1.0.1");
    cargo_process("install foo")
        .with_stderr_contains("[COMPILING] foo v1.0.1")
        .with_stderr_contains("[REPLACING] [..]/foo[EXE]")
        .run();
    validate_trackers("foo", "1.0.1", &["foo"]);
}

#[cargo_test]
fn change_features_rebuilds() {
    Package::new("foo", "1.0.0")
        .file(
            "src/main.rs",
            r#"
            fn main() {
                if cfg!(feature = "f1") {
                    println!("f1");
                }
                if cfg!(feature = "f2") {
                    println!("f2");
                }
            }
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"

            [features]
            f1 = []
            f2 = []
            default = ["f1"]
            "#,
        )
        .publish();
    cargo_process("install foo").run();
    installed_process("foo").with_stdout("f1").run();
    cargo_process("install foo --no-default-features").run();
    installed_process("foo").with_stdout("").run();
    cargo_process("install foo --all-features").run();
    installed_process("foo").with_stdout("f1\nf2").run();
    cargo_process("install foo --no-default-features --features=f1").run();
    installed_process("foo").with_stdout("f1").run();
}

#[cargo_test]
fn change_profile_rebuilds() {
    pkg("foo", "1.0.0");
    cargo_process("install foo").run();
    cargo_process("install foo --debug")
        .with_stderr_contains("[COMPILING] foo v1.0.0")
        .with_stderr_contains("[REPLACING] [..]foo[EXE]")
        .run();
    cargo_process("install foo --debug")
        .with_stderr_contains("[IGNORED] package `foo v1.0.0` is already installed[..]")
        .run();
}

#[cargo_test]
fn change_target_rebuilds() {
    if cross_compile::disabled() {
        return;
    }
    pkg("foo", "1.0.0");
    cargo_process("install foo").run();
    let target = cross_compile::alternate();
    cargo_process("install foo -v --target")
        .arg(&target)
        .with_stderr_contains("[COMPILING] foo v1.0.0")
        .with_stderr_contains("[REPLACING] [..]foo[EXE]")
        .with_stderr_contains(&format!("[..]--target {}[..]", target))
        .run();
}

#[cargo_test]
fn change_bin_sets_rebuilds() {
    // Changing which bins in a multi-bin project should reinstall.
    Package::new("foo", "1.0.0")
        .file("src/main.rs", "fn main() { }")
        .file("src/bin/x.rs", "fn main() { }")
        .file("src/bin/y.rs", "fn main() { }")
        .publish();
    cargo_process("install foo --bin x").run();
    assert!(installed_exe("x").exists());
    assert!(!installed_exe("y").exists());
    assert!(!installed_exe("foo").exists());
    validate_trackers("foo", "1.0.0", &["x"]);
    cargo_process("install foo --bin y")
        .with_stderr_contains("[INSTALLED] package `foo v1.0.0` (executable `y[EXE]`)")
        .run();
    assert!(installed_exe("x").exists());
    assert!(installed_exe("y").exists());
    assert!(!installed_exe("foo").exists());
    validate_trackers("foo", "1.0.0", &["x", "y"]);
    cargo_process("install foo")
        .with_stderr_contains("[INSTALLED] package `foo v1.0.0` (executable `foo[EXE]`)")
        .with_stderr_contains(
            "[REPLACED] package `foo v1.0.0` with `foo v1.0.0` (executables `x[EXE]`, `y[EXE]`)",
        )
        .run();
    assert!(installed_exe("x").exists());
    assert!(installed_exe("y").exists());
    assert!(installed_exe("foo").exists());
    validate_trackers("foo", "1.0.0", &["foo", "x", "y"]);
}

#[cargo_test]
fn forwards_compatible() {
    // Unknown fields should be preserved.
    pkg("foo", "1.0.0");
    pkg("bar", "1.0.0");
    cargo_process("install foo").run();
    let key = "foo 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)";
    let v2 = cargo_home().join(".crates2.json");
    let mut data = load_crates2();
    data["newfield"] = serde_json::Value::Bool(true);
    data["installs"][key]["moreinfo"] = serde_json::Value::String("shazam".to_string());
    fs::write(&v2, serde_json::to_string(&data).unwrap()).unwrap();
    cargo_process("install bar").run();
    let data: serde_json::Value = serde_json::from_str(&fs::read_to_string(&v2).unwrap()).unwrap();
    assert_eq!(data["newfield"].as_bool().unwrap(), true);
    assert_eq!(
        data["installs"][key]["moreinfo"].as_str().unwrap(),
        "shazam"
    );
}

#[cargo_test]
fn v2_syncs() {
    // V2 inherits the installs from V1.
    pkg("one", "1.0.0");
    pkg("two", "1.0.0");
    pkg("three", "1.0.0");
    let p = project()
        .file("src/bin/x.rs", "fn main() {}")
        .file("src/bin/y.rs", "fn main() {}")
        .build();
    cargo_process("install one").run();
    validate_trackers("one", "1.0.0", &["one"]);
    p.cargo("install --path .").run();
    validate_trackers("foo", "1.0.0", &["x", "y"]);
    // v1 add/remove
    cargo_process("install two").run();
    cargo_process("uninstall one").run();
    // This should pick up that `two` was added, `one` was removed.
    cargo_process("install three").run();
    validate_trackers("three", "1.0.0", &["three"]);
    cargo_process("install --list")
        .with_stdout(
            "\
foo v0.0.1 ([..]/foo):
    x[EXE]
    y[EXE]
three v1.0.0:
    three[EXE]
two v1.0.0:
    two[EXE]
",
        )
        .run();
    cargo_process("install one").run();
    installed_process("one").with_stdout("1.0.0").run();
    validate_trackers("one", "1.0.0", &["one"]);
    cargo_process("install two")
        .with_stderr_contains("[IGNORED] package `two v1.0.0` is already installed[..]")
        .run();
    // v1 remove
    p.cargo("uninstall --bin x").run();
    pkg("x", "1.0.0");
    pkg("y", "1.0.0");
    // This should succeed because `x` was removed in V1.
    cargo_process("install x").run();
    validate_trackers("x", "1.0.0", &["x"]);
    // This should fail because `y` still exists in a different package.
    cargo_process("install y")
        .with_stderr_contains(
            "[ERROR] binary `y[EXE]` already exists in destination \
             as part of `foo v0.0.1 ([..])`",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn upgrade_git() {
    let git_project = git::new("foo", |project| project.file("src/main.rs", "fn main() {}"));
    // install
    cargo_process("install --git")
        .arg(git_project.url().to_string())
        .run();
    // Check install stays fresh.
    cargo_process("install --git")
        .arg(git_project.url().to_string())
        .with_stderr_contains(
            "[IGNORED] package `foo v0.0.1 (file://[..]/foo#[..])` is \
             already installed,[..]",
        )
        .run();
    // Modify a file.
    let repo = git2::Repository::open(git_project.root()).unwrap();
    git_project.change_file("src/main.rs", r#"fn main() {println!("onomatopoeia");}"#);
    git::add(&repo);
    git::commit(&repo);
    // Install should reinstall.
    cargo_process("install --git")
        .arg(git_project.url().to_string())
        .with_stderr_contains("[COMPILING] foo v0.0.1 ([..])")
        .with_stderr_contains("[REPLACING] [..]/foo[EXE]")
        .run();
    installed_process("foo").with_stdout("onomatopoeia").run();
    // Check install stays fresh.
    cargo_process("install --git")
        .arg(git_project.url().to_string())
        .with_stderr_contains(
            "[IGNORED] package `foo v0.0.1 (file://[..]/foo#[..])` is \
             already installed,[..]",
        )
        .run();
}

#[cargo_test]
fn switch_sources() {
    // Installing what appears to be the same thing, but from different
    // sources should reinstall.
    registry::alt_init();
    pkg("foo", "1.0.0");
    Package::new("foo", "1.0.0")
        .file("src/main.rs", r#"fn main() { println!("alt"); }"#)
        .alternative(true)
        .publish();
    let p = project()
        .at("foo-local") // so it doesn't use the same directory as the git project
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", r#"fn main() { println!("local"); }"#)
        .build();
    let git_project = git::new("foo", |project| {
        project.file("src/main.rs", r#"fn main() { println!("git"); }"#)
    });

    cargo_process("install foo").run();
    installed_process("foo").with_stdout("1.0.0").run();
    cargo_process("install foo --registry alternative").run();
    installed_process("foo").with_stdout("alt").run();
    p.cargo("install --path .").run();
    installed_process("foo").with_stdout("local").run();
    cargo_process("install --git")
        .arg(git_project.url().to_string())
        .run();
    installed_process("foo").with_stdout("git").run();
}

#[cargo_test]
fn multiple_report() {
    // Testing the full output that indicates installed/ignored/replaced/summary.
    pkg("one", "1.0.0");
    pkg("two", "1.0.0");
    fn three(vers: &str) {
        Package::new("three", vers)
            .file("src/main.rs", "fn main() { }")
            .file("src/bin/x.rs", "fn main() { }")
            .file("src/bin/y.rs", "fn main() { }")
            .publish();
    }
    three("1.0.0");
    cargo_process("install one two three")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] one v1.0.0 (registry `[..]`)
[DOWNLOADING] crates ...
[DOWNLOADED] two v1.0.0 (registry `[..]`)
[DOWNLOADING] crates ...
[DOWNLOADED] three v1.0.0 (registry `[..]`)
[INSTALLING] one v1.0.0
[COMPILING] one v1.0.0
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [..]/.cargo/bin/one[EXE]
[INSTALLED] package `one v1.0.0` (executable `one[EXE]`)
[INSTALLING] two v1.0.0
[COMPILING] two v1.0.0
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [..]/.cargo/bin/two[EXE]
[INSTALLED] package `two v1.0.0` (executable `two[EXE]`)
[INSTALLING] three v1.0.0
[COMPILING] three v1.0.0
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [..]/.cargo/bin/three[EXE]
[INSTALLING] [..]/.cargo/bin/x[EXE]
[INSTALLING] [..]/.cargo/bin/y[EXE]
[INSTALLED] package `three v1.0.0` (executables `three[EXE]`, `x[EXE]`, `y[EXE]`)
[SUMMARY] Successfully installed one, two, three!
[WARNING] be sure to add `[..]/.cargo/bin` to your PATH [..]
",
        )
        .run();
    pkg("foo", "1.0.1");
    pkg("bar", "1.0.1");
    three("1.0.1");
    cargo_process("install one two three")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[IGNORED] package `one v1.0.0` is already installed, use --force to override
[IGNORED] package `two v1.0.0` is already installed, use --force to override
[DOWNLOADING] crates ...
[DOWNLOADED] three v1.0.1 (registry `[..]`)
[INSTALLING] three v1.0.1
[COMPILING] three v1.0.1
[FINISHED] release [optimized] target(s) in [..]
[REPLACING] [..]/.cargo/bin/three[EXE]
[REPLACING] [..]/.cargo/bin/x[EXE]
[REPLACING] [..]/.cargo/bin/y[EXE]
[REPLACED] package `three v1.0.0` with `three v1.0.1` (executables `three[EXE]`, `x[EXE]`, `y[EXE]`)
[SUMMARY] Successfully installed one, two, three!
[WARNING] be sure to add `[..]/.cargo/bin` to your PATH [..]
",
        )
        .run();
    cargo_process("uninstall three")
        .with_stderr(
            "\
[REMOVING] [..]/.cargo/bin/three[EXE]
[REMOVING] [..]/.cargo/bin/x[EXE]
[REMOVING] [..]/.cargo/bin/y[EXE]
",
        )
        .run();
    cargo_process("install three --bin x")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[INSTALLING] three v1.0.1
[COMPILING] three v1.0.1
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [..]/.cargo/bin/x[EXE]
[INSTALLED] package `three v1.0.1` (executable `x[EXE]`)
[WARNING] be sure to add `[..]/.cargo/bin` to your PATH [..]
",
        )
        .run();
    cargo_process("install three")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[INSTALLING] three v1.0.1
[COMPILING] three v1.0.1
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [..]/.cargo/bin/three[EXE]
[INSTALLING] [..]/.cargo/bin/y[EXE]
[REPLACING] [..]/.cargo/bin/x[EXE]
[INSTALLED] package `three v1.0.1` (executables `three[EXE]`, `y[EXE]`)
[REPLACED] package `three v1.0.1` with `three v1.0.1` (executable `x[EXE]`)
[WARNING] be sure to add `[..]/.cargo/bin` to your PATH [..]
",
        )
        .run();
}

#[cargo_test]
fn no_track() {
    pkg("foo", "1.0.0");
    cargo_process("install --no-track foo").run();
    assert!(!v1_path().exists());
    assert!(!v2_path().exists());
    cargo_process("install --no-track foo")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[ERROR] binary `foo[EXE]` already exists in destination `[..]/.cargo/bin/foo[EXE]`
Add --force to overwrite
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn deletes_orphaned() {
    // When an executable is removed from a project, upgrading should remove it.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/other.rs", "fn main() {}")
        .file("examples/ex1.rs", "fn main() {}")
        .build();
    p.cargo("install --path . --bins --examples").run();
    assert!(installed_exe("other").exists());

    // Remove a binary, add a new one, and bump the version.
    fs::remove_file(p.root().join("src/bin/other.rs")).unwrap();
    p.change_file("examples/ex2.rs", "fn main() {}");
    p.change_file(
        "Cargo.toml",
        r#"
        [package]
        name = "foo"
        version = "0.2.0"
        "#,
    );
    p.cargo("install --path . --bins --examples")
        .with_stderr(
            "\
[INSTALLING] foo v0.2.0 [..]
[COMPILING] foo v0.2.0 [..]
[FINISHED] release [..]
[INSTALLING] [..]/.cargo/bin/ex2[EXE]
[REPLACING] [..]/.cargo/bin/ex1[EXE]
[REPLACING] [..]/.cargo/bin/foo[EXE]
[REMOVING] executable `[..]/.cargo/bin/other[EXE]` from previous version foo v0.1.0 [..]
[INSTALLED] package `foo v0.2.0 [..]` (executable `ex2[EXE]`)
[REPLACED] package `foo v0.1.0 [..]` with `foo v0.2.0 [..]` (executables `ex1[EXE]`, `foo[EXE]`)
[WARNING] be sure to add [..]
",
        )
        .run();
    assert!(!installed_exe("other").exists());
    validate_trackers("foo", "0.2.0", &["foo", "ex1", "ex2"]);
    // 0.1.0 should not have any entries.
    validate_trackers("foo", "0.1.0", &[]);
}

#[cargo_test]
fn already_installed_exact_does_not_update() {
    pkg("foo", "1.0.0");
    cargo_process("install foo  --version=1.0.0").run();
    cargo_process("install foo --version=1.0.0")
        .with_stderr(
            "\
[IGNORED] package `foo v1.0.0` is already installed[..]
[WARNING] be sure to add [..]
",
        )
        .run();

    cargo_process("install foo --version=>=1.0.0")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[IGNORED] package `foo v1.0.0` is already installed[..]
[WARNING] be sure to add [..]
",
        )
        .run();
    pkg("foo", "1.0.1");
    cargo_process("install foo --version=>=1.0.0")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v1.0.1 (registry [..])
[INSTALLING] foo v1.0.1
[COMPILING] foo v1.0.1
[FINISHED] release [optimized] target(s) in [..]
[REPLACING] [CWD]/home/.cargo/bin/foo[EXE]
[REPLACED] package `foo v1.0.0` with `foo v1.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add [..]
",
        )
        .run();
}

#[cargo_test]
fn already_installed_updates_yank_status_on_upgrade() {
    pkg("foo", "1.0.0");
    pkg_maybe_yanked("foo", "1.0.1", true);
    cargo_process("install foo  --version=1.0.0").run();

    cargo_process("install foo --version=1.0.1")
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] cannot install package `foo`, it has been yanked from registry `crates-io`
",
        )
        .run();

    pkg_maybe_yanked("foo", "1.0.1", false);

    pkg("foo", "1.0.1");
    cargo_process("install foo --version=1.0.1")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v1.0.1 (registry [..])
[INSTALLING] foo v1.0.1
[COMPILING] foo v1.0.1
[FINISHED] release [optimized] target(s) in [..]
[REPLACING] [CWD]/home/.cargo/bin/foo[EXE]
[REPLACED] package `foo v1.0.0` with `foo v1.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add [..]
",
        )
        .run();
}

#[cargo_test]
fn partially_already_installed_does_one_update() {
    pkg("foo", "1.0.0");
    cargo_process("install foo  --version=1.0.0").run();
    pkg("bar", "1.0.0");
    pkg("baz", "1.0.0");
    cargo_process("install foo bar baz --version=1.0.0")
        .with_stderr(
            "\
[IGNORED] package `foo v1.0.0` is already installed[..]
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry [..])
[DOWNLOADING] crates ...
[DOWNLOADED] baz v1.0.0 (registry [..])
[INSTALLING] bar v1.0.0
[COMPILING] bar v1.0.0
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [CWD]/home/.cargo/bin/bar[EXE]
[INSTALLED] package `bar v1.0.0` (executable `bar[EXE]`)
[INSTALLING] baz v1.0.0
[COMPILING] baz v1.0.0
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [CWD]/home/.cargo/bin/baz[EXE]
[INSTALLED] package `baz v1.0.0` (executable `baz[EXE]`)
[SUMMARY] Successfully installed foo, bar, baz!
[WARNING] be sure to add [..]
",
        )
        .run();
}
