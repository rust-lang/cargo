//! Tests for the `cargo install` command.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use std::thread;

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::cross_compile;
use cargo_test_support::git;
use cargo_test_support::registry::{self, Package};
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, project, project_in, symlink_supported, t};
use cargo_util::{ProcessBuilder, ProcessError};

use crate::utils::cross_compile::disabled as cross_compile_disabled;
use cargo_test_support::install::{assert_has_installed_exe, assert_has_not_installed_exe, exe};
use cargo_test_support::paths;

fn pkg(name: &str, vers: &str) {
    Package::new(name, vers)
        .file("src/lib.rs", "")
        .file(
            "src/main.rs",
            &format!("extern crate {}; fn main() {{}}", name),
        )
        .publish();
}

#[cargo_test]
fn simple() {
    pkg("foo", "0.0.1");

    cargo_process("install foo").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]]).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");

    cargo_process("uninstall foo")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn install_the_same_version_twice() {
    pkg("foo", "0.0.1");

    cargo_process("install foo foo")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn toolchain() {
    pkg("foo", "0.0.1");

    cargo_process("install +nightly")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character `+` in package name: `+nightly`
    Use `cargo +nightly install` if you meant to use the `nightly` toolchain.

"#]])
        .run();
}

#[cargo_test]
fn url() {
    pkg("foo", "0.0.1");
    cargo_process("install https://github.com/bar/foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid package name: `https://github.com/bar/foo`
    Use `cargo install --git https://github.com/bar/foo` if you meant to install from a git repository.

"#]])
        .run();
}

#[cargo_test]
fn simple_with_message_format() {
    pkg("foo", "0.0.1");

    cargo_process("install foo --message-format=json")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[..]` to your PATH to be able to run the installed binaries

"#]])
        .with_stdout_data(
            str![[r#"
[
  {
    "executable": null,
    "features": [],
    "filenames": "{...}",
    "fresh": false,
    "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/foo-0.0.1/Cargo.toml",
    "package_id": "registry+https://github.com/rust-lang/crates.io-index#foo@0.0.1",
    "profile": "{...}",
    "reason": "compiler-artifact",
    "target": {
      "crate_types": [
        "lib"
      ],
      "doc": true,
      "doctest": true,
      "edition": "2015",
      "kind": [
        "lib"
      ],
      "name": "foo",
      "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/foo-0.0.1/src/lib.rs",
      "test": true
    }
  },
  {
    "executable": "[..]",
    "features": [],
    "filenames": "{...}",
    "fresh": false,
    "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/foo-0.0.1/Cargo.toml",
    "package_id": "registry+https://github.com/rust-lang/crates.io-index#foo@0.0.1",
    "profile": "{...}",
    "reason": "compiler-artifact",
    "target": {
      "crate_types": [
        "bin"
      ],
      "doc": true,
      "doctest": false,
      "edition": "2015",
      "kind": [
        "bin"
      ],
      "name": "foo",
      "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/foo-0.0.1/src/main.rs",
      "test": true
    }
  },
  {
    "reason": "build-finished",
    "success": true
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn with_index() {
    let registry = registry::init();
    pkg("foo", "0.0.1");

    cargo_process("install foo --index")
        .arg(registry.index_url().as_str())
        .with_stderr_data(str![[r#"
[UPDATING] `[ROOT]/registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `[ROOT]/registry`)
[INSTALLING] foo v0.0.1 (registry `[ROOT]/registry`)
[COMPILING] foo v0.0.1 (registry `[ROOT]/registry`)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 (registry `[ROOT]/registry`)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");

    cargo_process("uninstall foo")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn multiple_pkgs() {
    pkg("foo", "0.0.1");
    pkg("bar", "0.0.2");

    cargo_process("install foo bar baz")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.2 (registry `dummy-registry`)
[ERROR] could not find `baz` in registry `crates-io` with version `*`
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[INSTALLING] bar v0.0.2
[COMPILING] bar v0.0.2
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/bar[EXE]
[INSTALLED] package `bar v0.0.2` (executable `bar[EXE]`)
[SUMMARY] Successfully installed foo, bar! Failed to install baz (see error(s) above).
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries
[ERROR] some crates failed to install

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
    assert_has_installed_exe(paths::cargo_home(), "bar");

    cargo_process("uninstall foo bar")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]
[REMOVING] [ROOT]/home/.cargo/bin/bar[EXE]
[SUMMARY] Successfully uninstalled foo, bar!

"#]])
        .run();

    assert_has_not_installed_exe(paths::cargo_home(), "foo");
    assert_has_not_installed_exe(paths::cargo_home(), "bar");
}

fn path() -> Vec<PathBuf> {
    env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect()
}

#[cargo_test]
fn multiple_pkgs_path_set() {
    // confirm partial failure results in 101 status code and does not have the
    //      '[WARNING] be sure to add `[..]` to your PATH to be able to run the installed binaries'
    //  even if CARGO_HOME/bin is in the PATH
    pkg("foo", "0.0.1");
    pkg("bar", "0.0.2");

    // add CARGO_HOME/bin to path
    let mut path = path();
    path.push(paths::cargo_home().join("bin"));
    let new_path = env::join_paths(path).unwrap();
    cargo_process("install foo bar baz")
        .env("PATH", new_path)
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.2 (registry `dummy-registry`)
[ERROR] could not find `baz` in registry `crates-io` with version `*`
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[INSTALLING] bar v0.0.2
[COMPILING] bar v0.0.2
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/bar[EXE]
[INSTALLED] package `bar v0.0.2` (executable `bar[EXE]`)
[SUMMARY] Successfully installed foo, bar! Failed to install baz (see error(s) above).
[ERROR] some crates failed to install

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
    assert_has_installed_exe(paths::cargo_home(), "bar");

    cargo_process("uninstall foo bar")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]
[REMOVING] [ROOT]/home/.cargo/bin/bar[EXE]
[SUMMARY] Successfully uninstalled foo, bar!

"#]])
        .run();

    assert_has_not_installed_exe(paths::cargo_home(), "foo");
    assert_has_not_installed_exe(paths::cargo_home(), "bar");
}

#[cargo_test]
fn pick_max_version() {
    pkg("foo", "0.1.0");
    pkg("foo", "0.2.0");
    pkg("foo", "0.2.1");
    pkg("foo", "0.2.1-pre.1");
    pkg("foo", "0.3.0-pre.2");

    cargo_process("install foo").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.2.1 (registry `dummy-registry`)
[INSTALLING] foo v0.2.1
[COMPILING] foo v0.2.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.2.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]]).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn installs_beta_version_by_explicit_name_from_git() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.3.0-beta.1"))
        .file("src/main.rs", "fn main() {}")
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .arg("foo")
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn missing() {
    pkg("foo", "0.0.1");
    cargo_process("install bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] could not find `bar` in registry `crates-io` with version `*`

"#]])
        .run();
}

#[cargo_test]
fn missing_current_working_directory() {
    cargo_process("install .")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] To install the binaries for the package in current working directory use `cargo install --path .`. 
Use `cargo build` if you want to simply build the package.

"#]])
        .run();
}

#[cargo_test]
fn bad_version() {
    pkg("foo", "0.0.1");
    cargo_process("install foo --version=0.2.0")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] could not find `foo` in registry `crates-io` with version `=0.2.0`

"#]])
        .run();
}

#[cargo_test]
fn missing_at_symbol_before_version() {
    pkg("foo", "0.0.1");
    cargo_process("install foo=0.2.0")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character `=` in package name: `foo=0.2.0`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)

[HELP] if this is meant to be a package name followed by a version, insert an `@` like `foo@=0.2.0`

"#]])
        .run();
}

