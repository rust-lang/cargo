//! Tests for the `cargo new` command.

use cargo_test_support::paths;
use cargo_test_support::{cargo_process, rustc_host};
use std::env;
use std::fs::{self, File};

fn create_empty_gitconfig() {
    // This helps on Windows where libgit2 is very aggressive in attempting to
    // find a git config file.
    let gitconfig = paths::home().join(".gitconfig");
    File::create(gitconfig).unwrap();
}

#[cargo_test]
fn simple_lib() {
    cargo_process("new --lib foo --vcs none --edition 2015")
        .with_stderr("[CREATED] library `foo` package")
        .run();

    assert!(paths::root().join("foo").is_dir());
    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(paths::root().join("foo/src/lib.rs").is_file());
    assert!(!paths::root().join("foo/.gitignore").is_file());

    let lib = paths::root().join("foo/src/lib.rs");
    let contents = fs::read_to_string(&lib).unwrap();
    assert_eq!(
        contents,
        r#"#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
"#
    );

    cargo_process("build").cwd(&paths::root().join("foo")).run();
}

#[cargo_test]
fn simple_bin() {
    cargo_process("new --bin foo --edition 2015")
        .with_stderr("[CREATED] binary (application) `foo` package")
        .run();

    assert!(paths::root().join("foo").is_dir());
    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(paths::root().join("foo/src/main.rs").is_file());

    cargo_process("build").cwd(&paths::root().join("foo")).run();
    assert!(paths::root()
        .join(&format!(
            "foo/target/{}/debug/foo{}",
            rustc_host(),
            env::consts::EXE_SUFFIX
        ))
        .is_file());
}

#[cargo_test]
fn both_lib_and_bin() {
    cargo_process("new --lib --bin foo")
        .with_status(101)
        .with_stderr("[ERROR] can't specify both lib and binary outputs")
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
    assert_eq!(contents, "/target\nCargo.lock\n",);

    cargo_process("build").cwd(&paths::root().join("foo")).run();
}

#[cargo_test]
fn no_argument() {
    cargo_process("new")
        .with_status(1)
        .with_stderr_contains(
            "\
error: The following required arguments were not provided:
    <path>
",
        )
        .run();
}

#[cargo_test]
fn existing() {
    let dst = paths::root().join("foo");
    fs::create_dir(&dst).unwrap();
    cargo_process("new foo")
        .with_status(101)
        .with_stderr(
            "[ERROR] destination `[CWD]/foo` already exists\n\n\
             Use `cargo init` to initialize the directory",
        )
        .run();
}

#[cargo_test]
fn invalid_characters() {
    cargo_process("new foo.rs")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `.` in package name: `foo.rs`, [..]
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name \"foo.rs\", use a valid package name, \
and set the binary name to be different from the package. \
This can be done by setting the binary filename to `src/bin/foo.rs.rs` \
or change the name in Cargo.toml with:

    [bin]
    name = \"foo.rs\"
    path = \"src/main.rs\"

",
        )
        .run();
}

#[cargo_test]
fn reserved_name() {
    cargo_process("new test")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the name `test` cannot be used as a package name, it conflicts [..]
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name \"test\", use a valid package name, \
and set the binary name to be different from the package. \
This can be done by setting the binary filename to `src/bin/test.rs` \
or change the name in Cargo.toml with:

    [bin]
    name = \"test\"
    path = \"src/main.rs\"

",
        )
        .run();
}

#[cargo_test]
fn reserved_binary_name() {
    cargo_process("new --bin incremental")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the name `incremental` cannot be used as a package name, it conflicts [..]
If you need a package name to not match the directory name, consider using --name flag.
",
        )
        .run();

    cargo_process("new --lib incremental")
        .with_stderr(
            "\
[WARNING] the name `incremental` will not support binary executables with that name, \
it conflicts with cargo's build directory names
[CREATED] library `incremental` package
",
        )
        .run();
}

#[cargo_test]
fn keyword_name() {
    cargo_process("new pub")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the name `pub` cannot be used as a package name, it is a Rust keyword
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name \"pub\", use a valid package name, \
and set the binary name to be different from the package. \
This can be done by setting the binary filename to `src/bin/pub.rs` \
or change the name in Cargo.toml with:

    [bin]
    name = \"pub\"
    path = \"src/main.rs\"

",
        )
        .run();
}

#[cargo_test]
fn std_name() {
    cargo_process("new core")
        .with_stderr(
            "\
[WARNING] the name `core` is part of Rust's standard library
It is recommended to use a different name to avoid problems.
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name \"core\", use a valid package name, \
and set the binary name to be different from the package. \
This can be done by setting the binary filename to `src/bin/core.rs` \
or change the name in Cargo.toml with:

    [bin]
    name = \"core\"
    path = \"src/main.rs\"

[CREATED] binary (application) `core` package
",
        )
        .run();
}

