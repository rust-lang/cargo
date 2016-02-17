use std::fs::{self, File};
use std::io::prelude::*;
use std::env;
use tempdir::TempDir;
use support::{execs, paths, cargo_dir};
use hamcrest::{assert_that, existing_file, existing_dir, is_not};

use cargo::util::{process, ProcessBuilder};

fn setup() {
}

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut p = process(&cargo_dir().join("cargo"));
    p.arg(s).cwd(&paths::root()).env("HOME", &paths::home());
    return p;
}

test!(simple_lib {
    assert_that(cargo_process("init").arg("--vcs").arg("none")
                                    .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".gitignore"), is_not(existing_file()));

    assert_that(cargo_process("build"),
                execs().with_status(0));
});

test!(simple_bin {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();
    assert_that(cargo_process("init").arg("--bin").arg("--vcs").arg("none")
                                    .env("USER", "foo").cwd(&path),
                execs().with_status(0));

    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
    assert_that(&paths::root().join("foo/src/main.rs"), existing_file());

    assert_that(cargo_process("build").cwd(&path),
                execs().with_status(0));
    assert_that(&paths::root().join(&format!("foo/target/debug/foo{}",
                                             env::consts::EXE_SUFFIX)),
                existing_file());
});

fn bin_already_exists(explicit: bool, rellocation: &str) {
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
}

test!(bin_already_exists_explicit {
    bin_already_exists(true, "src/main.rs")
});

test!(bin_already_exists_implicit {
    bin_already_exists(false, "src/main.rs")
});

test!(bin_already_exists_explicit_nosrc {
    bin_already_exists(true, "main.rs")
});

test!(bin_already_exists_implicit_nosrc {
    bin_already_exists(false, "main.rs")
});

test!(bin_already_exists_implicit_namenosrc {
    bin_already_exists(false, "foo.rs")
});

test!(bin_already_exists_implicit_namesrc {
    bin_already_exists(false, "src/foo.rs")
});

test!(confused_by_multiple_lib_files {
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
cannot have a project with multiple libraries, found both `src/lib.rs` and `lib.rs`
"));
    
    assert_that(&paths::root().join("foo/Cargo.toml"), is_not(existing_file()));
});


test!(multibin_project_name_clash {
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
    
    assert_that(cargo_process("init").arg("--vcs").arg("none")
                                    .env("USER", "foo").cwd(&path),
                execs().with_status(101).with_stderr("\
multiple possible binary sources found:
  main.rs
  foo.rs
cannot automatically generate Cargo.toml as the main target would be ambiguous
"));
                
    assert_that(&paths::root().join("foo/Cargo.toml"), is_not(existing_file()));
});

fn lib_already_exists(rellocation: &str) {
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
}

test!(lib_already_exists_src {
    lib_already_exists("src/lib.rs")
});

test!(lib_already_exists_nosrc {
    lib_already_exists("lib.rs")
});

test!(simple_git {
    assert_that(cargo_process("init").arg("--vcs").arg("git")
                                    .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".git"), existing_dir());
    assert_that(&paths::root().join(".gitignore"), existing_file());
});

test!(auto_git {
    let td = TempDir::new("cargo").unwrap();
    let foo = &td.path().join("foo");
    fs::create_dir_all(&foo).unwrap();
    assert_that(cargo_process("init").cwd(foo.clone())
                                     .env("USER", "foo"),
                execs().with_status(0));

    assert_that(&foo.join("Cargo.toml"), existing_file());
    assert_that(&foo.join("src/lib.rs"), existing_file());
    assert_that(&foo.join(".git"), existing_dir());
    assert_that(&foo.join(".gitignore"), existing_file());
});

test!(invalid_dir_name {
    let foo = &paths::root().join("foo.bar");
    fs::create_dir_all(&foo).unwrap();
    assert_that(cargo_process("init").cwd(foo.clone())
                                     .env("USER", "foo"),
                execs().with_status(101).with_stderr("\
Invalid character `.` in crate name: `foo.bar`
use --name to override crate name
"));

    assert_that(&foo.join("Cargo.toml"), is_not(existing_file()));
});

test!(git_autodetect {
    fs::create_dir(&paths::root().join(".git")).unwrap();
    
    assert_that(cargo_process("init")
                                    .env("USER", "foo"),
                execs().with_status(0));
    

    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".git"), existing_dir());
    assert_that(&paths::root().join(".gitignore"), existing_file());
});


test!(mercurial_autodetect {
    fs::create_dir(&paths::root().join(".hg")).unwrap();
    
    assert_that(cargo_process("init")
                                    .env("USER", "foo"),
                execs().with_status(0));
    

    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".git"), is_not(existing_dir()));
    assert_that(&paths::root().join(".hgignore"), existing_file());
});