#[cargo_test]
fn bad_paths() {
    cargo_process("install")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `[ROOT]` is not a crate root; specify a crate to install from crates.io, or use --path or --git to specify an alternate source

"#]])
        .run();

    cargo_process("install --path .")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `[ROOT]` does not contain a Cargo.toml file. --path must point to a directory containing a Cargo.toml file.

"#]])
        .run();

    let toml = paths::root().join("Cargo.toml");
    fs::write(toml, "").unwrap();
    cargo_process("install --path Cargo.toml")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `[ROOT]/Cargo.toml` is not a directory. --path must point to a directory containing a Cargo.toml file.

"#]])
        .run();

    cargo_process("install --path .")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/Cargo.toml`
...
"#]])
        .run();
}

#[cargo_test]
fn install_location_precedence() {
    pkg("foo", "0.0.1");

    let root = paths::root();
    let t1 = root.join("t1");
    let t2 = root.join("t2");
    let t3 = root.join("t3");
    let t4 = paths::cargo_home();

    fs::create_dir(root.join(".cargo")).unwrap();
    fs::write(
        root.join(".cargo/config.toml"),
        &format!(
            "[install]
             root = '{}'
            ",
            t3.display()
        ),
    )
    .unwrap();

    println!("install --root");

    cargo_process("install foo --root")
        .arg(&t1)
        .env("CARGO_INSTALL_ROOT", &t2)
        .run();
    assert_has_installed_exe(&t1, "foo");
    assert_has_not_installed_exe(&t2, "foo");

    println!("install CARGO_INSTALL_ROOT");

    cargo_process("install foo")
        .env("CARGO_INSTALL_ROOT", &t2)
        .run();
    assert_has_installed_exe(&t2, "foo");
    assert_has_not_installed_exe(&t3, "foo");

    println!("install install.root");

    cargo_process("install foo").run();
    assert_has_installed_exe(&t3, "foo");
    assert_has_not_installed_exe(&t4, "foo");

    fs::remove_file(root.join(".cargo/config.toml")).unwrap();

    println!("install cargo home");

    cargo_process("install foo").run();
    assert_has_installed_exe(&t4, "foo");
}

#[cargo_test]
fn relative_install_location_without_trailing_slash() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let root = paths::root();
    let root_t1 = root.join("t1");
    let p_path = p.root().to_path_buf();
    let project_t1 = p_path.join("t1");

    fs::create_dir(root.join(".cargo")).unwrap();
    fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [install]
            root = "t1"
        "#,
    )
    .unwrap();

    let mut cmd = cargo_process("install --path .");
    cmd.cwd(p.root());
    cmd.with_stderr_data(str![[r#"
[WARNING] the `install.root` value `t1` defined in [ROOT]/.cargo/config.toml without a trailing slash is deprecated
  |
  = [NOTE] a future version of Cargo will treat it as relative to the configuration directory
  = [HELP] add a trailing slash (`t1/`) to adopt the correct behavior and silence this warning
  = [NOTE] see more at https://doc.rust-lang.org/cargo/reference/config.html#config-relative-paths
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/foo/t1/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/foo/t1/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();

    // NOTE: the install location is relative to the CWD, not the config file
    assert_has_not_installed_exe(&root_t1, "foo");
    assert_has_installed_exe(&project_t1, "foo");
}

#[cargo_test]
fn cli_root_argument_without_deprecation_warning() {
    // Verify that using the --root CLI argument does not produce the deprecation warning.
    let p = project().file("src/main.rs", "fn main() {}").build();

    let root = paths::root();
    let root_t1 = root.join("t1");
    let p_path = p.root().to_path_buf();
    let project_t1 = p_path.join("t1");

    cargo_process("install --path . --root")
        .arg("t1")
        .cwd(p.root())
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/foo/t1/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/foo/t1/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_not_installed_exe(&root_t1, "foo");
    assert_has_installed_exe(&project_t1, "foo");
}

#[cargo_test]
fn relative_install_location_with_trailing_slash() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let root = paths::root();
    let root_t1 = root.join("t1");
    let p_path = p.root().to_path_buf();
    let project_t1 = p_path.join("t1");

    fs::create_dir(root.join(".cargo")).unwrap();
    fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [install]
            root = "t1/"
        "#,
    )
    .unwrap();

    let mut cmd = cargo_process("install --path .");
    cmd.cwd(p.root());
    cmd.with_stderr_data(str![[r#"
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/t1/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/t1/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();

    assert_has_installed_exe(&root_t1, "foo");
    assert_has_not_installed_exe(&project_t1, "foo");
}

#[cargo_test]
fn relative_install_location_with_path_set() {
    // Test that when the absolute install path is in PATH, no warning is shown
    let p = project().file("src/main.rs", "fn main() {}").build();

    let root = paths::root();
    let p_path = p.root().to_path_buf();
    let project_t1 = p_path.join("t1");

    fs::create_dir(root.join(".cargo")).unwrap();
    fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [install]
            root = "t1"
        "#,
    )
    .unwrap();

    // Add the absolute path to PATH environment variable
    let install_bin_path = project_t1.join("bin");
    let mut path = path();
    path.push(install_bin_path);
    let new_path = env::join_paths(path).unwrap();

    let mut cmd = cargo_process("install --path .");
    cmd.cwd(p.root());
    cmd.env("PATH", new_path);
    cmd.with_stderr_data(str![[r#"
[WARNING] the `install.root` value `t1` defined in [ROOT]/.cargo/config.toml without a trailing slash is deprecated
  |
  = [NOTE] a future version of Cargo will treat it as relative to the configuration directory
  = [HELP] add a trailing slash (`t1/`) to adopt the correct behavior and silence this warning
  = [NOTE] see more at https://doc.rust-lang.org/cargo/reference/config.html#config-relative-paths
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/foo/t1/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `foo[EXE]`)

"#]])
        .run();

    assert_has_installed_exe(&project_t1, "foo");
}

#[cargo_test]
fn install_path() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    cargo_process("install --path").arg(p.root()).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
    // path-style installs force a reinstall
    p.cargo("install --path .").with_stderr_data(str![[r#"
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[REPLACING] [ROOT]/home/.cargo/bin/foo[EXE]
[REPLACED] package `foo v0.0.1 ([ROOT]/foo)` with `foo v0.0.1 ([ROOT]/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]]).run();
}

#[cargo_test]
fn install_target_dir() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("install --target-dir td_test")
        .with_stderr_data(str![[r#"
[WARNING] Using `cargo install` to install the binaries from the package in current working directory is deprecated, use `cargo install --path .` instead. Use `cargo build` if you want to simply build the package.
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();

    let mut path = p.root();
    path.push("td_test");
    assert!(path.exists());

    #[cfg(not(windows))]
    path.push("release/foo");
    #[cfg(windows)]
    path.push("release/foo.exe");
    assert!(path.exists());
}

#[cargo_test]
#[cfg(target_os = "linux")]
fn install_path_with_lowercase_cargo_toml() {
    let toml = paths::root().join("cargo.toml");
    fs::write(toml, "").unwrap();

    cargo_process("install --path .")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `[ROOT]` does not contain a Cargo.toml file, but found cargo.toml please try to rename it to Cargo.toml. --path must point to a directory containing a Cargo.toml file.

"#]]
        )
        .run();
}

#[cargo_test]
fn install_relative_path_outside_current_ws() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                authors = []
                edition = "2021"

                [dependencies]
                foo = "1"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    let _bin_project = project_in("bar")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("install --path ../bar/foo")
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.0.1 ([ROOT]/bar/foo)
[COMPILING] foo v0.0.1 ([ROOT]/bar/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/bar/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();

    // Validate the workspace error message to display available targets.
    p.cargo("install --path ../bar/foo --bin")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    foo


"#]])
        .run();
}

#[cargo_test]
fn multiple_packages_containing_binaries() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("a/src/main.rs", "fn main() {}")
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/foo`
[ERROR] multiple packages with binaries found: bar, foo. When installing a git repository, cargo will always search the entire repo for any Cargo.toml.
Please specify a package, e.g. `cargo install --git [ROOTURL]/foo bar`.

"#]])
        .run();
}

#[cargo_test]
fn multiple_packages_matching_example() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .file("bar/examples/ex1.rs", "fn main() {}")
        .build();

    cargo_process("install --example ex1 --git")
        .arg(p.url().to_string())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/foo`
[ERROR] multiple packages with examples found: bar, foo. When installing a git repository, cargo will always search the entire repo for any Cargo.toml.
Please specify a package, e.g. `cargo install --git [ROOTURL]/foo bar`.

"#]])
        .run();
}

