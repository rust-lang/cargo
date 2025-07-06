//! Tests for `lockfile-path` flag

use std::fs;

use snapbox::str;

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::install::assert_has_installed_exe;
use cargo_test_support::registry::{Package, RegistryBuilder};
use cargo_test_support::{
    ProjectBuilder, basic_bin_manifest, cargo_test, paths, project, symlink_supported,
};
///////////////////////////////
//// Unstable feature tests start
///////////////////////////////

#[cargo_test]
fn must_have_unstable_options() {
    let lockfile_path = "mylockfile/is/burried/Cargo.lock";
    let p = make_project().build();

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .with_stderr_data(str![[
            r#"[ERROR] the `--lockfile-path` flag is unstable, pass `-Z unstable-options` to enable it
See https://github.com/rust-lang/cargo/issues/14421 for more information about the `--lockfile-path` flag.

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn must_be_nightly() {
    let lockfile_path = "mylockfile/is/burried/Cargo.lock";
    let p = make_project().build();

    p.cargo("generate-lockfile")
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .with_stderr_data(str![[
            r#"[ERROR] the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.

"#]])
        .with_status(101)
        .run();
}

///////////////////////////////
//// Unstable feature tests end
///////////////////////////////

#[cargo_test]
fn basic_lockfile_created() {
    let lockfile_path = "mylockfile/is/burried/Cargo.lock";
    let p = make_project().build();

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .run();
    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path).is_file());
}

#[cargo_test]
fn basic_lockfile_read() {
    let lockfile_path = "mylockfile/Cargo.lock";
    let p = make_project().file(lockfile_path, VALID_LOCKFILE).build();

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .run();

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path).is_file());
}

#[cargo_test]
fn basic_lockfile_override() {
    let lockfile_path = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .run();

    assert!(p.root().join(lockfile_path).is_file());
}

//////////////////////
///// Symlink tests
//////////////////////

#[cargo_test]
fn symlink_in_path() {
    if !symlink_supported() {
        return;
    }

    let dst = "dst";
    let src = "somedir/link";
    let lockfile_path = format!("{src}/Cargo.lock");

    let p = make_project().symlink_dir(dst, src).build();

    fs::create_dir(p.root().join("dst")).unwrap();
    assert!(p.root().join(src).is_dir());

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path.as_str())
        .run();

    assert!(p.root().join(lockfile_path).is_file());
    assert!(p.root().join(dst).join("Cargo.lock").is_file());
}

#[cargo_test]
fn symlink_lockfile() {
    if !symlink_supported() {
        return;
    }

    let lockfile_path = "dst/Cargo.lock";
    let src = "somedir/link";
    let lock_body = VALID_LOCKFILE;

    let p = make_project()
        .file(lockfile_path, lock_body)
        .symlink(lockfile_path, src)
        .build();

    assert!(p.root().join(src).is_file());

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .run();

    assert!(!p.root().join("Cargo.lock").exists());
}

#[cargo_test]
fn broken_symlink() {
    if !symlink_supported() {
        return;
    }

    let invalid_dst = "invalid_path";
    let src = "somedir/link";
    let lockfile_path = format!("{src}/Cargo.lock");

    let p = make_project().symlink_dir(invalid_dst, src).build();
    assert!(!p.root().join(src).is_dir());

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .with_status(101)
        .with_stderr_data(str![[
            r#"[ERROR] failed to create directory `[ROOT]/foo/somedir/link`

...

"#
        ]])
        .run();
}

#[cargo_test]
fn loop_symlink() {
    if !symlink_supported() {
        return;
    }

    let loop_link = "loop";
    let src = "somedir/link";
    let lockfile_path = format!("{src}/Cargo.lock");

    let p = make_project()
        .symlink_dir(loop_link, src)
        .symlink_dir(src, loop_link)
        .build();
    assert!(!p.root().join(src).is_dir());

    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .with_status(101)
        .with_stderr_data(str![[
            r#"[ERROR] failed to create directory `[ROOT]/foo/somedir/link`

...

"#
        ]])
        .run();
}

/////////////////////////
//// Commands tests
/////////////////////////

#[cargo_test]
fn add_lockfile_override() {
    let lockfile_path = "mylockfile/Cargo.lock";
    project()
        .at("bar")
        .file("Cargo.toml", LIB_TOML)
        .file("src/main.rs", "fn main() {}")
        .build();
    let p = make_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    p.cargo("add")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .arg("--path")
        .arg("../bar")
        .run();

    assert!(p.root().join(lockfile_path).is_file());
}

#[cargo_test]
fn clean_lockfile_override() {
    let lockfile_path = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    p.cargo("clean")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .arg("--package")
        .arg("test_foo")
        .run();

    assert!(p.root().join(lockfile_path).is_file());
}

#[cargo_test]
fn fix_lockfile_override() {
    let lockfile_path = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    p.cargo("fix")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .arg("--package")
        .arg("test_foo")
        .arg("--allow-no-vcs")
        .run();

    assert!(p.root().join(lockfile_path).is_file());
}

#[cargo_test]
fn publish_lockfile_read() {
    let lockfile_path = "mylockfile/Cargo.lock";
    let p = make_project().file(lockfile_path, VALID_LOCKFILE).build();
    let registry = RegistryBuilder::new().http_api().http_index().build();

    p.cargo("publish")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .replace_crates_io(registry.index_url())
        .run();

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path).is_file());
}