#[cargo_test]
fn git_prefers_command_line() {
    let root = paths::root();
    fs::create_dir(&root.join(".cargo")).unwrap();
    fs::write(
        &root.join(".cargo/config"),
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
    assert!(!fs::read_to_string(paths::root().join("foo/Cargo.toml"))
        .unwrap()
        .contains("authors ="));
}

#[cargo_test]
fn subpackage_no_git() {
    cargo_process("new foo").run();

    assert!(paths::root().join("foo/.git").is_dir());
    assert!(paths::root().join("foo/.gitignore").is_file());

    let subpackage = paths::root().join("foo").join("components");
    fs::create_dir(&subpackage).unwrap();
    cargo_process("new foo/components/subcomponent").run();

    assert!(!paths::root()
        .join("foo/components/subcomponent/.git")
        .is_file());
    assert!(!paths::root()
        .join("foo/components/subcomponent/.gitignore")
        .is_file());
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

    assert!(paths::root()
        .join("foo/components/subcomponent/.git")
        .is_dir());
    assert!(paths::root()
        .join("foo/components/subcomponent/.gitignore")
        .is_file());
}

#[cargo_test]
fn subpackage_git_with_vcs_arg() {
    cargo_process("new foo").run();

    let subpackage = paths::root().join("foo").join("components");
    fs::create_dir(&subpackage).unwrap();
    cargo_process("new foo/components/subcomponent --vcs git").run();

    assert!(paths::root()
        .join("foo/components/subcomponent/.git")
        .is_dir());
    assert!(paths::root()
        .join("foo/components/subcomponent/.gitignore")
        .is_file());
}

#[cargo_test]
fn unknown_flags() {
    cargo_process("new foo --flag")
        .with_status(1)
        .with_stderr_contains(
            "error: Found argument '--flag' which wasn't expected, or isn't valid in this context",
        )
        .run();
}

#[cargo_test]
fn explicit_invalid_name_not_suggested() {
    cargo_process("new --name 10-invalid a")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the name `10-invalid` cannot be used as a package name, \
the name cannot start with a digit\n\
If you need a binary with the name \"10-invalid\", use a valid package name, \
and set the binary name to be different from the package. \
This can be done by setting the binary filename to `src/bin/10-invalid.rs` \
or change the name in Cargo.toml with:

    [bin]
    name = \"10-invalid\"
    path = \"src/main.rs\"

",
        )
        .run();
}

#[cargo_test]
fn explicit_project_name() {
    cargo_process("new --lib foo --name bar")
        .with_stderr("[CREATED] library `bar` package")
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
    assert!(manifest.contains("edition = \"2018\""));
}

#[cargo_test]
fn new_with_bad_edition() {
    cargo_process("new --edition something_else foo")
        .with_stderr_contains("error: 'something_else' isn't a valid value[..]")
        .with_status(1)
        .run();
}

#[cargo_test]
fn new_with_reference_link() {
    cargo_process("new foo").run();

    let contents = fs::read_to_string(paths::root().join("foo/Cargo.toml")).unwrap();
    assert!(contents.contains("# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html"))
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
            .with_stderr(
                "\
[ERROR] cannot use name `nul`, it is a reserved Windows filename
If you need a package name to not match the directory name, consider using --name flag.
",
            )
            .run();
    } else {
        cargo_process("new nul")
            .with_stderr(
                "\
[WARNING] the name `nul` is a reserved Windows filename
This package will not work on Windows platforms.
[CREATED] binary (application) `nul` package
",
            )
            .run();
    }
}

#[cargo_test]
fn non_ascii_name() {
    cargo_process("new Привет")
        .with_stderr(
            "\
[WARNING] the name `Привет` contains non-ASCII characters
Support for non-ASCII crate names is experimental and only valid on the nightly toolchain.
[CREATED] binary (application) `Привет` package
",
        )
        .run();
}

#[cargo_test]
fn non_ascii_name_invalid() {
    // These are alphanumeric characters, but not Unicode XID.
    cargo_process("new ⒶⒷⒸ")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `Ⓐ` in package name: `ⒶⒷⒸ`, \
the first character must be a Unicode XID start character (most letters or `_`)
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name \"ⒶⒷⒸ\", use a valid package name, \
and set the binary name to be different from the package. \
This can be done by setting the binary filename to `src/bin/ⒶⒷⒸ.rs` \
or change the name in Cargo.toml with:

    [bin]
    name = \"ⒶⒷⒸ\"
    path = \"src/main.rs\"

",
        )
        .run();

    cargo_process("new a¼")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `¼` in package name: `a¼`, \
characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name \"a¼\", use a valid package name, \
and set the binary name to be different from the package. \
This can be done by setting the binary filename to `src/bin/a¼.rs` \
or change the name in Cargo.toml with:

    [bin]
    name = \"a¼\"
    path = \"src/main.rs\"

",
        )
        .run();
}

#[cargo_test]
fn git_default_branch() {
    // Check for init.defaultBranch support.
    create_empty_gitconfig();
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
