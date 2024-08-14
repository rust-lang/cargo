//! Tests for `lockfile-path` flag

use cargo_test_support::compare::assert_e2e;
use cargo_test_support::registry::RegistryBuilder;
use cargo_test_support::{
    basic_bin_manifest, cargo_test, project, symlink_supported, Execs, Project, ProjectBuilder,
};
use snapbox::str;
use std::fs;

const VALID_LOCKFILE: &str = r#"# Test lockfile
version = 3

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
    return project()
        .file("Cargo.toml", &basic_bin_manifest("test_foo"))
        .file("src/main.rs", "fn main() {}");
}

fn make_execs(execs: &mut Execs, lockfile_path_argument: String) -> &mut Execs {
    return execs
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("--lockfile-path")
        .arg(lockfile_path_argument);
}

#[cargo_test]
fn basic_lockfile_created() {
    let lockfile_path_argument = "mylockfile/is/burried/Cargo.lock";
    let p = make_project().build();

    make_execs(&mut p.cargo("metadata"), lockfile_path_argument.to_string()).run();
    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn basic_lockfile_read() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("mylockfile/Cargo.lock", VALID_LOCKFILE)
        .build();

    make_execs(&mut p.cargo("metadata"), lockfile_path_argument.to_string()).run();

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn basic_lockfile_override() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();

    make_execs(&mut p.cargo("metadata"), lockfile_path_argument.to_string()).run();

    assert!(p.root().join(lockfile_path_argument).is_file());
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
    let lockfile_path_argument = format!("{src}/Cargo.lock");

    let p = make_project().symlink_dir(dst, src).build();

    fs::create_dir(p.root().join("dst"))
        .unwrap_or_else(|e| panic!("could not create directory {}", e));
    assert!(p.root().join(src).is_dir());

    make_execs(&mut p.cargo("metadata"), lockfile_path_argument.to_string()).run();

    assert!(p.root().join(format!("{src}/Cargo.lock")).is_file());
    assert!(p.root().join(lockfile_path_argument).is_file());
    assert!(p.root().join(dst).join("Cargo.lock").is_file());
}

#[cargo_test]
fn symlink_lockfile() {
    if !symlink_supported() {
        return;
    }

    let lockfile_path_argument = "dst/Cargo.lock";
    let src = "somedir/link";
    let lock_body = VALID_LOCKFILE;

    let p = make_project()
        .file(lockfile_path_argument, lock_body)
        .symlink(lockfile_path_argument, src)
        .build();

    assert!(p.root().join(src).is_file());

    make_execs(&mut p.cargo("metadata"), lockfile_path_argument.to_string()).run();

    assert!(!p.root().join("Cargo.lock").exists());
}

#[cargo_test]
fn broken_symlink() {
    if !symlink_supported() {
        return;
    }

    let invalid_dst = "invalid_path";
    let src = "somedir/link";
    let lockfile_path_argument = format!("{src}/Cargo.lock");

    let p = make_project().symlink_dir(invalid_dst, src).build();
    assert!(!p.root().join(src).is_dir());

    let err_msg = if !cfg!(windows) {
        str![[
            r#"[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[ERROR] failed to create directory `[ROOT]/foo/somedir/link`

Caused by:
  File exists (os error 17)

"#
        ]]
    } else {
        str![[
            r#"[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[ERROR] failed to create directory `[ROOT]/foo/somedir/link`

Caused by:
  Cannot create a file when that file already exists. (os error 183)

"#
        ]]
    };

    make_execs(&mut p.cargo("metadata"), lockfile_path_argument.to_string())
        .with_status(101)
        .with_stderr_data(err_msg)
        .run();
}

#[cargo_test]
fn loop_symlink() {
    if !symlink_supported() {
        return;
    }

    let loop_link = "loop";
    let src = "somedir/link";
    let lockfile_path_argument = format!("{src}/Cargo.lock");

    let p = make_project()
        .symlink_dir(loop_link, src)
        .symlink_dir(src, loop_link)
        .build();
    assert!(!p.root().join(src).is_dir());

    let err_msg = if !cfg!(windows) {
        str![[
            r#"[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[ERROR] failed to create directory `[ROOT]/foo/somedir/link`

Caused by:
  File exists (os error 17)

"#
        ]]
    } else {
        str![[
            r#"[WARNING] please specify `--format-version` flag explicitly to avoid compatibility problems
[ERROR] failed to create directory `[ROOT]/foo/somedir/link`

Caused by:
  Cannot create a file when that file already exists. (os error 183)

"#
        ]]
    };

    make_execs(&mut p.cargo("metadata"), lockfile_path_argument.to_string())
        .with_status(101)
        .with_stderr_data(err_msg)
        .run();
}

fn run_add_command(p: &Project, lockfile_path_argument: String) {
    make_execs(&mut p.cargo("add"), lockfile_path_argument)
        .arg("--path")
        .arg("../bar")
        .run();
}

fn make_add_project() -> ProjectBuilder {
    return make_project()
        .file("../bar/Cargo.toml", LIB_TOML)
        .file("../bar/src/main.rs", "fn main() {}");
}

/////////////////////////
//// Commands tests
/////////////////////////

#[cargo_test]
fn add_lockfile_created() {
    let lockfile_path_argument = "mylockfile/is/burried/Cargo.lock";
    let p = make_add_project().build();
    run_add_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn add_lockfile_read() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_add_project()
        .file("mylockfile/Cargo.lock", VALID_LOCKFILE)
        .build();
    run_add_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn add_lockfile_override() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_add_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    run_add_command(&p, lockfile_path_argument.to_string());

    assert!(p.root().join(lockfile_path_argument).is_file());
}

