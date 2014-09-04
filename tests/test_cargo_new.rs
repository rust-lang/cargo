use std::io::{fs, UserRWX, File};
use std::os;

use support::{execs, paths, cargo_dir, ResultTest};
use hamcrest::{assert_that, existing_file, existing_dir};

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
    assert_that(cargo_process("new").arg("foo"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());

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
    os::setenv("USER", "foo");
    assert_that(cargo_process("new").arg("foo").arg("--git"),
                execs().with_status(0));

    assert_that(&paths::root().join("foo"), existing_dir());
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());
    assert_that(&paths::root().join("foo/.git"), existing_dir());
    assert_that(&paths::root().join("foo/.gitignore"), existing_file());

    assert_that(cargo_process("build").cwd(paths::root().join("foo")),
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
    fs::mkdir(&dst, UserRWX).assert();
    assert_that(cargo_process("new").arg("foo"),
                execs().with_status(101)
                       .with_stderr(format!("Destination `{}` already exists\n",
                                            dst.display())));
})

test!(finds_author_user {
    assert_that(cargo_process("new").arg("foo").env("USER", Some("foo")),
                execs().with_status(0));

    let toml = paths::root().join("foo/Cargo.toml");
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
