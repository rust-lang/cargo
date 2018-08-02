use std::fs::{self, File};
use std::io::prelude::*;
use std::env;

use support::{cargo_process, git_process};
use support::{execs, paths};
use support::hamcrest::{assert_that, existing_dir, existing_file, is_not};

fn create_empty_gitconfig() {
    // This helps on Windows where libgit2 is very aggressive in attempting to
    // find a git config file.
    let gitconfig = paths::home().join(".gitconfig");
    File::create(gitconfig).unwrap();
}

#[test]
fn simple_lib() {
    assert_that(
        cargo_process("new --lib foo --vcs none").env("USER", "foo"),
        execs().with_stderr("[CREATED] library `foo` project"),
    );

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());
    assert_that(
        &paths::root().join("foo/.gitignore"),
        is_not(existing_file()),
    );

    let lib = paths::root().join("foo/src/lib.rs");
    let mut contents = String::new();
    File::open(&lib)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
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

    assert_that(
        cargo_process("build").cwd(&paths::root().join("foo")),
        execs(),
    );
}

#[test]
fn simple_bin() {
    assert_that(
        cargo_process("new --bin foo").env("USER", "foo"),
        execs().with_stderr("[CREATED] binary (application) `foo` project"),
    );

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/main.rs"), existing_file());

    assert_that(
        cargo_process("build").cwd(&paths::root().join("foo")),
        execs(),
    );
    assert_that(
        &paths::root().join(&format!("foo/target/debug/foo{}", env::consts::EXE_SUFFIX)),
        existing_file(),
    );
}

#[test]
fn both_lib_and_bin() {
    assert_that(
        cargo_process("new --lib --bin foo").env("USER", "foo"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] can't specify both lib and binary outputs"),
    );
}

#[test]
fn simple_git() {
    assert_that(
        cargo_process("new --lib foo").env("USER", "foo"),
        execs(),
    );

    assert_that(&paths::root(), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());
    assert_that(&paths::root().join("foo/.git"), existing_dir());
    assert_that(&paths::root().join("foo/.gitignore"), existing_file());

    assert_that(
        cargo_process("build").cwd(&paths::root().join("foo")),
        execs(),
    );
}

#[test]
fn no_argument() {
    assert_that(
        cargo_process("new"),
        execs().with_status(1).with_stderr_contains(
            "\
error: The following required arguments were not provided:
    <path>
",
        ),
    );
}

#[test]
fn existing() {
    let dst = paths::root().join("foo");
    fs::create_dir(&dst).unwrap();
    assert_that(
        cargo_process("new foo"),
        execs().with_status(101).with_stderr(format!(
            "[ERROR] destination `{}` already exists\n\n\
             Use `cargo init` to initialize the directory",
            dst.display()
        )),
    );
}

#[test]
fn invalid_characters() {
    assert_that(
        cargo_process("new foo.rs"),
        execs().with_status(101).with_stderr(
            "\
[ERROR] Invalid character `.` in crate name: `foo.rs`
use --name to override crate name",
        ),
    );
}

#[test]
fn reserved_name() {
    assert_that(
        cargo_process("new test"),
        execs().with_status(101).with_stderr(
            "\
             [ERROR] The name `test` cannot be used as a crate name\n\
             use --name to override crate name",
        ),
    );
}

#[test]
fn reserved_binary_name() {
    assert_that(
        cargo_process("new --bin incremental"),
        execs().with_status(101).with_stderr(
            "\
             [ERROR] The name `incremental` cannot be used as a crate name\n\
             use --name to override crate name",
        ),
    );
}

#[test]
fn keyword_name() {
    assert_that(
        cargo_process("new pub"),
        execs().with_status(101).with_stderr(
            "\
             [ERROR] The name `pub` cannot be used as a crate name\n\
             use --name to override crate name",
        ),
    );
}

