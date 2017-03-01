extern crate cargotest;
extern crate cargo;
extern crate tempdir;
extern crate hamcrest;

use std::fs::{self, File};
use std::io::prelude::*;
use std::env;

use cargo::util::ProcessBuilder;
use cargotest::support::{execs, paths, cargo_dir};
use hamcrest::{assert_that, existing_file, existing_dir, is_not};
use tempdir::TempDir;

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut p = cargotest::process(&cargo_dir().join("cargo"));
    p.arg(s).cwd(&paths::root()).env("HOME", &paths::home());
    p
}

#[test]
fn simple_lib() {
    assert_that(cargo_process("init").arg("--lib").arg("--vcs").arg("none")
                                    .env("USER", "foo"),
                execs().with_status(0).with_stderr("\
[CREATED] library project
"));

    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".gitignore"), is_not(existing_file()));

    assert_that(cargo_process("build"),
                execs().with_status(0));
}

#[test]
fn simple_bin() {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();
    assert_that(cargo_process("init").arg("--bin").arg("--vcs").arg("none")
                                    .env("USER", "foo").cwd(&path),
                execs().with_status(0).with_stderr("\
[CREATED] binary (application) project
"));

    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/main.rs"), existing_file());

    assert_that(cargo_process("build").cwd(&path),
                execs().with_status(0));
    assert_that(&paths::root().join(&format!("foo/target/debug/foo{}",
                                             env::consts::EXE_SUFFIX)),
                existing_file());
}

#[test]
fn both_lib_and_bin() {
    let td = TempDir::new("cargo").unwrap();
    assert_that(cargo_process("init").arg("--lib").arg("--bin").cwd(td.path())
                                    .env("USER", "foo"),
                execs().with_status(101).with_stderr(
                    "[ERROR] can't specify both lib and binary outputs"));
}

fn bin_already_exists(explicit: bool, rellocation: &str, needs_bin_section: bool) {
    let path = paths::root().join("foo");
    fs::create_dir_all(&path.join("src")).unwrap();

    let sourcefile_path = path.join(rellocation);

    let content = br#"
        fn main() {
            println!("Hello, world 2!");
        }
    "#;

    File::create(&sourcefile_path).unwrap().write_all(content).unwrap();

    if explicit {
        assert_that(cargo_process("init").arg("--bin").arg("--vcs").arg("none")
                                        .env("USER", "foo").cwd(&path),
                    execs().with_status(0));
    } else {
        assert_that(cargo_process("init").arg("--vcs").arg("none")
                                        .env("USER", "foo").cwd(&path),
                    execs().with_status(0));
    }

    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/lib.rs"), is_not(existing_file()));

    // Check that our file is not overwritten
    let mut new_content = Vec::new();
    File::open(&sourcefile_path).unwrap().read_to_end(&mut new_content).unwrap();
    assert_eq!(Vec::from(content as &[u8]), new_content);

    let mut cargo_content = String::new();
    File::open(&paths::root().join("foo/Cargo.toml")).unwrap()
        .read_to_string(&mut cargo_content).unwrap();
    // Check that Cargo.toml has a bin section pointing to the correct location (if needed)
    if needs_bin_section {
        assert!(cargo_content.contains(r#"[[bin]]"#));
        assert_that(&paths::root().join("foo/src/main.rs"), is_not(existing_file()));
    } else {
        assert!(!cargo_content.contains(r#"[[bin]]"#));
        assert_that(&paths::root().join("foo/src/main.rs"), existing_file());
    }
}

#[test]
fn bin_already_exists_explicit() {
    bin_already_exists(true, "src/main.rs", false)
}

#[test]
fn bin_already_exists_implicit() {
    bin_already_exists(false, "src/main.rs", false)
}

#[test]
fn bin_already_exists_explicit_nosrc() {
    bin_already_exists(true, "main.rs", true)
}

#[test]
fn bin_already_exists_implicit_nosrc() {
    bin_already_exists(false, "main.rs", true)
}

#[test]
fn bin_already_exists_implicit_namenosrc() {
    bin_already_exists(false, "foo.rs", true)
}

#[test]
fn bin_already_exists_implicit_namesrc() {
    bin_already_exists(false, "src/foo.rs", true)
}

#[test]
fn confused_by_multiple_lib_files() {
    let path = paths::root().join("foo");
    fs::create_dir_all(&path.join("src")).unwrap();

    let sourcefile_path1 = path.join("src/lib.rs");

    File::create(&sourcefile_path1).unwrap().write_all(br#"
        fn qqq () {
            println!("Hello, world 2!");
        }
    "#).unwrap();

    let sourcefile_path2 = path.join("lib.rs");

    File::create(&sourcefile_path2).unwrap().write_all(br#"
        fn qqq () {
            println!("Hello, world 3!");
        }
    "#).unwrap();

    assert_that(cargo_process("init").arg("--vcs").arg("none")
                                    .env("USER", "foo").cwd(&path),
                execs().with_status(101).with_stderr("\
[ERROR] cannot have a project with multiple libraries, found both `src/lib.rs` and `lib.rs`
"));

    assert_that(&paths::root().join("foo/Cargo.toml"), is_not(existing_file()));
}


#[test]
fn multibin_project_name_clash() {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();

    let sourcefile_path1 = path.join("foo.rs");

    File::create(&sourcefile_path1).unwrap().write_all(br#"
        fn main () {
            println!("Hello, world 2!");
        }
    "#).unwrap();

    let sourcefile_path2 = path.join("main.rs");

    File::create(&sourcefile_path2).unwrap().write_all(br#"
        fn main () {
            println!("Hello, world 3!");
        }
    "#).unwrap();

    assert_that(cargo_process("init").arg("--lib").arg("--vcs").arg("none")
                                    .env("USER", "foo").cwd(&path),
                execs().with_status(101).with_stderr("\
[ERROR] multiple possible binary sources found:
  main.rs
  foo.rs
cannot automatically generate Cargo.toml as the main target would be ambiguous
"));

    assert_that(&paths::root().join("foo/Cargo.toml"), is_not(existing_file()));
}

fn lib_already_exists(rellocation: &str, needs_lib_section: bool) {
    let path = paths::root().join("foo");
    fs::create_dir_all(&path.join("src")).unwrap();

    let sourcefile_path = path.join(rellocation);

    let content = br#"
        pub fn qqq() {}
    "#;

    File::create(&sourcefile_path).unwrap().write_all(content).unwrap();

    assert_that(cargo_process("init").arg("--vcs").arg("none")
                                    .env("USER", "foo").cwd(&path),
                execs().with_status(0));

    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/main.rs"), is_not(existing_file()));

    // Check that our file is not overwritten
    let mut new_content = Vec::new();
    File::open(&sourcefile_path).unwrap().read_to_end(&mut new_content).unwrap();
    assert_eq!(Vec::from(content as &[u8]), new_content);

    let mut cargo_content = String::new();
    File::open(&paths::root().join("foo/Cargo.toml")).unwrap()
        .read_to_string(&mut cargo_content).unwrap();
    // Check that Cargo.toml has a lib section pointing to the correct location (if needed)
    if needs_lib_section {
        assert!(cargo_content.contains(r#"[lib]"#));
        assert_that(&paths::root().join("foo/src/lib.rs"), is_not(existing_file()));
    } else {
        assert!(!cargo_content.contains(r#"[lib]"#));
        assert_that(&paths::root().join("foo/src/lib.rs"), existing_file());
    }

}

#[test]
fn lib_already_exists_src() {
    lib_already_exists("src/lib.rs", false)
}

#[test]
fn lib_already_exists_nosrc() {
    lib_already_exists("lib.rs", true)
}

#[test]
fn no_lib_already_exists_src_add_lib_section() {
    lib_already_exists("src/foo.rs", true)
}

#[test]
fn no_lib_already_exists_nosrc_add_lib_section() {
    lib_already_exists("foo.rs", true)
}

#[test]
fn simple_git() {
    assert_that(cargo_process("init").arg("--lib")
                                     .arg("--vcs")
                                     .arg("git")
                                     .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".git"), existing_dir());
    assert_that(&paths::root().join(".gitignore"), existing_file());
}

#[test]
fn auto_git() {
    let td = TempDir::new("cargo").unwrap();
    let foo = &td.path().join("foo");
    fs::create_dir_all(&foo).unwrap();
    assert_that(cargo_process("init").arg("--lib")
                                     .cwd(foo.clone())
                                     .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&foo.join("Cargo.toml"), existing_file());
    assert_that(&foo.join("src/lib.rs"), existing_file());
    assert_that(&foo.join(".git"), existing_dir());
    assert_that(&foo.join(".gitignore"), existing_file());
}

#[test]
fn invalid_dir_name() {
    let foo = &paths::root().join("foo.bar");
    fs::create_dir_all(&foo).unwrap();
    assert_that(cargo_process("init").cwd(foo.clone())
                                     .env("USER", "foo"),
                execs().with_status(101).with_stderr("\
[ERROR] Invalid character `.` in crate name: `foo.bar`
use --name to override crate name
"));

    assert_that(&foo.join("Cargo.toml"), is_not(existing_file()));
}

#[test]
fn reserved_name() {
    let test = &paths::root().join("test");
    fs::create_dir_all(&test).unwrap();
    assert_that(cargo_process("init").cwd(test.clone())
                                     .env("USER", "foo"),
                execs().with_status(101).with_stderr("\
[ERROR] The name `test` cannot be used as a crate name\n\
use --name to override crate name
"));

    assert_that(&test.join("Cargo.toml"), is_not(existing_file()));
}

#[test]
fn git_autodetect() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    assert_that(cargo_process("init").arg("--lib")
                                    .env("USER", "foo"),
                execs().with_status(0));


    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".git"), existing_dir());
    assert_that(&paths::root().join(".gitignore"), existing_file());
}


#[test]
fn mercurial_autodetect() {
    fs::create_dir(&paths::root().join(".hg")).unwrap();

    assert_that(cargo_process("init").arg("--lib")
                                    .env("USER", "foo"),
                execs().with_status(0));


    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".git"), is_not(existing_dir()));
    assert_that(&paths::root().join(".hgignore"), existing_file());
}

#[test]
fn gitignore_appended_not_replaced() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    File::create(&paths::root().join(".gitignore")).unwrap().write_all(b"qqqqqq\n").unwrap();

    assert_that(cargo_process("init").arg("--lib")
                                     .env("USER", "foo"),
                execs().with_status(0));


    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".git"), existing_dir());
    assert_that(&paths::root().join(".gitignore"), existing_file());

    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"qqqqqq"#));
}

#[test]
fn cargo_lock_gitignored_if_lib1() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    assert_that(cargo_process("init").arg("--lib").arg("--vcs").arg("git")
                                     .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join(".gitignore"), existing_file());

    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"Cargo.lock"#));
}

#[test]
fn cargo_lock_gitignored_if_lib2() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    File::create(&paths::root().join("lib.rs")).unwrap().write_all(br#""#).unwrap();

    assert_that(cargo_process("init").arg("--vcs").arg("git")
                                     .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join(".gitignore"), existing_file());

    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"Cargo.lock"#));
}

#[test]
fn cargo_lock_not_gitignored_if_bin1() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    assert_that(cargo_process("init").arg("--vcs").arg("git")
                                     .arg("--bin")
                                     .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join(".gitignore"), existing_file());

    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(!contents.contains(r#"Cargo.lock"#));
}

#[test]
fn cargo_lock_not_gitignored_if_bin2() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    File::create(&paths::root().join("main.rs")).unwrap().write_all(br#""#).unwrap();

    assert_that(cargo_process("init").arg("--vcs").arg("git")
                                     .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join(".gitignore"), existing_file());

    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(!contents.contains(r#"Cargo.lock"#));
}

#[test]
fn with_argument() {
    assert_that(cargo_process("init").arg("foo").arg("--vcs").arg("none")
                                     .env("USER", "foo"),
                execs().with_status(0));
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
}


#[test]
fn unknown_flags() {
    assert_that(cargo_process("init").arg("foo").arg("--flag"),
                execs().with_status(1)
                       .with_stderr("\
[ERROR] Unknown flag: '--flag'

Usage:
    cargo init [options] [<path>]
    cargo init -h | --help
"));
}

#[cfg(not(windows))]
#[test]
fn no_filename() {
    assert_that(cargo_process("init").arg("/"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] cannot auto-detect project name from path \"/\" ; use --name to override
".to_string()));
}
