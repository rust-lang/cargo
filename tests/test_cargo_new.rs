use std::io::{fs, USER_RWX, File, TempDir};
use std::io::fs::PathExtensions;
use std::os;

use support::{execs, paths, cargo_dir, ResultTest};
use hamcrest::{assert_that, existing_file, existing_dir, is_not};

use cargo::util::{process, ProcessBuilder};

fn setup() {
}

fn my_process(s: &str) -> ProcessBuilder {
    process(s)
        .cwd(paths::root())
        .env("HOME", Some(paths::home()))
}

fn cargo_process(s: &str) -> ProcessBuilder {
    process(cargo_dir().join("cargo")).arg(s)
        .cwd(paths::root())
        .env("HOME", Some(paths::home()))
}

test!(simple_lib {
    os::setenv("USER", "foo");
    assert_that(cargo_process("new").arg("foo").arg("--no-git"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());
    assert_that(&paths::root().join("foo/.gitignore"), is_not(existing_file()));

    assert_that(cargo_process("build").cwd(paths::root().join("foo")),
                execs().with_status(0));
})

test!(simple_bin {
    os::setenv("USER", "foo");
    assert_that(cargo_process("new").arg("foo").arg("--bin"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/main.rs"), existing_file());

    assert_that(cargo_process("build").cwd(paths::root().join("foo")),
                execs().with_status(0));
    assert_that(&paths::root().join(format!("foo/target/foo{}",
                                            os::consts::EXE_SUFFIX)),
                existing_file());
})

test!(simple_git {
    let td = TempDir::new("cargo").unwrap();
    os::setenv("USER", "foo");
    assert_that(cargo_process("new").arg("foo").cwd(td.path().clone()),
                execs().with_status(0));

    assert_that(td.path(), existing_dir());
    assert_that(&td.path().join("foo/Cargo.toml"), existing_file());
    assert_that(&td.path().join("foo/src/lib.rs"), existing_file());
    assert_that(&td.path().join("foo/.git"), existing_dir());
    assert_that(&td.path().join("foo/.gitignore"), existing_file());

    assert_that(cargo_process("build").cwd(td.path().clone().join("foo")),
                execs().with_status(0));
})

test!(simple_travis {
    os::setenv("USER", "foo");
    assert_that(cargo_process("new").arg("foo").arg("--travis"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());
    assert_that(&paths::root().join("foo/.travis.yml"), existing_file());

    assert_that(cargo_process("build").cwd(paths::root().join("foo")),
                execs().with_status(0));
})

test!(no_argument {
    assert_that(cargo_process("new"),
                execs().with_status(1)
                       .with_stderr("Invalid arguments.
Usage:
    cargo new [options] <path>
    cargo new -h | --help
"));
})

test!(existing {
    let dst = paths::root().join("foo");
    fs::mkdir(&dst, USER_RWX).assert();
    assert_that(cargo_process("new").arg("foo"),
                execs().with_status(101)
                       .with_stderr(format!("Destination `{}` already exists\n",
                                            dst.display())));
})

test!(invalid_characters {
    assert_that(cargo_process("new").arg("foo.rs"),
                execs().with_status(101)
                       .with_stderr("Invalid character `.` in crate name: `foo.rs`"));
})

test!(finds_author_user {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo").env("USER", Some("foo"))
                                    .cwd(td.path().clone()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let toml = File::open(&toml).read_to_string().assert();
    assert!(toml.as_slice().contains(r#"authors = ["foo"]"#));
})

test!(finds_author_username {
    // Use a temp dir to make sure we don't pick up .cargo/config somewhere in
    // the hierarchy
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("new").arg("foo")
                                    .env("USER", None::<&str>)
                                    .env("USERNAME", Some("foo"))
                                    .cwd(td.path().clone()),
                execs().with_status(0));

    let toml = td.path().join("foo/Cargo.toml");
    let toml = File::open(&toml).read_to_string().assert();
    assert!(toml.as_slice().contains(r#"authors = ["foo"]"#));
})

test!(finds_author_git {
    my_process("git").args(["config", "--global", "user.name", "bar"])
                     .exec().assert();
    my_process("git").args(["config", "--global", "user.email", "baz"])
                     .exec().assert();
    assert_that(cargo_process("new").arg("foo").env("USER", Some("foo")),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
    let toml = File::open(&toml).read_to_string().assert();
    assert!(toml.as_slice().contains(r#"authors = ["bar <baz>"]"#));
})

test!(author_prefers_cargo {
    my_process("git").args(["config", "--global", "user.name", "bar"])
                     .exec().assert();
    my_process("git").args(["config", "--global", "user.email", "baz"])
                     .exec().assert();
    let root = paths::root();
    fs::mkdir(&root.join(".cargo"), USER_RWX).assert();
    File::create(&root.join(".cargo/config")).write_str(r#"
        [cargo-new]
        name = "new-foo"
        email = "new-bar"
        git = false
    "#).assert();

    assert_that(cargo_process("new").arg("foo").env("USER", Some("foo")),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
    let toml = File::open(&toml).read_to_string().assert();
    assert!(toml.as_slice().contains(r#"authors = ["new-foo <new-bar>"]"#));
    assert!(!root.join("foo/.gitignore").exists());
})

test!(git_prefers_command_line {
    let root = paths::root();
    let td = TempDir::new("cargo").unwrap();
    fs::mkdir(&root.join(".cargo"), USER_RWX).assert();
    File::create(&root.join(".cargo/config")).write_str(r#"
        [cargo-new]
        git = false
        name = "foo"
        email = "bar"
    "#).assert();

    assert_that(cargo_process("new").arg("foo").arg("--git").cwd(td.path().clone())
                                    .env("USER", Some("foo")),
                execs().with_status(0));
    assert!(td.path().join("foo/.gitignore").exists());
})

test!(subpackage_no_git {
    os::setenv("USER", "foo");
    assert_that(cargo_process("new").arg("foo"), execs().with_status(0));

    let subpackage = paths::root().join("foo").join("components");
    fs::mkdir(&subpackage, USER_RWX).assert();
    assert_that(cargo_process("new").arg("foo/components/subcomponent"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo/components/subcomponent/.git"),
                 is_not(existing_file()));
    assert_that(&paths::root().join("foo/components/subcomponent/.gitignore"),
                 is_not(existing_file()));
})
