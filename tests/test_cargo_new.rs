use std::fs::{self, File};
use std::io::prelude::*;
use std::env;
use tempdir::TempDir;

use support::{execs, paths};
use support::paths::CargoPathExt;
use hamcrest::{assert_that, existing_file, existing_dir, is_not};

use cargo::util::{process, ProcessBuilder};

fn setup() {
}

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut p = ::cargo_process();
    p.arg(s);
    return p;
}

test!(simple_lib {
    assert_that(cargo_process("new").arg("foo").arg("--vcs").arg("none")
                                    .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());
    assert_that(&paths::root().join("foo/.gitignore"), is_not(existing_file()));

    assert_that(cargo_process("build").cwd(&paths::root().join("foo")),
                execs().with_status(0));
});

test!(simple_bin {
    assert_that(cargo_process("new").arg("foo").arg("--bin")
                                    .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/main.rs"), existing_file());

    assert_that(cargo_process("build").cwd(&paths::root().join("foo")),
                execs().with_status(0));
    assert_that(&paths::root().join(&format!("foo/target/debug/foo{}",
                                             env::consts::EXE_SUFFIX)),
                existing_file());
});

test!(simple_git {
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo").cwd(td.path().clone())
                                    .env("USER", "foo"),
                execs().with_status(0));

    assert_that(td.path(), existing_dir());
    assert_that(&td.path().join("foo/Cargo.toml"), existing_file());
    assert_that(&td.path().join("foo/src/lib.rs"), existing_file());
    assert_that(&td.path().join("foo/.git"), existing_dir());
    assert_that(&td.path().join("foo/.gitignore"), existing_file());

    assert_that(cargo_process("build").cwd(&td.path().clone().join("foo")),
                execs().with_status(0));
});

test!(no_argument {
    assert_that(cargo_process("new"),
                execs().with_status(1)
                       .with_stderr("\
Invalid arguments.

Usage:
    cargo new [options] <path>
    cargo new -h | --help
"));
});

test!(existing {
    let dst = paths::root().join("foo");
    fs::create_dir(&dst).unwrap();
    assert_that(cargo_process("new").arg("foo"),
                execs().with_status(101)
                       .with_stderr(format!("destination `{}` already exists\n",
                                            dst.display())));
});

test!(invalid_characters {
    assert_that(cargo_process("new").arg("foo.rs"),
                execs().with_status(101)
                       .with_stderr("\
Invalid character `.` in crate name: `foo.rs`
use --name to override crate name"));
});

test!(rust_prefix_stripped {
    assert_that(cargo_process("new").arg("rust-foo").env("USER", "foo"),
                execs().with_status(0)
                       .with_stdout("note: package will be named `foo`; use --name to override"));
    let toml = paths::root().join("rust-foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"name = "foo""#));
});

test!(bin_disables_stripping {
    assert_that(cargo_process("new").arg("rust-foo").arg("--bin").env("USER", "foo"),
                execs().with_status(0));
    let toml = paths::root().join("rust-foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"name = "rust-foo""#));
});

test!(explicit_name_not_stripped {
    assert_that(cargo_process("new").arg("foo").arg("--name").arg("rust-bar").env("USER", "foo"),
                execs().with_status(0));
    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"name = "rust-bar""#));
});

test!(finds_author_user {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo").env("USER", "foo")
                                    .cwd(td.path().clone()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["foo"]"#));
});

test!(finds_author_user_escaped {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo").env("USER", "foo \"bar\"")
                                    .cwd(td.path().clone()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["foo \"bar\""]"#));
});

test!(finds_author_username {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo")
                                    .env_remove("USER")
                                    .env("USERNAME", "foo")
                                    .cwd(td.path().clone()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["foo"]"#));
});

test!(finds_author_email {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo")
                                    .env("USER", "bar")
                                    .env("EMAIL", "baz")
                                    .cwd(td.path().clone()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
});

test!(finds_author_git {
    ::process("git").args(&["config", "--global", "user.name", "bar"])
                    .exec().unwrap();
    ::process("git").args(&["config", "--global", "user.email", "baz"])
                    .exec().unwrap();
    assert_that(cargo_process("new").arg("foo").env("USER", "foo"),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["bar <baz>"]"#));
});

test!(author_prefers_cargo {
    ::process("git").args(&["config", "--global", "user.name", "foo"])
                    .exec().unwrap();
    ::process("git").args(&["config", "--global", "user.email", "bar"])
                    .exec().unwrap();
    let root = paths::root();
    fs::create_dir(&root.join(".cargo")).unwrap();
    File::create(&root.join(".cargo/config")).unwrap().write_all(br#"
        [cargo-new]
        name = "new-foo"
        email = "new-bar"
        git = false
    "#).unwrap();

    assert_that(cargo_process("new").arg("foo").env("USER", "foo"),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
    let mut contents = String::new();
    File::open(&toml).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"authors = ["new-foo <new-bar>"]"#));
    assert!(!root.join("foo/.gitignore").c_exists());
});

test!(git_prefers_command_line {
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
    assert!(td.path().join("foo/.gitignore").c_exists());
});

test!(subpackage_no_git {
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
});

test!(subpackage_git_with_vcs_arg {
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
});

test!(unknown_flags {
    assert_that(cargo_process("new").arg("foo").arg("--flag"),
                execs().with_status(1)
                       .with_stderr("\
Unknown flag: '--flag'

Usage:
    cargo new [..]
    cargo new [..]
"));
});