#[cargo_test]
fn multiple_binaries_deep_select_uses_package_name() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .file("bar/baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("bar/baz/src/main.rs", "fn main() {}")
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .arg("baz")
        .run();
}

#[cargo_test]
fn multiple_binaries_in_selected_package_installs_all() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/bin/bin1.rs", "fn main() {}")
        .file("bar/src/bin/bin2.rs", "fn main() {}")
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .arg("bar")
        .run();

    let cargo_home = paths::cargo_home();
    assert_has_installed_exe(&cargo_home, "bin1");
    assert_has_installed_exe(&cargo_home, "bin2");
}

#[cargo_test]
fn multiple_binaries_in_selected_package_with_bin_option_installs_only_one() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/bin/bin1.rs", "fn main() {}")
        .file("bar/src/bin/bin2.rs", "fn main() {}")
        .build();

    cargo_process("install --bin bin1 --git")
        .arg(p.url().to_string())
        .arg("bar")
        .run();

    let cargo_home = paths::cargo_home();
    assert_has_installed_exe(&cargo_home, "bin1");
    assert_has_not_installed_exe(&cargo_home, "bin2");
}

#[cargo_test]
fn multiple_crates_select() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("a/src/main.rs", "fn main() {}")
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .arg("foo")
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
    assert_has_not_installed_exe(paths::cargo_home(), "bar");

    cargo_process("install --git")
        .arg(p.url().to_string())
        .arg("bar")
        .run();
    assert_has_installed_exe(paths::cargo_home(), "bar");
}

#[cargo_test]
fn multiple_crates_git_all() {
    let p = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bin1", "bin2"]
            "#,
        )
        .file("bin1/Cargo.toml", &basic_manifest("bin1", "0.1.0"))
        .file("bin2/Cargo.toml", &basic_manifest("bin2", "0.1.0"))
        .file(
            "bin1/src/main.rs",
            r#"fn main() { println!("Hello, world!"); }"#,
        )
        .file(
            "bin2/src/main.rs",
            r#"fn main() { println!("Hello, world!"); }"#,
        )
        .build();

    cargo_process(&format!("install --git {} bin1 bin2", p.url())).run();
}

#[cargo_test]
fn multiple_crates_auto_binaries() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "a" }
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("a/src/lib.rs", "")
        .build();

    cargo_process("install --path").arg(p.root()).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn multiple_crates_auto_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file(
            "examples/foo.rs",
            "
            extern crate bar;
            extern crate foo;
            fn main() {}
        ",
        )
        .file("a/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("a/src/lib.rs", "")
        .build();

    cargo_process("install --path")
        .arg(p.root())
        .arg("--example=foo")
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn no_binaries_or_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("a/src/lib.rs", "")
        .build();

    cargo_process("install --path")
        .arg(p.root())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no packages found with binaries or examples

"#]])
        .run();
}