#[test]
fn finds_author_user() {
    create_empty_gitconfig();
    assert_that(
        cargo_process("new foo").env("USER", "foo"),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["foo"]"#));
}

#[test]
fn finds_author_user_escaped() {
    create_empty_gitconfig();
    assert_that(
        cargo_process("new foo").env("USER", "foo \"bar\""),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["foo \"bar\""]"#));
}

#[test]
fn finds_author_username() {
    create_empty_gitconfig();
    assert_that(
        cargo_process("new foo").env_remove("USER").env("USERNAME", "foo"),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["foo"]"#));
}

#[test]
fn finds_author_priority() {
    assert_that(
        cargo_process("new foo")
            .env("USER", "bar2")
            .env("EMAIL", "baz2")
            .env("CARGO_NAME", "bar")
            .env("CARGO_EMAIL", "baz"),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
}

#[test]
fn finds_author_email() {
    create_empty_gitconfig();
    assert_that(
        cargo_process("new foo")
            .env("USER", "bar")
            .env("EMAIL", "baz"),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
}

#[test]
fn finds_author_git() {
    git_process("config --global user.name bar").exec().unwrap();
    git_process("config --global user.email baz").exec().unwrap();
    assert_that(
        cargo_process("new foo").env("USER", "foo"),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
}

#[test]
fn finds_local_author_git() {
    git_process("init").exec().unwrap();
    git_process("config --global user.name foo").exec().unwrap();
    git_process("config --global user.email foo@bar").exec().unwrap();

    // Set local git user config
    git_process("config user.name bar").exec().unwrap();
    git_process("config user.email baz").exec().unwrap();
    assert_that(
        cargo_process("init").env("USER", "foo"),
        execs(),
    );

    let toml = paths::root().join("Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
}

#[test]
fn finds_git_email() {
    assert_that(
        cargo_process("new foo")
            .env("GIT_AUTHOR_NAME", "foo")
            .env("GIT_AUTHOR_EMAIL", "gitfoo"),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["foo <gitfoo>"]"#), contents);
}

#[test]
fn finds_git_author() {
    create_empty_gitconfig();
    assert_that(
        cargo_process("new foo")
            .env_remove("USER")
            .env("GIT_COMMITTER_NAME", "gitfoo"),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["gitfoo"]"#));
}

#[test]
fn author_prefers_cargo() {
    git_process("config --global user.name foo").exec().unwrap();
    git_process("config --global user.email bar").exec().unwrap();
    let root = paths::root();
    fs::create_dir(&root.join(".cargo")).unwrap();
    File::create(&root.join(".cargo/config"))
        .unwrap()
        .write_all(
            br#"
        [cargo-new]
        name = "new-foo"
        email = "new-bar"
        vcs = "none"
    "#,
        )
        .unwrap();

    assert_that(
        cargo_process("new foo").env("USER", "foo"),
        execs(),
    );

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains(r#"authors = ["new-foo <new-bar>"]"#));
    assert!(!root.join("foo/.gitignore").exists());
}

#[test]
fn git_prefers_command_line() {
    let root = paths::root();
    fs::create_dir(&root.join(".cargo")).unwrap();
    File::create(&root.join(".cargo/config"))
        .unwrap()
        .write_all(
            br#"
        [cargo-new]
        vcs = "none"
        name = "foo"
        email = "bar"
    "#,
        )
        .unwrap();

    assert_that(
        cargo_process("new foo --vcs git").env("USER", "foo"),
        execs(),
    );
    assert!(paths::root().join("foo/.gitignore").exists());
}

#[test]
fn subpackage_no_git() {
    assert_that(
        cargo_process("new foo").env("USER", "foo"),
        execs(),
    );

    assert_that(&paths::root().join("foo/.git"), existing_dir());
    assert_that(&paths::root().join("foo/.gitignore"), existing_file());

    let subpackage = paths::root().join("foo").join("components");
    fs::create_dir(&subpackage).unwrap();
    assert_that(
        cargo_process("new foo/components/subcomponent").env("USER", "foo"),
        execs(),
    );

    assert_that(
        &paths::root().join("foo/components/subcomponent/.git"),
        is_not(existing_file()),
    );
    assert_that(
        &paths::root().join("foo/components/subcomponent/.gitignore"),
        is_not(existing_file()),
    );
}

#[test]
fn subpackage_git_with_gitignore() {
    assert_that(
        cargo_process("new foo").env("USER", "foo"),
        execs(),
    );

    assert_that(
        &paths::root().join("foo/.git"),
        existing_dir(),
    );
    assert_that(
        &paths::root().join("foo/.gitignore"),
        existing_file(),
    );

    let gitignore = paths::root().join("foo/.gitignore");
    fs::write(gitignore, b"components").unwrap();

    let subpackage = paths::root().join("foo/components");
    fs::create_dir(&subpackage).unwrap();
    assert_that(
        cargo_process("new foo/components/subcomponent").env("USER", "foo"),
        execs(),
    );

    assert_that(
        &paths::root().join("foo/components/subcomponent/.git"),
        existing_dir(),
    );
    assert_that(
        &paths::root().join("foo/components/subcomponent/.gitignore"),
        existing_file(),
    );

}

#[test]
fn subpackage_git_with_vcs_arg() {
    assert_that(
        cargo_process("new foo").env("USER", "foo"),
        execs(),
    );

    let subpackage = paths::root().join("foo").join("components");
    fs::create_dir(&subpackage).unwrap();
    assert_that(
        cargo_process("new foo/components/subcomponent --vcs git").env("USER", "foo"),
        execs(),
    );

    assert_that(
        &paths::root().join("foo/components/subcomponent/.git"),
        existing_dir(),
    );
    assert_that(
        &paths::root().join("foo/components/subcomponent/.gitignore"),
        existing_file(),
    );
}

#[test]
fn unknown_flags() {
    assert_that(
        cargo_process("new foo --flag"),
        execs().with_status(1).with_stderr_contains(
            "error: Found argument '--flag' which wasn't expected, or isn't valid in this context",
        ),
    );
}

#[test]
fn explicit_invalid_name_not_suggested() {
    assert_that(
        cargo_process("new --name 10-invalid a"),
        execs().with_status(101).with_stderr(
            "[ERROR] Package names starting with a digit cannot be used as a crate name",
        ),
    );
}

#[test]
fn explicit_project_name() {
    assert_that(
        cargo_process("new --lib foo --name bar").env("USER", "foo"),
        execs().with_stderr("[CREATED] library `bar` project"),
    );
}
