//! Tests for the `cargo new` command.

use std::env;
use std::fs::{self, File};

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::paths;
use cargo_test_support::str;

fn create_default_gitconfig() {
    // This helps on Windows where libgit2 is very aggressive in attempting to
    // find a git config file.
    let gitconfig = paths::home().join(".gitconfig");
    File::create(gitconfig).unwrap();

    // If we're running this under a user account that has a different default branch set up
    // then tests that assume the default branch is master will fail. We set the default branch
    // to master explicitly so that tests that rely on this behavior still pass.
    fs::write(
        paths::home().join(".gitconfig"),
        r#"
        [init]
            defaultBranch = master
        "#,
    )
    .unwrap();
}

#[cargo_test]
fn simple_lib() {
    cargo_process("new --lib foo --vcs none --edition 2015")
        .with_stderr_data(str![[r#"
[CREATING] library `foo` package
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]])
        .run();

    assert!(paths::root().join("foo").is_dir());
    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(paths::root().join("foo/src/lib.rs").is_file());
    assert!(!paths::root().join("foo/.gitignore").is_file());

    let lib = paths::root().join("foo/src/lib.rs");
    let contents = fs::read_to_string(&lib).unwrap();
    assert_eq!(
        contents,
        r#"pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
"#
    );

    cargo_process("build").cwd(&paths::root().join("foo")).run();
}

#[cargo_test]
fn simple_bin() {
    cargo_process("new --bin foo --edition 2015")
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `foo` package
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]])
        .run();

    assert!(paths::root().join("foo").is_dir());
    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(paths::root().join("foo/src/main.rs").is_file());

    cargo_process("build").cwd(&paths::root().join("foo")).run();
    assert!(
        paths::root()
            .join(&format!("foo/target/debug/foo{}", env::consts::EXE_SUFFIX))
            .is_file()
    );
}