#[cargo_test]
fn no_binaries() {
    let p = project()
        .file("src/lib.rs", "")
        .file("examples/foo.rs", "fn main() {}")
        .build();

    cargo_process("install --path")
        .arg(p.root())
        .arg("foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] there is nothing to install in `foo v0.0.1 ([ROOT]/foo)`, because it has no binaries
`cargo install` is only for installing programs, and can't be used with libraries.
To use a library crate, add it as a dependency to a Cargo project with `cargo add`.

"#]])
        .run();
}

#[cargo_test]
fn examples() {
    let p = project()
        .file("src/lib.rs", "")
        .file("examples/foo.rs", "extern crate foo; fn main() {}")
        .build();

    cargo_process("install --path")
        .arg(p.root())
        .arg("--example=foo")
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn install_force() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    cargo_process("install --path").arg(p.root()).run();

    let p = project()
        .at("foo2")
        .file("Cargo.toml", &basic_manifest("foo", "0.2.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    cargo_process("install --force --path")
        .arg(p.root())
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.2.0 ([ROOT]/foo2)
[COMPILING] foo v0.2.0 ([ROOT]/foo2)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[REPLACING] [ROOT]/home/.cargo/bin/foo[EXE]
[REPLACED] package `foo v0.0.1 ([ROOT]/foo)` with `foo v0.2.0 ([ROOT]/foo2)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();

    cargo_process("install --list")
        .with_stdout_data(str![[r#"
foo v0.2.0 ([ROOT]/foo2):
    foo[EXE]

"#]])
        .run();
}

#[cargo_test]
fn install_force_partial_overlap() {
    let p = project()
        .file("src/bin/foo-bin1.rs", "fn main() {}")
        .file("src/bin/foo-bin2.rs", "fn main() {}")
        .build();

    cargo_process("install --path").arg(p.root()).run();

    let p = project()
        .at("foo2")
        .file("Cargo.toml", &basic_manifest("foo", "0.2.0"))
        .file("src/bin/foo-bin2.rs", "fn main() {}")
        .file("src/bin/foo-bin3.rs", "fn main() {}")
        .build();

    cargo_process("install --force --path")
        .arg(p.root())
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.2.0 ([ROOT]/foo2)
[COMPILING] foo v0.2.0 ([ROOT]/foo2)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo-bin3[EXE]
[REPLACING] [ROOT]/home/.cargo/bin/foo-bin2[EXE]
[REMOVING] executable `[ROOT]/home/.cargo/bin/foo-bin1[EXE]` from previous version foo v0.0.1 ([ROOT]/foo)
[INSTALLED] package `foo v0.2.0 ([ROOT]/foo2)` (executable `foo-bin3[EXE]`)
[REPLACED] package `foo v0.0.1 ([ROOT]/foo)` with `foo v0.2.0 ([ROOT]/foo2)` (executable `foo-bin2[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();

    cargo_process("install --list")
        .with_stdout_data(str![[r#"
foo v0.2.0 ([ROOT]/foo2):
    foo-bin2[EXE]
    foo-bin3[EXE]

"#]])
        .run();
}

#[cargo_test]
fn install_force_bin() {
    let p = project()
        .file("src/bin/foo-bin1.rs", "fn main() {}")
        .file("src/bin/foo-bin2.rs", "fn main() {}")
        .build();

    cargo_process("install --path").arg(p.root()).run();

    let p = project()
        .at("foo2")
        .file("Cargo.toml", &basic_manifest("foo", "0.2.0"))
        .file("src/bin/foo-bin1.rs", "fn main() {}")
        .file("src/bin/foo-bin2.rs", "fn main() {}")
        .build();

    cargo_process("install --force --bin foo-bin2 --path")
        .arg(p.root())
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.2.0 ([ROOT]/foo2)
[COMPILING] foo v0.2.0 ([ROOT]/foo2)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[REPLACING] [ROOT]/home/.cargo/bin/foo-bin2[EXE]
[REPLACED] package `foo v0.0.1 ([ROOT]/foo)` with `foo v0.2.0 ([ROOT]/foo2)` (executable `foo-bin2[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();

    cargo_process("install --list")
        .with_stdout_data(str![[r#"
foo v0.0.1 ([ROOT]/foo):
    foo-bin1[EXE]
foo v0.2.0 ([ROOT]/foo2):
    foo-bin2[EXE]

"#]])
        .run();
}

#[cargo_test]
fn compile_failure() {
    let p = project().file("src/main.rs", "").build();

    cargo_process("install --path")
        .arg(p.root())
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error
[ERROR] failed to compile `foo v0.0.1 ([ROOT]/foo)`, intermediate artifacts can be found at `[ROOT]/foo/target`.
To reuse those artifacts with a future compilation, set the environment variable `CARGO_TARGET_DIR` to that path.
...
"#]])
        .run();
}

#[cargo_test]
fn git_repo() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    // Use `--locked` to test that we don't even try to write a lock file.
    cargo_process("install --locked --git")
        .arg(p.url().to_string())
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/foo`
[WARNING] no Cargo.lock file published in foo v0.1.0 ([ROOTURL]/foo#[..])
[INSTALLING] foo v0.1.0 ([ROOTURL]/foo#[..])
[COMPILING] foo v0.1.0 ([ROOT]/home/.cargo/git/checkouts/foo-[HASH]/[..])
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.1.0 ([ROOTURL]/foo#[..])` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
#[cfg(target_os = "linux")]
fn git_repo_with_lowercase_cargo_toml() {
    let p = git::repo(&paths::root().join("foo"))
        .file("cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository [..]
[ERROR] Could not find Cargo.toml in `[..]`, but found cargo.toml please try to rename it to Cargo.toml

"#]]
        )
        .run();
}

#[cargo_test]
fn list() {
    pkg("foo", "0.0.1");
    pkg("bar", "0.2.1");
    pkg("bar", "0.2.2");

    cargo_process("install --list").with_stdout_data("").run();

    cargo_process("install bar --version =0.2.1").run();
    cargo_process("install foo").run();
    cargo_process("install --list")
        .with_stdout_data(str![[r#"
bar v0.2.1:
    bar[EXE]
foo v0.0.1:
    foo[EXE]

"#]])
        .run();
}

#[cargo_test]
fn list_error() {
    pkg("foo", "0.0.1");
    cargo_process("install foo").run();
    cargo_process("install --list")
        .with_stdout_data(str![[r#"
foo v0.0.1:
    foo[EXE]

"#]])
        .run();
    let mut worldfile_path = paths::cargo_home();
    worldfile_path.push(".crates.toml");
    let mut worldfile = OpenOptions::new()
        .write(true)
        .open(worldfile_path)
        .expect(".crates.toml should be there");
    worldfile.write_all(b"\x00").unwrap();
    drop(worldfile);
    cargo_process("install --list --verbose")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse crate metadata at `[ROOT]/home/.cargo/.crates.toml`

Caused by:
  invalid TOML found for metadata

Caused by:
  TOML parse error at line 1, column 4
    |
  1 | v1]
    |    ^
  key with no value, expected `=`

"#]])
        .run();
}

#[cargo_test]
fn uninstall_pkg_does_not_exist() {
    cargo_process("uninstall foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `foo` did not match any packages

"#]])
        .run();
}

#[cargo_test]
fn uninstall_bin_does_not_exist() {
    pkg("foo", "0.0.1");

    cargo_process("install foo").run();
    cargo_process("uninstall foo --bin=bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] binary `bar[EXE]` not installed as part of `foo v0.0.1`

"#]])
        .run();
}

#[cargo_test]
fn uninstall_piecemeal() {
    let p = project()
        .file("src/bin/foo.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .build();

    cargo_process("install --path").arg(p.root()).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
    assert_has_installed_exe(paths::cargo_home(), "bar");

    cargo_process("uninstall foo --bin=bar")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/bar[EXE]

"#]])
        .run();

    assert_has_installed_exe(paths::cargo_home(), "foo");
    assert_has_not_installed_exe(paths::cargo_home(), "bar");

    cargo_process("uninstall foo --bin=foo")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "foo");

    cargo_process("uninstall foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `foo` did not match any packages

"#]])
        .run();
}

#[cargo_test]
fn subcommand_works_out_of_the_box() {
    Package::new("cargo-foo", "1.0.0")
        .file("src/main.rs", r#"fn main() { println!("bar"); }"#)
        .publish();
    cargo_process("install cargo-foo").run();
    cargo_process("foo")
        .with_stdout_data(str![[r#"
bar

"#]])
        .run();
    cargo_process("--list")
        .with_stdout_data(str![[r#"
...
    foo
...
"#]])
        .run();
}

#[cargo_test]
fn installs_from_cwd_by_default() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("install").with_stderr_data(str![[r#"
[WARNING] Using `cargo install` to install the binaries from the package in current working directory is deprecated, use `cargo install --path .` instead. Use `cargo build` if you want to simply build the package.
...
"#]]).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn installs_from_cwd_with_2018_warnings() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                edition = "2018"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("install")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] Using `cargo install` to install the binaries from the package in current working directory is no longer supported, use `cargo install --path .` instead. Use `cargo build` if you want to simply build the package.

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn uninstall_cwd() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("install --path .").with_stderr_data(str![[r#"
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]]).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");

    p.cargo("uninstall")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn uninstall_cwd_not_installed() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("uninstall")
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] package `foo v0.0.1 ([ROOT]/foo)` is not installed

"#]])
        .run();
}

#[cargo_test]
fn uninstall_cwd_no_project() {
    cargo_process("uninstall")
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] failed to read `[ROOT]/Cargo.toml`

Caused by:
  [NOT_FOUND]

"#]])
        .run();
}

#[cargo_test]
fn do_not_rebuilds_on_local_install() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("build --release").run();
    cargo_process("install --path")
        .arg(p.root())
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 ([ROOT]/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();

    assert!(p.build_dir().exists());
    assert!(p.release_bin("foo").exists());
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn reports_unsuccessful_subcommand_result() {
    Package::new("cargo-fail", "1.0.0")
        .file("src/main.rs", r#"fn main() { panic!("EXPLICIT PANIC!"); }"#)
        .publish();
    cargo_process("install cargo-fail").run();
    cargo_process("--list")
        .with_stdout_data(str![[r#"
...
    fail
...
"#]])
        .run();
    cargo_process("fail")
        .with_status(101)
        .with_stderr_data("...\n[..]EXPLICIT PANIC![..]\n...")
        .run();
}

#[cargo_test]
fn git_with_lockfile() {
    let p = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "fn main() {}")
        .file(
            "Cargo.lock",
            r#"
                [[package]]
                name = "foo"
                version = "0.1.0"
                dependencies = [ "bar 0.1.0" ]

                [[package]]
                name = "bar"
                version = "0.1.0"
            "#,
        )
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .run();
}

#[cargo_test]
fn q_silences_warnings() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    cargo_process("install -q --path")
        .arg(p.root())
        .with_stderr_data("")
        .run();
}

#[cargo_test]
fn readonly_dir() {
    pkg("foo", "0.0.1");

    let root = paths::root();
    let dir = &root.join("readonly");
    fs::create_dir(root.join("readonly")).unwrap();
    let mut perms = fs::metadata(dir).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(dir, perms).unwrap();

    cargo_process("install foo").cwd(dir).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn use_path_workspace() {
    Package::new("foo", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                authors = []

                [dependencies]
                foo = "1"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    let lock = p.read_lockfile();
    p.cargo("install").run();
    let lock2 = p.read_lockfile();
    assert_eq!(lock, lock2, "different lockfiles");
}

#[cargo_test]
fn path_install_workspace_root_despite_default_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "ws-root"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["ws-member"]
                default-members = ["ws-member"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "ws-member/Cargo.toml",
            r#"
                [package]
                name = "ws-member"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("ws-member/src/main.rs", "fn main() {}")
        .build();

    p.cargo("install --path")
        .arg(p.root())
        .arg("ws-root")
        .with_stderr_data(str![[r#"
[INSTALLING] ws-root v0.1.0 ([ROOT]/foo)
[COMPILING] ws-root v0.1.0 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/ws-root[EXE]
[INSTALLED] package `ws-root v0.1.0 ([ROOT]/foo)` (executable `ws-root[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        // Particularly avoid "Installed package `ws-root v0.1.0 ([..]])` (executable `ws-member`)":
        .with_stderr_does_not_contain("ws-member")
        .run();
}

#[cargo_test]
fn git_install_workspace_root_despite_default_members() {
    let p = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "ws-root"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["ws-member"]
                default-members = ["ws-member"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "ws-member/Cargo.toml",
            r#"
                [package]
                name = "ws-member"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("ws-member/src/main.rs", "fn main() {}")
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .arg("ws-root")
        .with_stderr_data(str![[r#"
...
[INSTALLED] package `ws-root v0.1.0 ([ROOTURL]/foo#[..])` (executable `ws-root[EXE]`)
...
"#]])
        // Particularly avoid "Installed package `ws-root v0.1.0 ([..]])` (executable `ws-member`)":
        .with_stderr_does_not_contain("ws-member")
        .run();
}

#[cargo_test]
fn dev_dependencies_no_check() {
    Package::new("foo", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dev-dependencies]
                baz = "1.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] no matching package named `baz` found
...
"#]])
        .run();
    p.cargo("install").run();
}

#[cargo_test]
fn dev_dependencies_lock_file_untouched() {
    Package::new("foo", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dev-dependencies]
                bar = { path = "a" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    let lock = p.read_lockfile();
    p.cargo("install").run();
    let lock2 = p.read_lockfile();
    assert!(lock == lock2, "different lockfiles");
}

#[cargo_test]
fn install_target_native() {
    pkg("foo", "0.1.0");

    cargo_process("install foo --target")
        .arg(cargo_test_support::rustc_host())
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn install_target_foreign() {
    if cross_compile_disabled() {
        return;
    }

    pkg("foo", "0.1.0");

    cargo_process("install foo --target")
        .arg(cross_compile::alternate())
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn vers_precise() {
    pkg("foo", "0.1.1");
    pkg("foo", "0.1.2");

    cargo_process("install foo --vers 0.1.1")
        .with_stderr_data(str![[r#"
...
[DOWNLOADED] foo v0.1.1 (registry `dummy-registry`)
...
"#]])
        .run();
}

#[cargo_test]
fn version_precise() {
    pkg("foo", "0.1.1");
    pkg("foo", "0.1.2");

    cargo_process("install foo --version 0.1.1")
        .with_stderr_data(str![[r#"
...
[DOWNLOADED] foo v0.1.1 (registry `dummy-registry`)
...
"#]])
        .run();
}

#[cargo_test]
fn inline_version_precise() {
    pkg("foo", "0.1.1");
    pkg("foo", "0.1.2");

    cargo_process("install foo@0.1.1")
        .with_stderr_data(str![[r#"
...
[DOWNLOADED] foo v0.1.1 (registry `dummy-registry`)
...
"#]])
        .run();
}

#[cargo_test]
fn inline_version_multiple() {
    pkg("foo", "0.1.0");
    pkg("foo", "0.1.1");
    pkg("foo", "0.1.2");
    pkg("bar", "0.2.0");
    pkg("bar", "0.2.1");
    pkg("bar", "0.2.2");

    cargo_process("install foo@0.1.1 bar@0.2.1")
        .with_stderr_data(str![[r#"
...
[DOWNLOADED] foo v0.1.1 (registry `dummy-registry`)
...
[DOWNLOADED] bar v0.2.1 (registry `dummy-registry`)
...
"#]])
        .run();
}

#[cargo_test]
fn inline_version_without_name() {
    pkg("foo", "0.1.1");
    pkg("foo", "0.1.2");

    cargo_process("install @0.1.1")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] invalid value '@0.1.1' for '[CRATE[@<VER>]]...': missing crate name before '@'

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn inline_and_explicit_version() {
    pkg("foo", "0.1.1");
    pkg("foo", "0.1.2");

    cargo_process("install foo@0.1.1 --version 0.1.1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot specify both `@<VERSION>` and `--version <VERSION>`

"#]])
        .run();
}

#[cargo_test]
fn not_both_vers_and_version() {
    pkg("foo", "0.1.1");
    pkg("foo", "0.1.2");

    cargo_process("install foo --version 0.1.1 --vers 0.1.2")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--version <VERSION>' cannot be used multiple times

Usage: cargo[EXE] install [OPTIONS] [CRATE[@<VER>]]...

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn test_install_git_cannot_be_a_base_url() {
    cargo_process("install --git github.com:rust-lang/rustfmt.git")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid url `github.com:rust-lang/rustfmt.git`: cannot-be-a-base-URLs are not supported

"#]])
        .run();
}

#[cargo_test]
fn uninstall_multiple_and_specifying_bin() {
    cargo_process("uninstall foo bar --bin baz")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] A binary can only be associated with a single installed package, specifying multiple specs with --bin is redundant.

"#]])
        .run();
}

#[cargo_test]
fn uninstall_with_empty_package_option() {
    cargo_process("uninstall -p")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] "--package <SPEC>" requires a SPEC format value.
Run `cargo help pkgid` for more information about SPEC format.

"#]])
        .run();
}

#[cargo_test]
fn uninstall_multiple_and_some_pkg_does_not_exist() {
    pkg("foo", "0.0.1");

    cargo_process("install foo").run();

    cargo_process("uninstall foo bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]
[ERROR] package ID specification `bar` did not match any packages
[SUMMARY] Successfully uninstalled foo! Failed to uninstall bar (see error(s) above).
[ERROR] some packages failed to uninstall

"#]])
        .run();

    assert_has_not_installed_exe(paths::cargo_home(), "foo");
    assert_has_not_installed_exe(paths::cargo_home(), "bar");
}

#[cargo_test]
fn custom_target_dir_for_git_source() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    cargo_process("install --git")
        .arg(p.url().to_string())
        .run();
    assert!(!paths::root().join("target/release").is_dir());

    cargo_process("install --force --git")
        .arg(p.url().to_string())
        .env("CARGO_TARGET_DIR", "target")
        .run();
    assert!(paths::root().join("target/release").is_dir());
}

#[cargo_test]
fn install_respects_lock_file() {
    // `cargo install` now requires --locked to use a Cargo.lock.
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.1.1")
        .file("src/lib.rs", "not rust")
        .publish();
    Package::new("foo", "0.1.0")
        .dep("bar", "0.1")
        .file("src/lib.rs", "")
        .file(
            "src/main.rs",
            "extern crate foo; extern crate bar; fn main() {}",
        )
        .file(
            "Cargo.lock",
            r#"
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "foo"
version = "0.1.0"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]
"#,
        )
        .publish();

    cargo_process("install foo")
        .with_stderr_data(str![[r#"
...
[..]not rust[..]
...
"#]])
        .with_status(101)
        .run();
    cargo_process("install --locked foo").run();
}

#[cargo_test]
fn install_path_respects_lock_file() {
    // --path version of install_path_respects_lock_file, --locked is required
    // to use Cargo.lock.
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.1.1")
        .file("src/lib.rs", "not rust")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = "0.1"
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .file(
            "Cargo.lock",
            r#"
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "foo"
version = "0.1.0"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]
"#,
        )
        .build();

    p.cargo("install --path .")
        .with_stderr_data(str![[r#"
...
[..]not rust[..]
...
"#]])
        .with_status(101)
        .run();
    p.cargo("install --path . --locked").run();
}

#[cargo_test]
fn lock_file_path_deps_ok() {
    Package::new("bar", "0.1.0").publish();

    Package::new("foo", "0.1.0")
        .dep("bar", "0.1")
        .file("src/lib.rs", "")
        .file(
            "src/main.rs",
            "extern crate foo; extern crate bar; fn main() {}",
        )
        .file(
            "Cargo.lock",
            r#"
            [[package]]
            name = "bar"
            version = "0.1.0"

            [[package]]
            name = "foo"
            version = "0.1.0"
            dependencies = [
             "bar 0.1.0",
            ]
            "#,
        )
        .publish();

    cargo_process("install foo").run();
}

#[cargo_test]
fn install_empty_argument() {
    // Bug 5229
    cargo_process("install")
        .arg("")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] invalid value '' for '[CRATE[@<VER>]]...': crate name is empty

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn git_repo_replace() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();
    let repo = git2::Repository::open(&p.root()).unwrap();
    let old_rev = repo.revparse_single("HEAD").unwrap().id();
    cargo_process("install --git")
        .arg(p.url().to_string())
        .run();
    git::commit(&repo);
    let new_rev = repo.revparse_single("HEAD").unwrap().id();
    let mut path = paths::home();
    path.push(".cargo/.crates.toml");

    assert_ne!(old_rev, new_rev);
    assert!(
        fs::read_to_string(path.clone())
            .unwrap()
            .contains(&format!("{}", old_rev))
    );
    cargo_process("install --force --git")
        .arg(p.url().to_string())
        .run();
    assert!(
        fs::read_to_string(path)
            .unwrap()
            .contains(&format!("{}", new_rev))
    );
}

#[cargo_test]
fn workspace_uses_workspace_target_dir() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [workspace]

                [dependencies]
                bar = { path = 'bar' }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --release").cwd("bar").run();
    cargo_process("install --path")
        .arg(p.root().join("bar"))
        .with_stderr_data(str![[r#"
[INSTALLING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/bar[EXE]
[INSTALLED] package `bar v0.1.0 ([ROOT]/foo/bar)` (executable `bar[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn install_ignores_local_cargo_config() {
    pkg("bar", "0.0.1");

    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                target = "non-existing-target"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("install bar").run();
    assert_has_installed_exe(paths::cargo_home(), "bar");
}

#[cargo_test]
fn install_ignores_unstable_table_in_local_cargo_config() {
    pkg("bar", "0.0.1");

    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [unstable]
                build-std = ["core"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("install bar")
        .masquerade_as_nightly_cargo(&["build-std"])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "bar");
}

#[cargo_test]
fn install_global_cargo_config() {
    pkg("bar", "0.0.1");

    let config = paths::cargo_home().join("config.toml");
    let mut toml = fs::read_to_string(&config).unwrap_or_default();

    toml.push_str(
        r#"
            [build]
            target = 'nonexistent'
        "#,
    );
    fs::write(&config, toml).unwrap();

    cargo_process("install bar")
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[INSTALLING] bar v0.0.1
Caused by:
  process didn't exit successfully: `rustc [..]--target nonexistent[..]` ([EXIT_STATUS]: 1)
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn install_path_config() {
    project()
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target = 'nonexistent'
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    cargo_process("install --path foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
  process didn't exit successfully: `rustc [..]--target nonexistent[..]` ([EXIT_STATUS]: 1)
...
"#]])
        .run();
}

#[cargo_test]
fn install_version_req() {
    // Try using a few versionreq styles.
    pkg("foo", "0.0.3");
    pkg("foo", "1.0.4");
    pkg("foo", "1.0.5");
    cargo_process("install foo --version=*")
        .with_stderr_does_not_contain("[WARNING][..]is not a valid semver[..]")
        .with_stderr_data(str![[r#"
...
[INSTALLING] foo v1.0.5
...
"#]])
        .run();
    cargo_process("uninstall foo").run();
    cargo_process("install foo --version=^1.0")
        .with_stderr_does_not_contain("[WARNING][..]is not a valid semver[..]")
        .with_stderr_data(str![[r#"
...
[INSTALLING] foo v1.0.5
...
"#]])
        .run();
    cargo_process("uninstall foo").run();
    cargo_process("install foo --version=0.0.*")
        .with_stderr_does_not_contain("[WARNING][..]is not a valid semver[..]")
        .with_stderr_data(str![[r#"
...
[INSTALLING] foo v0.0.3
...
"#]])
        .run();
}

#[cargo_test]
fn git_install_reads_workspace_manifest() {
    let p = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bin1"]

            [profile.release]
            incremental = 3
            "#,
        )
        .file("bin1/Cargo.toml", &basic_manifest("bin1", "0.1.0"))
        .file(
            "bin1/src/main.rs",
            r#"fn main() { println!("Hello, world!"); }"#,
        )
        .build();

    cargo_process(&format!("install --git {}", p.url()))
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/foo`
[ERROR] invalid type: integer `3`, expected a boolean
 --> home/.cargo/git/checkouts/foo-[HASH]/[..]/Cargo.toml:6:27
  |
6 |             incremental = 3
  |                           ^
[ERROR] invalid type: integer `3`, expected a boolean
 --> home/.cargo/git/checkouts/foo-[HASH]/[..]/Cargo.toml:6:27
  |
6 |             incremental = 3
  |                           ^

"#]])
        .run();
}

#[cargo_test]
fn install_git_with_symlink_home() {
    // Ensure that `cargo install` with a git repo is OK when CARGO_HOME is a
    // symlink, and uses an build script.
    if !symlink_supported() {
        return;
    }
    let p = git::new("foo", |p| {
        p.file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
            .file("src/main.rs", "fn main() {}")
            // This triggers discover_git_and_list_files for detecting changed files.
            .file("build.rs", "fn main() {}")
    });
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir as symlink;

    let actual = paths::root().join("actual-home");
    t!(std::fs::create_dir(&actual));
    t!(symlink(&actual, paths::home().join(".cargo")));
    cargo_process("install --git")
        .arg(p.url().to_string())
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/foo`
[INSTALLING] foo v1.0.0 ([ROOTURL]/foo#[..])
[COMPILING] foo v1.0.0 ([ROOT]/home/.cargo/git/checkouts/foo-[HASH]/[..])
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v1.0.0 ([ROOTURL]/foo#[..])` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn install_yanked_cargo_package() {
    Package::new("baz", "0.0.1").yanked(true).publish();
    cargo_process("install baz --version 0.0.1")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] cannot install package `baz`, it has been yanked from registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn install_cargo_package_in_a_patched_workspace() {
    pkg("foo", "0.1.0");
    pkg("fizz", "1.0.0");

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [workspace]
                members = ["baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                authors = []

                [dependencies]
                fizz = "1"

                [patch.crates-io]
                fizz = { version = "=1.0.0" }
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] patch for the non root package will be ignored, specify patch at the workspace root:
package:   [ROOT]/foo/baz/Cargo.toml
workspace: [ROOT]/foo/Cargo.toml
...
"#]])
        .run();

    // A crate installation must not emit any message from a workspace under
    // current working directory.
    // See https://github.com/rust-lang/cargo/issues/8619
    p.cargo("install foo").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.1.0 (registry `dummy-registry`)
[INSTALLING] foo v0.1.0
[COMPILING] foo v0.1.0
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.1.0` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]]).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn locked_install_without_published_lockfile() {
    Package::new("foo", "0.1.0")
        .file("src/main.rs", "//! Some docs\nfn main() {}")
        .publish();

    cargo_process("install foo --locked")
        .with_stderr_data(str![[r#"
...
[WARNING] no Cargo.lock file published in foo v0.1.0
...
"#]])
        .run();
}

#[cargo_test]
fn install_semver_metadata() {
    // Check trying to install a package that uses semver metadata.
    // This uses alt registry because the bug this is exercising doesn't
    // trigger with a replaced source.
    registry::alt_init();
    Package::new("foo", "1.0.0+abc")
        .alternative(true)
        .file("src/main.rs", "fn main() {}")
        .publish();

    cargo_process("install foo --registry alternative --version 1.0.0+abc").run();
    cargo_process("install foo --registry alternative")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[IGNORED] package `foo v1.0.0+abc (registry `alternative`)` is already installed, use --force to override
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    // "Updating" is not displayed here due to the --version fast-path.
    cargo_process("install foo --registry alternative --version 1.0.0+abc")
        .with_stderr_data(str![[r#"
[IGNORED] package `foo v1.0.0+abc (registry `alternative`)` is already installed, use --force to override
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    cargo_process("install foo --registry alternative --version 1.0.0 --force")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[INSTALLING] foo v1.0.0+abc (registry `alternative`)
[COMPILING] foo v1.0.0+abc (registry `alternative`)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[REPLACING] [ROOT]/home/.cargo/bin/foo[EXE]
[REPLACED] package `foo v1.0.0+abc (registry `alternative`)` with `foo v1.0.0+abc (registry `alternative`)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    // Check that from a fresh cache will work without metadata, too.
    paths::home().join(".cargo/registry").rm_rf();
    paths::home().join(".cargo/bin").rm_rf();
    cargo_process("install foo --registry alternative --version 1.0.0")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v1.0.0+abc (registry `alternative`)
[INSTALLING] foo v1.0.0+abc (registry `alternative`)
[COMPILING] foo v1.0.0+abc (registry `alternative`)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v1.0.0+abc (registry `alternative`)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
}

#[cargo_test]
fn no_auto_fix_note() {
    Package::new("auto_fix", "0.0.1")
        .file("src/lib.rs", "use std::io;")
        .file(
            "src/main.rs",
            &format!("extern crate {}; use std::io; fn main() {{}}", "auto_fix"),
        )
        .publish();

    // This should not contain a suggestion to run `cargo fix`
    //
    // This is checked by matching the full output as `with_stderr_does_not_contain`
    // can be brittle
    cargo_process("install auto_fix")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] auto_fix v0.0.1 (registry `dummy-registry`)
[INSTALLING] auto_fix v0.0.1
[COMPILING] auto_fix v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/auto_fix[EXE]
[INSTALLED] package `auto_fix v0.0.1` (executable `auto_fix[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "auto_fix");

    cargo_process("uninstall auto_fix")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/auto_fix[EXE]

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "auto_fix");
}

#[cargo_test]
fn failed_install_retains_temp_directory() {
    // Verifies that the temporary directory persists after a build failure.
    Package::new("foo", "0.0.1")
        .file("src/main.rs", "x")
        .publish();
    let err = cargo_process("install foo").exec_with_output().unwrap_err();
    let err = err.downcast::<ProcessError>().unwrap();
    let stderr = String::from_utf8(err.stderr.unwrap()).unwrap();
    assert_e2e().eq(&stderr, str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[ERROR] expected one of `!` or `::`, found `<eof>`
 --> [ROOT]/home/.cargo/registry/src/-[..]/foo-0.0.1/src/main.rs:1:1
  |
1 | x
  | ^ expected one of `!` or `::`

[ERROR] could not compile `foo` (bin "foo") due to 1 previous error
[ERROR] failed to compile `foo v0.0.1`, intermediate artifacts can be found at `[..]`.
To reuse those artifacts with a future compilation, set the environment variable `CARGO_TARGET_DIR` to that path.

"#]]);

    // Find the path in the output.
    let stderr = stderr.split_once("found at `").unwrap().1;
    let end = stderr.find('.').unwrap() - 1;
    let path = Path::new(&stderr[..end]);
    assert!(path.exists());
    assert!(path.join("release/deps").exists());
}

#[cargo_test]
fn sparse_install() {
    // Checks for an issue where uninstalling something corrupted
    // the SourceIds of sparse registries.
    // See https://github.com/rust-lang/cargo/issues/11751
    let _registry = registry::RegistryBuilder::new().http_index().build();

    pkg("foo", "0.0.1");
    pkg("bar", "0.0.1");

    cargo_process("install foo --registry dummy-registry")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1 (registry `dummy-registry`)
[UPDATING] `dummy-registry` index
[COMPILING] foo v0.0.1 (registry `dummy-registry`)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1 (registry `dummy-registry`)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
    let assert_v1 = |expected| {
        let v1 = fs::read_to_string(paths::home().join(".cargo/.crates.toml")).unwrap();
        assert_e2e().eq(&v1, expected);
    };
    assert_v1(str![[r#"
[v1]
"foo 0.0.1 (sparse+http://127.0.0.1:[..]/index/)" = ["foo[EXE]"]

"#]]);
    cargo_process("install bar").run();
    assert_has_installed_exe(paths::cargo_home(), "bar");
    assert_v1(str![[r#"
[v1]
"bar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)" = ["bar[EXE]"]
"foo 0.0.1 (sparse+http://127.0.0.1:[..]/index/)" = ["foo[EXE]"]

"#]]);

    cargo_process("uninstall bar")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/bar[EXE]

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "bar");
    assert_v1(str![[r#"
[v1]
"foo 0.0.1 (sparse+http://127.0.0.1:[..]/index/)" = ["foo[EXE]"]

"#]]);
    cargo_process("uninstall foo")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "foo");
    assert_v1(str![[r#"
[v1]

"#]]);
}

#[cargo_test]
fn self_referential() {
    // Some packages build-dep on prior versions of themselves.
    Package::new("foo", "0.0.1")
        .file("src/lib.rs", "fn hello() {}")
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "fn main() {}")
        .publish();
    Package::new("foo", "0.0.2")
        .file("src/lib.rs", "fn hello() {}")
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "fn main() {}")
        .build_dep("foo", "0.0.1")
        .publish();

    cargo_process("install foo").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.2 (registry `dummy-registry`)
[INSTALLING] foo v0.0.2
[LOCKING] 1 package to latest compatible version
[ADDING] foo v0.0.1 (available: v0.0.2)
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[COMPILING] foo v0.0.1
[COMPILING] foo v0.0.2
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.2` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]]).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn ambiguous_registry_vs_local_package() {
    // Correctly install 'foo' from a local package, even if that package also
    // depends on a registry dependency named 'foo'.
    Package::new("foo", "0.0.1")
        .file("src/lib.rs", "fn hello() {}")
        .publish();

    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "0.1.0"
        authors = []
        edition = "2021"

        [dependencies]
        foo = "0.0.1"
    "#,
        )
        .build();

    cargo_process("install --path")
        .arg(p.root())
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.1.0 ([ROOT]/foo)
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[COMPILING] foo v0.0.1
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.1.0 ([ROOT]/foo)` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn install_with_redundant_default_mode() {
    pkg("foo", "0.0.1");

    cargo_process("install foo --release")
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '--release' found

  tip: `--release` is the default for `cargo install`; instead `--debug` is supported

Usage: cargo[EXE] install [OPTIONS] [CRATE[@<VER>]]...

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn install_incompat_msrv() {
    Package::new("foo", "0.1.0")
        .file("src/main.rs", "fn main() {}")
        .rust_version("1.30")
        .publish();
    Package::new("foo", "0.2.0")
        .file("src/main.rs", "fn main() {}")
        .rust_version("1.9876.0")
        .publish();

    cargo_process("install foo")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] cannot install package `foo 0.2.0`, it requires rustc 1.9876.0 or newer, while the currently active rustc version is [..]
`foo 0.1.0` supports rustc 1.30

"#]])
        .with_status(101)
        .run();
}

fn assert_tracker_noexistence(key: &str) {
    let v1_data: toml::Value =
        toml::from_str(&fs::read_to_string(paths::cargo_home().join(".crates.toml")).unwrap())
            .unwrap();
    let v2_data: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(paths::cargo_home().join(".crates2.json")).unwrap(),
    )
    .unwrap();

    assert!(v1_data["v1"].get(key).is_none());
    assert!(v2_data["installs"][key].is_null());
}

#[cargo_test]
fn uninstall_running_binary() {
    use std::io::Write;

    Package::new("foo", "0.0.1")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                use std::net::TcpStream;
                use std::env::var;
                use std::io::Read;
                fn main() {
                    for i in 0..2 {
                        TcpStream::connect(&var("__ADDR__").unwrap()[..])
                            .unwrap()
                            .read_to_end(&mut Vec::new())
                            .unwrap();
                    }
                }
            "#,
        )
        .publish();

    cargo_process("install foo").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]]).run();
    assert_has_installed_exe(paths::cargo_home(), "foo");

    let foo_bin = paths::cargo_home().join("bin").join(exe("foo"));
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let t = thread::spawn(move || {
        ProcessBuilder::new(foo_bin)
            .env("__ADDR__", addr)
            .exec()
            .unwrap();
    });
    let key = "foo 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)";

    #[cfg(windows)]
    {
        // Ensure foo is running before the first `cargo uninstall` call
        l.accept().unwrap().0.write_all(&[1]).unwrap();
        cargo_process("uninstall foo")
            .with_status(101)
            .with_stderr_data(str![[r#"
...
[ERROR] failed to remove file `[ROOT]/home/.cargo/bin/foo[EXE]`
...
"#]])
            .run();
        // Ensure foo is stopped before the second `cargo uninstall` call
        l.accept().unwrap().0.write_all(&[1]).unwrap();
        t.join().unwrap();
        cargo_process("uninstall foo")
            .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]

"#]])
            .run();
    };

    #[cfg(not(windows))]
    {
        // Ensure foo is running before the first `cargo uninstall` call
        l.accept().unwrap().0.write_all(&[1]).unwrap();
        cargo_process("uninstall foo")
            .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/foo[EXE]

"#]])
            .run();
        l.accept().unwrap().0.write_all(&[1]).unwrap();
        t.join().unwrap();
    };

    assert_has_not_installed_exe(paths::cargo_home(), "foo");
    assert_tracker_noexistence(key);

    cargo_process("install foo").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[INSTALLED] package `foo v0.0.1` (executable `foo[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]]).run();
}

#[cargo_test]
fn dry_run() {
    pkg("foo", "0.0.1");

    cargo_process("-Z unstable-options install --dry-run foo")
        .masquerade_as_nightly_cargo(&["install::dry-run"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.1 (registry `dummy-registry`)
[INSTALLING] foo v0.0.1
[INSTALLING] [ROOT]/home/.cargo/bin/foo[EXE]
[WARNING] aborting install due to dry run
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn dry_run_incompatible_package() {
    Package::new("some-package-from-the-distant-future", "0.0.1")
        .rust_version("1.2345.0")
        .file("src/main.rs", "fn main() {}")
        .publish();

    cargo_process("-Z unstable-options install --dry-run some-package-from-the-distant-future")
        .masquerade_as_nightly_cargo(&["install::dry-run"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] cannot install package `some-package-from-the-distant-future 0.0.1`, it requires rustc 1.2345.0 or newer, while the currently active rustc version is [..]

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "some-package-from-the-distant-future");
}

#[cargo_test]
fn dry_run_incompatible_package_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                some-package-from-the-distant-future = { path = "a" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "some-package-from-the-distant-future"
                version = "0.1.0"
                authors = []
                rust-version = "1.2345.0"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    cargo_process("-Z unstable-options install --dry-run --path")
        .arg(p.root())
        .arg("foo")
        .masquerade_as_nightly_cargo(&["install::dry-run"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[INSTALLING] foo v0.1.0 ([ROOT]/foo)
[LOCKING] 1 package to latest compatible version
[ERROR] failed to compile `foo v0.1.0 ([ROOT]/foo)`, intermediate artifacts can be found at `[ROOT]/foo/target`.
To reuse those artifacts with a future compilation, set the environment variable `CARGO_TARGET_DIR` to that path.

Caused by:
  rustc [..] is not supported by the following package:
    some-package-from-the-distant-future@0.1.0 requires rustc 1.2345.0

"#]])
        .run();
    assert_has_not_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn dry_run_upgrade() {
    pkg("foo", "0.0.1");
    cargo_process("install foo").run();
    assert_has_installed_exe(paths::cargo_home(), "foo");

    pkg("foo", "0.0.2");
    cargo_process("-Z unstable-options install --dry-run foo")
        .masquerade_as_nightly_cargo(&["install::dry-run"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.0.2 (registry `dummy-registry`)
[INSTALLING] foo v0.0.2
[REPLACING] [ROOT]/home/.cargo/bin/foo[EXE]
[WARNING] aborting install due to dry run
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn dry_run_remove_orphan() {
    Package::new("bar", "1.0.0")
        .file("src/bin/client.rs", "fn main() {}")
        .file("src/bin/server.rs", "fn main() {}")
        .publish();

    cargo_process("install bar")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[INSTALLING] bar v1.0.0
[COMPILING] bar v1.0.0
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[INSTALLING] [ROOT]/home/.cargo/bin/client[EXE]
[INSTALLING] [ROOT]/home/.cargo/bin/server[EXE]
[INSTALLED] package `bar v1.0.0` (executables `client[EXE]`, `server[EXE]`)
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "client");
    assert_has_installed_exe(paths::cargo_home(), "server");

    Package::new("bar", "2.0.0")
        .file("src/bin/client.rs", "fn main() {}")
        .publish();

    cargo_process("-Z unstable-options install --dry-run bar")
        .masquerade_as_nightly_cargo(&["install::dry-run"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v2.0.0 (registry `dummy-registry`)
[INSTALLING] bar v2.0.0
[REPLACING] [ROOT]/home/.cargo/bin/client[EXE]
[REMOVING] executable `[ROOT]/home/.cargo/bin/server[EXE]` from previous version bar v1.0.0
[WARNING] aborting install due to dry run
[WARNING] be sure to add `[ROOT]/home/.cargo/bin` to your PATH to be able to run the installed binaries

"#]])
        .run();
    assert_has_installed_exe(paths::cargo_home(), "client");
    // Ensure server is still installed after the dry run
    assert_has_installed_exe(paths::cargo_home(), "server");
}

#[cargo_test]
fn prefixed_v_in_version() {
    pkg("foo", "0.0.1");
    cargo_process("install foo@v0.0.1")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] invalid value 'foo@v0.0.1' for '[CRATE[@<VER>]]...': the version provided, `v0.0.1` is not a valid SemVer requirement

[HELP] try changing the version to `0.0.1`

For more information, try '--help'.

"#]])
        .run();
}