#[cargo_test]
fn remove_lockfile_override() {
    let lockfile_path = "mylockfile/Cargo.lock";
    let manifest = r#"
        [package]

        name = "foo"
        version = "0.5.0"
        authors = ["wycats@example.com"]
        edition = "2015"

        [[bin]]

        name = "foo"

        [dependencies]
        test_bar = { version = "0.1.0", path = "../bar" }
    "#;

    project()
        .at("bar")
        .file("Cargo.toml", LIB_TOML)
        .file("src/main.rs", "fn main() {}")
        .build();

    let p = project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", "fn main() {}")
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    p.cargo("remove")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .arg("test_bar")
        .run();

    assert!(p.root().join(lockfile_path).is_file());
}

#[cargo_test]
fn assert_respect_pinned_version_from_lockfile_path() {
    let lockfile_path = "mylockfile/Cargo.lock";
    let p = project()
        .file(
            "Cargo.toml",
            r#"#
[package]

name = "test_foo"
version = "0.5.0"
authors = ["wycats@example.com"]
edition = "2015"

[[bin]]

name = "test_foo"

[dependencies]
bar = "0.1.0"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.1.0").publish();
    p.cargo("generate-lockfile")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .run();

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path).is_file());

    let lockfile_original = fs::read_to_string(p.root().join(lockfile_path)).unwrap();

    Package::new("bar", "0.1.1").publish();
    p.cargo("package")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .run();

    assert!(
        p.root()
            .join("target/package/test_foo-0.5.0/Cargo.lock")
            .is_file()
    );

    let path = p.root().join("target/package/test_foo-0.5.0/Cargo.lock");
    let contents = fs::read_to_string(path).unwrap();

    assert_e2e().eq(contents, lockfile_original);
}

#[cargo_test]
fn install_respects_lock_file_path() {
    // `cargo install` will imply --locked when lockfile path is provided
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.1.1")
        .file("src/lib.rs", "not rust")
        .publish();
    // Publish with lockfile containing bad version of `bar` (0.1.1)
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
version = "0.1.1"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "foo"
version = "0.1.0"
dependencies = [
 "bar 0.1.1 (registry+https://github.com/rust-lang/crates.io-index)",
]
"#,
        )
        .publish();

    cargo_process("install foo --locked")
        .with_stderr_data(str![[r#"
...
[..]not rust[..]
...
"#]])
        .with_status(101)
        .run();

    // Create lockfile with the good `bar` version (0.1.0) and use it for install
    project()
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
    cargo_process("install foo -Zunstable-options --lockfile-path foo/Cargo.lock")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .run();

    assert!(paths::root().join("foo/Cargo.lock").is_file());
    assert_has_installed_exe(paths::cargo_home(), "foo");
}

#[cargo_test]
fn install_lock_file_path_must_present() {
    // `cargo install` will imply --locked when lockfile path is provided
    Package::new("bar", "0.1.0").publish();
    Package::new("foo", "0.1.0")
        .dep("bar", "0.1")
        .file("src/lib.rs", "")
        .file(
            "src/main.rs",
            "extern crate foo; extern crate bar; fn main() {}",
        )
        .publish();

    cargo_process("install foo -Zunstable-options --lockfile-path lockfile_dir/Cargo.lock")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .with_stderr_data(str![[r#"
...
[ERROR] no Cargo.lock file found in the requested path [ROOT]/lockfile_dir/Cargo.lock
...
"#]])
        .with_status(101)
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn run_embed() {
    let lockfile_path = "mylockfile/Cargo.lock";
    let invalid_lockfile = "Cargo.lock";
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("-Zscript")
        .arg("--lockfile-path")
        .arg(lockfile_path)
        .arg("--manifest-path")
        .arg("src/main.rs")
        .run();

    assert!(p.root().join(lockfile_path).is_file());

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("-Zscript")
        .arg("--lockfile-path")
        .arg(invalid_lockfile)
        .arg("--manifest-path")
        .arg("src/main.rs")
        .with_status(101)
        .with_stderr_data(str![[
            r#"[WARNING] `package.edition` is unspecified, defaulting to `2024`
[ERROR] failed to parse lock file at: [ROOT]/foo/Cargo.lock

...
"#
        ]])
        .run();
}

const VALID_LOCKFILE: &str = r#"# Test lockfile
version = 4

[[package]]
name = "test_foo"
version = "0.5.0"
"#;

const LIB_TOML: &str = r#"
        [package]
        name = "test_bar"
        version = "0.1.0"
        edition = "2021"
    "#;

fn make_project() -> ProjectBuilder {
    project()
        .file("Cargo.toml", &basic_bin_manifest("test_foo"))
        .file("src/main.rs", "fn main() {}")
}