test!(gitignore_appended_not_replaced {
    fs::create_dir(&paths::root().join(".git")).unwrap();
    
    File::create(&paths::root().join(".gitignore")).unwrap().write_all(b"qqqqqq\n").unwrap();
    
    assert_that(cargo_process("init")
                                    .env("USER", "foo"),
                execs().with_status(0));
    

    assert_that(&paths::root().join("Cargo.toml"), existing_file());
    assert_that(&paths::root().join("src/lib.rs"), existing_file());
    assert_that(&paths::root().join(".git"), existing_dir());
    assert_that(&paths::root().join(".gitignore"), existing_file());
    
    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"qqqqqq"#));
});

test!(cargo_lock_gitignored_if_lib1 {
    fs::create_dir(&paths::root().join(".git")).unwrap();
    
    assert_that(cargo_process("init").arg("--vcs").arg("git")
                                     .env("USER", "foo"),
                execs().with_status(0));
    
    assert_that(&paths::root().join(".gitignore"), existing_file());
    
    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"Cargo.lock"#));
});

test!(cargo_lock_gitignored_if_lib2 {
    fs::create_dir(&paths::root().join(".git")).unwrap();
    
    File::create(&paths::root().join("lib.rs")).unwrap().write_all(br#""#).unwrap();

    assert_that(cargo_process("init").arg("--vcs").arg("git")
                                     .env("USER", "foo"),
                execs().with_status(0));
    
    assert_that(&paths::root().join(".gitignore"), existing_file());
    
    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(contents.contains(r#"Cargo.lock"#));
});

test!(cargo_lock_not_gitignored_if_bin1 {
    fs::create_dir(&paths::root().join(".git")).unwrap();
    
    assert_that(cargo_process("init").arg("--vcs").arg("git")
                                     .arg("--bin")
                                     .env("USER", "foo"),
                execs().with_status(0));
    
    assert_that(&paths::root().join(".gitignore"), existing_file());
    
    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(!contents.contains(r#"Cargo.lock"#));
});

test!(cargo_lock_not_gitignored_if_bin2 {
    fs::create_dir(&paths::root().join(".git")).unwrap();
    
    File::create(&paths::root().join("main.rs")).unwrap().write_all(br#""#).unwrap();

    assert_that(cargo_process("init").arg("--vcs").arg("git")
                                     .env("USER", "foo"),
                execs().with_status(0));
    
    assert_that(&paths::root().join(".gitignore"), existing_file());
    
    let mut contents = String::new();
    File::open(&paths::root().join(".gitignore")).unwrap().read_to_string(&mut contents).unwrap();
    assert!(!contents.contains(r#"Cargo.lock"#));
});

test!(with_argument {
    assert_that(cargo_process("init").arg("foo").arg("--vcs").arg("none")
                                     .env("USER", "foo"),
                execs().with_status(0));
    assert_that(&paths::root().join("foo/Cargo.toml"), existing_file());
});


test!(unknown_flags {
    assert_that(cargo_process("init").arg("foo").arg("--flag"),
                execs().with_status(1)
                       .with_stderr("\
Unknown flag: '--flag'

Usage:
    cargo init [options] [<path>]
    cargo init -h | --help
"));
});

#[cfg(not(windows))]
test!(no_filename {
    assert_that(cargo_process("init").arg("/"),
                execs().with_status(101)
                       .with_stderr("\
cannot auto-detect project name from path \"/\" ; use --name to override
"));
});