#[cargo_test]
fn both_lib_and_bin() {
    cargo_process("new --lib --bin foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] can't specify both lib and binary outputs

"#]])
        .run();
}

#[cargo_test]
fn simple_git() {
    cargo_process("new --lib foo --edition 2015").run();

    assert!(paths::root().is_dir());
    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(paths::root().join("foo/src/lib.rs").is_file());
    assert!(paths::root().join("foo/.git").is_dir());
    assert!(paths::root().join("foo/.gitignore").is_file());

    let fp = paths::root().join("foo/.gitignore");
    let contents = fs::read_to_string(&fp).unwrap();
    assert_eq!(contents, "/target\n",);

    cargo_process("build").cwd(&paths::root().join("foo")).run();
}

#[cargo_test(requires = "hg")]
fn simple_hg() {
    cargo_process("new --lib foo --edition 2015 --vcs hg").run();

    assert!(paths::root().is_dir());
    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(paths::root().join("foo/src/lib.rs").is_file());
    assert!(paths::root().join("foo/.hg").is_dir());
    assert!(paths::root().join("foo/.hgignore").is_file());

    let fp = paths::root().join("foo/.hgignore");
    let contents = fs::read_to_string(&fp).unwrap();
    assert_eq!(contents, "^target$\n",);

    cargo_process("build").cwd(&paths::root().join("foo")).run();
}

#[cargo_test]
fn no_argument() {
    cargo_process("new")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the following required arguments were not provided:
  <PATH>
...
"#]])
        .run();
}

#[cargo_test]
fn existing() {
    let dst = paths::root().join("foo");
    fs::create_dir(&dst).unwrap();
    cargo_process("new foo")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `foo` package
[ERROR] destination `[ROOT]/foo` already exists

Use `cargo init` to initialize the directory

"#]])
        .run();
}

#[cargo_test]
fn invalid_characters() {
    cargo_process("new foo.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `foo.rs` package
[ERROR] invalid character `.` in package name: `foo.rs`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name "foo.rs", use a valid package name, and set the binary name to be different from the package. This can be done by setting the binary filename to `src/bin/foo.rs.rs` or change the name in Cargo.toml with:

    [[bin]]
    name = "foo.rs"
    path = "src/main.rs"


"#]])
        .run();
}

#[cargo_test]
fn reserved_name() {
    cargo_process("new test")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `test` package
[ERROR] the name `test` cannot be used as a package name, it conflicts with Rust's built-in test library
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name "test", use a valid package name, and set the binary name to be different from the package. This can be done by setting the binary filename to `src/bin/test.rs` or change the name in Cargo.toml with:

    [[bin]]
    name = "test"
    path = "src/main.rs"


"#]])
        .run();
}

#[cargo_test]
fn reserved_binary_name() {
    cargo_process("new --bin incremental")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `incremental` package
[ERROR] the name `incremental` cannot be used as a package name, it conflicts with cargo's build directory names
If you need a package name to not match the directory name, consider using --name flag.

"#]])
        .run();

    cargo_process("new --lib incremental")
        .with_stderr_data(str![[r#"
[CREATING] library `incremental` package
[WARNING] the name `incremental` will not support binary executables with that name, it conflicts with cargo's build directory names
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]])
        .run();
}

#[cargo_test]
fn keyword_name() {
    cargo_process("new pub")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `pub` package
[ERROR] the name `pub` cannot be used as a package name, it is a Rust keyword
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name "pub", use a valid package name, and set the binary name to be different from the package. This can be done by setting the binary filename to `src/bin/pub.rs` or change the name in Cargo.toml with:

    [[bin]]
    name = "pub"
    path = "src/main.rs"


"#]])
        .run();
}

#[cargo_test]
fn std_name() {
    cargo_process("new core").with_stderr_data(str![[r#"
[CREATING] binary (application) `core` package
[WARNING] the name `core` is part of Rust's standard library
It is recommended to use a different name to avoid problems.
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name "core", use a valid package name, and set the binary name to be different from the package. This can be done by setting the binary filename to `src/bin/core.rs` or change the name in Cargo.toml with:

    [[bin]]
    name = "core"
    path = "src/main.rs"

[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]]).run();
}

#[cargo_test]
fn git_prefers_command_line() {
    let root = paths::root();
    fs::create_dir(&root.join(".cargo")).unwrap();
    fs::write(
        &root.join(".cargo/config.toml"),
        r#"
            [cargo-new]
            vcs = "none"
            name = "foo"
            email = "bar"
        "#,
    )
    .unwrap();

    cargo_process("new foo --vcs git").run();
    assert!(paths::root().join("foo/.gitignore").exists());
    assert!(
        !fs::read_to_string(paths::root().join("foo/Cargo.toml"))
            .unwrap()
            .contains("authors =")
    );
}

#[cargo_test]
fn subpackage_no_git() {
    cargo_process("new foo").run();

    assert!(paths::root().join("foo/.git").is_dir());
    assert!(paths::root().join("foo/.gitignore").is_file());

    let subpackage = paths::root().join("foo").join("components");
    fs::create_dir(&subpackage).unwrap();
    cargo_process("new foo/components/subcomponent").run();

    assert!(
        !paths::root()
            .join("foo/components/subcomponent/.git")
            .is_file()
    );
    assert!(
        !paths::root()
            .join("foo/components/subcomponent/.gitignore")
            .is_file()
    );
}

#[cargo_test]
fn subpackage_git_with_gitignore() {
    cargo_process("new foo").run();

    assert!(paths::root().join("foo/.git").is_dir());
    assert!(paths::root().join("foo/.gitignore").is_file());

    let gitignore = paths::root().join("foo/.gitignore");
    fs::write(gitignore, b"components").unwrap();

    let subpackage = paths::root().join("foo/components");
    fs::create_dir(&subpackage).unwrap();
    cargo_process("new foo/components/subcomponent").run();

    assert!(
        paths::root()
            .join("foo/components/subcomponent/.git")
            .is_dir()
    );
    assert!(
        paths::root()
            .join("foo/components/subcomponent/.gitignore")
            .is_file()
    );
}

#[cargo_test]
fn subpackage_git_with_vcs_arg() {
    cargo_process("new foo").run();

    let subpackage = paths::root().join("foo").join("components");
    fs::create_dir(&subpackage).unwrap();
    cargo_process("new foo/components/subcomponent --vcs git").run();

    assert!(
        paths::root()
            .join("foo/components/subcomponent/.git")
            .is_dir()
    );
    assert!(
        paths::root()
            .join("foo/components/subcomponent/.gitignore")
            .is_file()
    );
}

#[cargo_test]
fn unknown_flags() {
    cargo_process("new foo --flag")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '--flag' found
...
"#]])
        .run();
}

#[cargo_test]
fn explicit_invalid_name_not_suggested() {
    cargo_process("new --name 10-invalid a")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `10-invalid` package
[ERROR] invalid character `1` in package name: `10-invalid`, the name cannot start with a digit
If you need a binary with the name "10-invalid", use a valid package name, and set the binary name to be different from the package. This can be done by setting the binary filename to `src/bin/10-invalid.rs` or change the name in Cargo.toml with:

    [[bin]]
    name = "10-invalid"
    path = "src/main.rs"


"#]])
        .run();
}

#[cargo_test]
fn explicit_project_name() {
    cargo_process("new --lib foo --name bar")
        .with_stderr_data(str![[r#"
[CREATING] library `bar` package
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]])
        .run();
}

#[cargo_test]
fn new_with_edition_2015() {
    cargo_process("new --edition 2015 foo").run();
    let manifest = fs::read_to_string(paths::root().join("foo/Cargo.toml")).unwrap();
    assert!(manifest.contains("edition = \"2015\""));
}

#[cargo_test]
fn new_with_edition_2018() {
    cargo_process("new --edition 2018 foo").run();
    let manifest = fs::read_to_string(paths::root().join("foo/Cargo.toml")).unwrap();
    assert!(manifest.contains("edition = \"2018\""));
}

#[cargo_test]
fn new_default_edition() {
    cargo_process("new foo").run();
    let manifest = fs::read_to_string(paths::root().join("foo/Cargo.toml")).unwrap();
    assert!(manifest.contains("edition = \"2024\""));
}

#[cargo_test]
fn new_with_bad_edition() {
    cargo_process("new --edition something_else foo")
        .with_stderr_data(str![[r#"
[ERROR] invalid value 'something_else' for '--edition <YEAR>'
...
"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn lockfile_constant_during_new() {
    cargo_process("new foo").run();

    cargo_process("build").cwd(&paths::root().join("foo")).run();
    let before = fs::read_to_string(paths::root().join("foo/Cargo.lock")).unwrap();
    cargo_process("build").cwd(&paths::root().join("foo")).run();
    let after = fs::read_to_string(paths::root().join("foo/Cargo.lock")).unwrap();
    assert_eq!(before, after);
}

#[cargo_test]
fn restricted_windows_name() {
    if cfg!(windows) {
        cargo_process("new nul")
            .with_status(101)
            .with_stderr_data(str![[r#"
[CREATING] binary (application) `nul` package
[ERROR] cannot use name `nul`, it is a reserved Windows filename
If you need a package name to not match the directory name, consider using --name flag.

"#]])
            .run();
    } else {
        cargo_process("new nul").with_stderr_data(str![[r#"
[CREATING] binary (application) `nul` package
[WARNING] the name `nul` is a reserved Windows filename
This package will not work on Windows platforms.
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]]).run();
    }
}

#[cargo_test]
fn non_ascii_name() {
    cargo_process("new Привет").with_stderr_data(str![[r#"
[CREATING] binary (application) `Привет` package
[WARNING] the name `Привет` contains non-ASCII characters
Non-ASCII crate names are not supported by Rust.
[WARNING] the name `Привет` is not snake_case or kebab-case which is recommended for package names, consider `привет`
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]]).run();
}

#[cargo_test]
fn non_ascii_name_invalid() {
    // These are alphanumeric characters, but not Unicode XID.
    cargo_process("new ⒶⒷⒸ")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `ⒶⒷⒸ` package
[ERROR] invalid character `Ⓐ` in package name: `ⒶⒷⒸ`, the first character must be a Unicode XID start character (most letters or `_`)
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name "ⒶⒷⒸ", use a valid package name, and set the binary name to be different from the package. This can be done by setting the binary filename to `src/bin/ⒶⒷⒸ.rs` or change the name in Cargo.toml with:

    [[bin]]
    name = "ⒶⒷⒸ"
    path = "src/main.rs"


"#]])
        .run();

    cargo_process("new a¼")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `a¼` package
[ERROR] invalid character `¼` in package name: `a¼`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name "a¼", use a valid package name, and set the binary name to be different from the package. This can be done by setting the binary filename to `src/bin/a¼.rs` or change the name in Cargo.toml with:

    [[bin]]
    name = "a¼"
    path = "src/main.rs"


"#]])
        .run();
}

#[cargo_test]
fn non_snake_case_name() {
    cargo_process("new UPPERcase_name")
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `UPPERcase_name` package
[WARNING] the name `UPPERcase_name` is not snake_case or kebab-case which is recommended for package names, consider `uppercase_name`
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]])
        .run();
}

#[cargo_test]
fn kebab_case_name_is_accepted() {
    cargo_process("new kebab-case-is-valid")
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `kebab-case-is-valid` package
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]])
        .run();
}

#[cargo_test]
fn git_default_branch() {
    // Check for init.defaultBranch support.
    create_default_gitconfig();

    cargo_process("new foo").run();
    let repo = git2::Repository::open(paths::root().join("foo")).unwrap();
    let head = repo.find_reference("HEAD").unwrap();
    assert_eq!(head.symbolic_target().unwrap(), "refs/heads/master");

    fs::write(
        paths::home().join(".gitconfig"),
        r#"
        [init]
            defaultBranch = hello
        "#,
    )
    .unwrap();
    cargo_process("new bar").run();
    let repo = git2::Repository::open(paths::root().join("bar")).unwrap();
    let head = repo.find_reference("HEAD").unwrap();
    assert_eq!(head.symbolic_target().unwrap(), "refs/heads/hello");
}

#[cargo_test]
fn non_utf8_str_in_ignore_file() {
    let gitignore = paths::home().join(".gitignore");
    File::create(gitignore).unwrap();

    fs::write(paths::home().join(".gitignore"), &[0xFF, 0xFE]).unwrap();

    cargo_process(&format!("init {} --vcs git", paths::home().display()))
        .with_status(101)
        .with_stderr_data(str![[r#"
[CREATING] binary (application) package
[ERROR] Failed to create package `home` at `[ROOT]/home`

Caused by:
  Character at line 0 is invalid. Cargo only supports UTF-8.

"#]])
        .run();
}

#[cfg(unix)]
#[cargo_test]
fn path_with_invalid_character() {
    cargo_process("new --name testing test:ing")
        .with_stderr_data(str![[r#"
[CREATING] binary (application) `testing` package
[WARNING] the path `[ROOT]/test:ing` contains invalid PATH characters (usually `:`, `;`, or `"`)
It is recommended to use a different name to avoid problems.
[NOTE] see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]])
        .run();
}
