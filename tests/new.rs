extern crate cargo;
extern crate cargotest;
extern crate chrono;
extern crate hamcrest;
extern crate tempdir;

use std::fs::{self, File};
use std::io::prelude::*;
use std::env;

use cargo::util::ProcessBuilder;
use cargotest::process;
use cargotest::support::{execs, paths};
use hamcrest::{assert_that, existing_file, existing_dir, is_not};
use tempdir::TempDir;

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut p = cargotest::cargo_process();
    p.arg(s);
    p
}

#[test]
fn simple_lib() {
    assert_that(cargo_process("new").arg("--lib").arg("foo").arg("--vcs").arg("none")
                                    .env("USER", "foo"),
                execs().with_status(0).with_stderr("\
[CREATED] library `foo` project
"));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());
    assert_that(&paths::root().join("foo/.gitignore"), is_not(existing_file()));

    let lib = paths::root().join("foo/src/lib.rs");
    let mut contents = String::new();
    File::open(&lib).unwrap().read_to_string(&mut contents).unwrap();
    assert_eq!(contents, r#"#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
"#);

    assert_that(cargo_process("build").cwd(&paths::root().join("foo")),
                execs().with_status(0));
}

#[test]
fn simple_bin() {
    assert_that(cargo_process("new").arg("--bin").arg("foo")
                                    .env("USER", "foo"),
                execs().with_status(0).with_stderr("\
[CREATED] binary (application) `foo` project
"));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/main.rs"), existing_file());

    assert_that(cargo_process("build").cwd(&paths::root().join("foo")),
                execs().with_status(0));
    assert_that(&paths::root().join(&format!("foo/target/debug/foo{}",
                                             env::consts::EXE_SUFFIX)),
                existing_file());
}

#[test]
fn both_lib_and_bin() {
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("--lib").arg("--bin").arg("foo").cwd(td.path())
                                    .env("USER", "foo"),
                execs().with_status(101).with_stderr(
                    "[ERROR] can't specify both lib and binary outputs"));
}

#[test]
fn simple_git() {
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("--lib").arg("foo").cwd(td.path())
                                    .env("USER", "foo"),
                execs().with_status(0));

    assert_that(td.path(), existing_dir());
    assert_that(&td.path().join("foo/Cargo.toml"), existing_file());
    assert_that(&td.path().join("foo/src/lib.rs"), existing_file());
    assert_that(&td.path().join("foo/.git"), existing_dir());
    assert_that(&td.path().join("foo/.gitignore"), existing_file());

    assert_that(cargo_process("build").cwd(&td.path().join("foo")),
                execs().with_status(0));
}

#[test]
fn no_argument() {
    assert_that(cargo_process("new"),
                execs().with_status(1)
                       .with_stderr("\
[ERROR] Invalid arguments.

Usage:
    cargo new [options] <path>
    cargo new -h | --help
"));
}

#[test]
fn existing() {
    let dst = paths::root().join("foo");
    fs::create_dir(&dst).unwrap();
    assert_that(cargo_process("new").arg("foo"),
                execs().with_status(101)
                       .with_stderr(format!("[ERROR] destination `{}` already exists\n",
                                            dst.display())));
}

#[test]
fn invalid_characters() {
    assert_that(cargo_process("new").arg("foo.rs"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] Invalid character `.` in crate name: `foo.rs`
use --name to override crate name"));
}

#[test]
fn reserved_name() {
    assert_that(cargo_process("new").arg("test"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] The name `test` cannot be used as a crate name\n\
use --name to override crate name"));
}

#[test]
fn reserved_binary_name() {
    assert_that(cargo_process("new").arg("--bin").arg("incremental"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] The name `incremental` cannot be used as a crate name\n\
use --name to override crate name"));
}

#[test]
fn keyword_name() {
    assert_that(cargo_process("new").arg("pub"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] The name `pub` cannot be used as a crate name\n\
use --name to override crate name"));
}

#[test]
fn rust_prefix_stripped() {
    assert_that(cargo_process("new").arg("--lib").arg("rust-foo").env("USER", "foo"),
                execs().with_status(0)
                       .with_stdout("note: package will be named `foo`; use --name to override"));
    let toml = paths::root().join("rust-foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"name = "foo""#));
}

#[test]
fn bin_disables_stripping() {
    assert_that(cargo_process("new").arg("rust-foo").arg("--bin").env("USER", "foo"),
                execs().with_status(0));
    let toml = paths::root().join("rust-foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"name = "rust-foo""#));
}

#[test]
fn explicit_name_not_stripped() {
    assert_that(cargo_process("new").arg("foo").arg("--name").arg("rust-bar").env("USER", "foo"),
                execs().with_status(0));
    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"name = "rust-bar""#));
}

#[test]
fn finds_author_user() {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo").env("USER", "foo")
                                    .cwd(td.path()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["foo"]"#));
}

#[test]
fn finds_author_user_escaped() {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo").env("USER", "foo \"bar\"")
                                    .cwd(td.path()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["foo \"bar\""]"#));
}

#[test]
fn finds_author_username() {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo")
                                    .env_remove("USER")
                                    .env("USERNAME", "foo")
                                    .cwd(td.path()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["foo"]"#));
}

#[test]
fn finds_author_priority() {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo")
                                    .env("USER", "bar2")
                                    .env("EMAIL", "baz2")
                                    .env("CARGO_NAME", "bar")
                                    .env("CARGO_EMAIL", "baz")
                                    .cwd(td.path()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
}

#[test]
fn finds_author_email() {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo")
                                    .env("USER", "bar")
                                    .env("EMAIL", "baz")
                                    .cwd(td.path()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
}

#[test]
fn finds_author_git() {
    process("git").args(&["config", "--global", "user.name", "bar"])
                  .exec().unwrap();
    process("git").args(&["config", "--global", "user.email", "baz"])
                  .exec().unwrap();
    assert_that(cargo_process("new").arg("foo").env("USER", "foo"),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
}

#[test]
fn finds_git_email() {
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo")
                                    .env("GIT_AUTHOR_NAME", "foo")
                                    .env("GIT_AUTHOR_EMAIL", "gitfoo")
                                    .cwd(td.path()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["foo <gitfoo>"]"#), contents);
}


#[test]
fn finds_git_author() {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo")
                                    .env_remove("USER")
                                    .env("GIT_COMMITTER_NAME", "gitfoo")
                                    .cwd(td.path()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["gitfoo"]"#));
}

#[test]
fn author_prefers_cargo() {
    process("git").args(&["config", "--global", "user.name", "foo"])
                  .exec().unwrap();
    process("git").args(&["config", "--global", "user.email", "bar"])
                  .exec().unwrap();
    let root = paths::root();
    fs::create_dir(&root.join(".cargo")).unwrap();
    File::create(&root.join(".cargo/config")).unwrap().write_all(br#"
        [cargo-new]
        name = "new-foo"
        email = "new-bar"
        vcs = "none"
    "#).unwrap();

    assert_that(cargo_process("new").arg("foo").env("USER", "foo"),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["new-foo <new-bar>"]"#));
    assert!(!root.join("foo/.gitignore").exists());
}

#[test]
fn git_prefers_command_line() {
    let root = paths::root();
    let td = TempDir::new("cargo").unwrap();
    fs::create_dir(&root.join(".cargo")).unwrap();
    File::create(&root.join(".cargo/config")).unwrap().write_all(br#"
        [cargo-new]
        vcs = "none"
        name = "foo"
        email = "bar"
    "#).unwrap();

    assert_that(cargo_process("new").arg("foo").arg("--vcs").arg("git")
                                    .cwd(td.path())
                                    .env("USER", "foo"),
                execs().with_status(0));
    assert!(td.path().join("foo/.gitignore").exists());
}

#[test]
fn subpackage_no_git() {
    assert_that(cargo_process("new").arg("foo").env("USER", "foo"),
                execs().with_status(0));

    let subpackage = paths::root().join("foo").join("components");
    fs::create_dir(&subpackage).unwrap();
    assert_that(cargo_process("new").arg("foo/components/subcomponent")
                                    .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo/components/subcomponent/.git"),
                 is_not(existing_file()));
    assert_that(&paths::root().join("foo/components/subcomponent/.gitignore"),
                 is_not(existing_file()));
}

#[test]
fn subpackage_git_with_vcs_arg() {
    assert_that(cargo_process("new").arg("foo").env("USER", "foo"),
                execs().with_status(0));

    let subpackage = paths::root().join("foo").join("components");
    fs::create_dir(&subpackage).unwrap();
    assert_that(cargo_process("new").arg("foo/components/subcomponent")
                                    .arg("--vcs").arg("git")
                                    .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo/components/subcomponent/.git"),
                 existing_dir());
    assert_that(&paths::root().join("foo/components/subcomponent/.gitignore"),
                 existing_file());
}

#[test]
fn unknown_flags() {
    assert_that(cargo_process("new").arg("foo").arg("--flag"),
                execs().with_status(1)
                       .with_stderr("\
[ERROR] Unknown flag: '--flag'

Usage:
    cargo new [..]
    cargo new [..]
"));
}