fn run_clean_command(p: &Project, lockfile_path_argument: String) {
    make_execs(&mut p.cargo("clean"), lockfile_path_argument)
        .arg("--package")
        .arg("test_foo")
        .run();
}

#[cargo_test]
fn clean_lockfile_created() {
    let lockfile_path_argument = "mylockfile/is/burried/Cargo.lock";
    let p = make_project().build();
    run_clean_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn clean_lockfile_read() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("mylockfile/Cargo.lock", VALID_LOCKFILE)
        .build();
    run_clean_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn clean_lockfile_override() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    run_clean_command(&p, lockfile_path_argument.to_string());

    assert!(p.root().join(lockfile_path_argument).is_file());
}

fn run_fix_command(p: &Project, lockfile_path_argument: String) {
    make_execs(&mut p.cargo("fix"), lockfile_path_argument)
        .arg("--package")
        .arg("test_foo")
        .arg("--allow-no-vcs")
        .run();
}

#[cargo_test]
fn fix_lockfile_created() {
    let lockfile_path_argument = "mylockfile/is/burried/Cargo.lock";
    let p = make_project().build();
    run_fix_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn fix_lockfile_read() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("mylockfile/Cargo.lock", VALID_LOCKFILE)
        .build();
    run_fix_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn fix_lockfile_override() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    run_fix_command(&p, lockfile_path_argument.to_string());

    assert!(p.root().join(lockfile_path_argument).is_file());
}

fn run_publish_command(p: &Project, lockfile_path_argument: String) {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    make_execs(&mut p.cargo("publish"), lockfile_path_argument)
        .replace_crates_io(registry.index_url())
        .run();
}

#[cargo_test]
fn publish_lockfile_read() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_project()
        .file("mylockfile/Cargo.lock", VALID_LOCKFILE)
        .build();
    run_publish_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

fn make_remove_project() -> ProjectBuilder {
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

    return project()
        .file("Cargo.toml", &manifest)
        .file("src/main.rs", "fn main() {}")
        .file("../bar/Cargo.toml", LIB_TOML)
        .file("../bar/src/main.rs", "fn main() {}");
}

fn run_remove_command(p: &Project, lockfile_path_argument: String) {
    make_execs(&mut p.cargo("remove"), lockfile_path_argument)
        .arg("test_bar")
        .run();
}
#[cargo_test]
fn remove_lockfile_created() {
    let lockfile_path_argument = "mylockfile/is/burried/Cargo.lock";
    let p = make_remove_project().build();
    run_remove_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn remove_lockfile_read() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_remove_project()
        .file("mylockfile/Cargo.lock", VALID_LOCKFILE)
        .build();
    run_remove_command(&p, lockfile_path_argument.to_string());

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn remove_lockfile_override() {
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = make_remove_project()
        .file("Cargo.lock", "This is an invalid lock file!")
        .build();
    run_remove_command(&p, lockfile_path_argument.to_string());

    assert!(p.root().join(lockfile_path_argument).is_file());
}

#[cargo_test]
fn assert_respect_pinned_version_from_lockfile_path() {
    const PINNED_VERSION: &str = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "hello"
version = "1.0.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "2b94f16c310ebbd9efcca5a5a17131d70bd454876f2af007f3da211afadff4fc"

[[package]]
name = "test_foo"
version = "0.5.0"
dependencies = [
 "hello",
]
"#;
    const TOML: &str = r#"#
[package]

name = "test_foo"
version = "0.5.0"
authors = ["wycats@example.com"]
edition = "2015"

[[bin]]

name = "test_foo"

[dependencies]
hello = "1.0.0"
"#;

    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = project()
        .file("Cargo.toml", TOML)
        .file("src/main.rs", "fn main() {}")
        .file("mylockfile/Cargo.lock", PINNED_VERSION)
        .build();

    make_execs(&mut p.cargo("package"), lockfile_path_argument.to_string()).run();

    assert!(!p.root().join("Cargo.lock").exists());
    assert!(p.root().join(lockfile_path_argument).is_file());

    assert!(p
        .root()
        .join("target/package/test_foo-0.5.0/Cargo.lock")
        .is_file());

    let path = p.root().join("target/package/test_foo-0.5.0/Cargo.lock");
    let contents = fs::read_to_string(path).unwrap();

    assert_e2e().eq(contents, PINNED_VERSION);
}

#[cargo_test]
fn run_embed() {
    const ECHO_SCRIPT: &str = r#"#!/usr/bin/env cargo

fn main() {
    let mut args = std::env::args_os();
    let bin = args.next().unwrap().to_str().unwrap().to_owned();
    let args = args.collect::<Vec<_>>();
    println!("bin: {bin}");
    println!("args: {args:?}");
}

#[test]
fn test () {}
"#;
    let lockfile_path_argument = "mylockfile/Cargo.lock";
    let p = project().file("src/main.rs", ECHO_SCRIPT).build();

    p.cargo("run")
        .masquerade_as_nightly_cargo(&["lockfile-path"])
        .arg("-Zunstable-options")
        .arg("-Zscript")
        .arg("--lockfile-path")
        .arg(lockfile_path_argument)
        .arg("--manifest-path")
        .arg("src/main.rs")
        .run();

    assert!(p.root().join(lockfile_path_argument).is_file());
    assert!(!p.root().join("Cargo.lock").exists());
}
