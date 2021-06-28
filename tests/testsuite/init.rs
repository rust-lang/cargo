//! Tests for the `cargo init` command.

use cargo_test_support::{command_is_available, paths, rustc_host, Execs};
use std::env;
use std::fs;
use std::process::Command;

fn cargo_process(s: &str) -> Execs {
    let mut execs = cargo_test_support::cargo_process(s);
    execs.cwd(&paths::root()).env("HOME", &paths::home());
    execs
}

fn mercurial_available() -> bool {
    let result = Command::new("hg")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !result {
        println!("`hg` not available, skipping test");
    }
    result
}

#[cargo_test]
fn simple_lib() {
    cargo_process("init --lib --vcs none --edition 2015")
        .with_stderr("[CREATED] library package")
        .run();

    assert!(paths::root().join("Cargo.toml").is_file());
    assert!(paths::root().join("src/lib.rs").is_file());
    assert!(!paths::root().join(".gitignore").is_file());

    cargo_process("build").run();
}

#[cargo_test]
fn simple_bin() {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();
    cargo_process("init --bin --vcs none --edition 2015")
        .cwd(&path)
        .with_stderr("[CREATED] binary (application) package")
        .run();

    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(paths::root().join("foo/src/main.rs").is_file());

    cargo_process("build").cwd(&path).run();
    assert!(paths::root()
        .join(&format!(
            "foo/target/{}/debug/foo{}",
            rustc_host(),
            env::consts::EXE_SUFFIX
        ))
        .is_file());
}

#[cargo_test]
fn simple_git_ignore_exists() {
    // write a .gitignore file with two entries
    fs::create_dir_all(paths::root().join("foo")).unwrap();
    fs::write(
        paths::root().join("foo/.gitignore"),
        "/target\n**/some.file",
    )
    .unwrap();

    cargo_process("init --lib foo --edition 2015").run();

    assert!(paths::root().is_dir());
    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(paths::root().join("foo/src/lib.rs").is_file());
    assert!(paths::root().join("foo/.git").is_dir());
    assert!(paths::root().join("foo/.gitignore").is_file());

    let fp = paths::root().join("foo/.gitignore");
    let contents = fs::read_to_string(fp).unwrap();
    assert_eq!(
        contents,
        "/target\n\
         **/some.file\n\n\
         # Added by cargo\n\
         #\n\
         # already existing elements were commented out\n\
         \n\
         #/target\n\
         Cargo.lock\n",
    );

    cargo_process("build").cwd(&paths::root().join("foo")).run();
}

#[cargo_test]
fn git_ignore_exists_no_conflicting_entries() {
    // write a .gitignore file with one entry
    fs::create_dir_all(paths::root().join("foo")).unwrap();
    fs::write(paths::root().join("foo/.gitignore"), "**/some.file").unwrap();

    cargo_process("init --lib foo --edition 2015").run();

    let fp = paths::root().join("foo/.gitignore");
    let contents = fs::read_to_string(&fp).unwrap();
    assert_eq!(
        contents,
        "**/some.file\n\n\
         # Added by cargo\n\
         \n\
         /target\n\
         Cargo.lock\n",
    );
}

#[cargo_test]
fn both_lib_and_bin() {
    cargo_process("init --lib --bin")
        .with_status(101)
        .with_stderr("[ERROR] can't specify both lib and binary outputs")
        .run();
}

fn bin_already_exists(explicit: bool, rellocation: &str) {
    let path = paths::root().join("foo");
    fs::create_dir_all(&path.join("src")).unwrap();

    let sourcefile_path = path.join(rellocation);

    let content = r#"
        fn main() {
            println!("Hello, world 2!");
        }
    "#;

    fs::write(&sourcefile_path, content).unwrap();

    if explicit {
        cargo_process("init --bin --vcs none").cwd(&path).run();
    } else {
        cargo_process("init --vcs none").cwd(&path).run();
    }

    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(!paths::root().join("foo/src/lib.rs").is_file());

    // Check that our file is not overwritten
    let new_content = fs::read_to_string(&sourcefile_path).unwrap();
    assert_eq!(content, new_content);
}

#[cargo_test]
fn bin_already_exists_explicit() {
    bin_already_exists(true, "src/main.rs")
}

#[cargo_test]
fn bin_already_exists_implicit() {
    bin_already_exists(false, "src/main.rs")
}

#[cargo_test]
fn bin_already_exists_explicit_nosrc() {
    bin_already_exists(true, "main.rs")
}

#[cargo_test]
fn bin_already_exists_implicit_nosrc() {
    bin_already_exists(false, "main.rs")
}

#[cargo_test]
fn bin_already_exists_implicit_namenosrc() {
    bin_already_exists(false, "foo.rs")
}

#[cargo_test]
fn bin_already_exists_implicit_namesrc() {
    bin_already_exists(false, "src/foo.rs")
}

#[cargo_test]
fn confused_by_multiple_lib_files() {
    let path = paths::root().join("foo");
    fs::create_dir_all(&path.join("src")).unwrap();

    let path1 = path.join("src/lib.rs");
    fs::write(path1, r#"fn qqq () { println!("Hello, world 2!"); }"#).unwrap();

    let path2 = path.join("lib.rs");
    fs::write(path2, r#" fn qqq () { println!("Hello, world 3!"); }"#).unwrap();

    cargo_process("init --vcs none")
        .cwd(&path)
        .with_status(101)
        .with_stderr(
            "[ERROR] cannot have a package with multiple libraries, \
            found both `src/lib.rs` and `lib.rs`",
        )
        .run();

    assert!(!paths::root().join("foo/Cargo.toml").is_file());
}

#[cargo_test]
fn multibin_project_name_clash() {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();

    let path1 = path.join("foo.rs");
    fs::write(path1, r#"fn main () { println!("Hello, world 2!"); }"#).unwrap();

    let path2 = path.join("main.rs");
    fs::write(path2, r#"fn main () { println!("Hello, world 3!"); }"#).unwrap();

    cargo_process("init --lib --vcs none")
        .cwd(&path)
        .with_status(101)
        .with_stderr(
            "\
[ERROR] multiple possible binary sources found:
  main.rs
  foo.rs
cannot automatically generate Cargo.toml as the main target would be ambiguous
",
        )
        .run();

    assert!(!paths::root().join("foo/Cargo.toml").is_file());
}

fn lib_already_exists(rellocation: &str) {
    let path = paths::root().join("foo");
    fs::create_dir_all(&path.join("src")).unwrap();

    let sourcefile_path = path.join(rellocation);

    let content = "pub fn qqq() {}";
    fs::write(&sourcefile_path, content).unwrap();

    cargo_process("init --vcs none").cwd(&path).run();

    assert!(paths::root().join("foo/Cargo.toml").is_file());
    assert!(!paths::root().join("foo/src/main.rs").is_file());

    // Check that our file is not overwritten
    let new_content = fs::read_to_string(&sourcefile_path).unwrap();
    assert_eq!(content, new_content);
}

#[cargo_test]
fn lib_already_exists_src() {
    lib_already_exists("src/lib.rs");
}

#[cargo_test]
fn lib_already_exists_nosrc() {
    lib_already_exists("lib.rs");
}

#[cargo_test]
fn simple_git() {
    cargo_process("init --lib --vcs git").run();

    assert!(paths::root().join("Cargo.toml").is_file());
    assert!(paths::root().join("src/lib.rs").is_file());
    assert!(paths::root().join(".git").is_dir());
    assert!(paths::root().join(".gitignore").is_file());
}

#[cargo_test]
fn auto_git() {
    cargo_process("init --lib").run();

    assert!(paths::root().join("Cargo.toml").is_file());
    assert!(paths::root().join("src/lib.rs").is_file());
    assert!(paths::root().join(".git").is_dir());
    assert!(paths::root().join(".gitignore").is_file());
}

#[cargo_test]
fn invalid_dir_name() {
    let foo = &paths::root().join("foo.bar");
    fs::create_dir_all(&foo).unwrap();
    cargo_process("init")
        .cwd(foo.clone())
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid character `.` in package name: `foo.bar`, [..]
If you need a package name to not match the directory name, consider using --name flag.
If you need a binary with the name \"foo.bar\", use a valid package name, \
and set the binary name to be different from the package. \
This can be done by setting the binary filename to `src/bin/foo.bar.rs` \
or change the name in Cargo.toml with:

    [bin]
    name = \"foo.bar\"
    path = \"src/main.rs\"

",
        )
        .run();

    assert!(!foo.join("Cargo.toml").is_file());
}

#[cargo_test]
fn reserved_name() {
    let test = &paths::root().join("test");
    fs::create_dir_all(&test).unwrap();
    cargo_process("init")
        .cwd(test.clone())
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the name `test` cannot be used as a package name, it conflicts [..]\n\
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

    assert!(!test.join("Cargo.toml").is_file());
}

#[cargo_test]
fn git_autodetect() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    cargo_process("init --lib").run();

    assert!(paths::root().join("Cargo.toml").is_file());
    assert!(paths::root().join("src/lib.rs").is_file());
    assert!(paths::root().join(".git").is_dir());
    assert!(paths::root().join(".gitignore").is_file());
}

#[cargo_test]
fn mercurial_autodetect() {
    fs::create_dir(&paths::root().join(".hg")).unwrap();

    cargo_process("init --lib").run();

    assert!(paths::root().join("Cargo.toml").is_file());
    assert!(paths::root().join("src/lib.rs").is_file());
    assert!(!paths::root().join(".git").is_dir());
    assert!(paths::root().join(".hgignore").is_file());
}

#[cargo_test]
fn gitignore_appended_not_replaced() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    fs::write(&paths::root().join(".gitignore"), "qqqqqq\n").unwrap();

    cargo_process("init --lib").run();

    assert!(paths::root().join("Cargo.toml").is_file());
    assert!(paths::root().join("src/lib.rs").is_file());
    assert!(paths::root().join(".git").is_dir());
    assert!(paths::root().join(".gitignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();
    assert!(contents.contains("qqqqqq"));
}

#[cargo_test]
fn gitignore_added_newline_in_existing() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    fs::write(&paths::root().join(".gitignore"), "first").unwrap();

    cargo_process("init --lib").run();

    assert!(paths::root().join(".gitignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();
    assert!(contents.starts_with("first\n"));
}

#[cargo_test]
fn gitignore_no_newline_in_new() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    cargo_process("init --lib").run();

    assert!(paths::root().join(".gitignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();
    assert!(!contents.starts_with('\n'));
}

#[cargo_test]
fn mercurial_added_newline_in_existing() {
    fs::create_dir(&paths::root().join(".hg")).unwrap();

    fs::write(&paths::root().join(".hgignore"), "first").unwrap();

    cargo_process("init --lib").run();

    assert!(paths::root().join(".hgignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".hgignore")).unwrap();
    assert!(contents.starts_with("first\n"));
}

#[cargo_test]
fn mercurial_no_newline_in_new() {
    fs::create_dir(&paths::root().join(".hg")).unwrap();

    cargo_process("init --lib").run();

    assert!(paths::root().join(".hgignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".hgignore")).unwrap();
    assert!(!contents.starts_with('\n'));
}

#[cargo_test]
fn terminating_newline_in_new_git_ignore() {
    cargo_process("init --vcs git --lib").run();

    let content = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();

    let mut last_chars = content.chars().rev();
    assert_eq!(last_chars.next(), Some('\n'));
    assert_ne!(last_chars.next(), Some('\n'));
}

#[cargo_test]
fn terminating_newline_in_new_mercurial_ignore() {
    if !mercurial_available() {
        return;
    }
    cargo_process("init --vcs hg --lib").run();

    let content = fs::read_to_string(&paths::root().join(".hgignore")).unwrap();

    let mut last_chars = content.chars().rev();
    assert_eq!(last_chars.next(), Some('\n'));
    assert_ne!(last_chars.next(), Some('\n'));
}

#[cargo_test]
fn terminating_newline_in_existing_git_ignore() {
    fs::create_dir(&paths::root().join(".git")).unwrap();
    fs::write(&paths::root().join(".gitignore"), b"first").unwrap();

    cargo_process("init --lib").run();

    let content = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();

    let mut last_chars = content.chars().rev();
    assert_eq!(last_chars.next(), Some('\n'));
    assert_ne!(last_chars.next(), Some('\n'));
}

#[cargo_test]
fn terminating_newline_in_existing_mercurial_ignore() {
    fs::create_dir(&paths::root().join(".hg")).unwrap();
    fs::write(&paths::root().join(".hgignore"), b"first").unwrap();

    cargo_process("init --lib").run();

    let content = fs::read_to_string(&paths::root().join(".hgignore")).unwrap();

    let mut last_chars = content.chars().rev();
    assert_eq!(last_chars.next(), Some('\n'));
    assert_ne!(last_chars.next(), Some('\n'));
}

#[cargo_test]
fn cargo_lock_gitignored_if_lib1() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    cargo_process("init --lib --vcs git").run();

    assert!(paths::root().join(".gitignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();
    assert!(contents.contains(r#"Cargo.lock"#));
}

#[cargo_test]
fn cargo_lock_gitignored_if_lib2() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    fs::write(&paths::root().join("lib.rs"), "").unwrap();

    cargo_process("init --vcs git").run();

    assert!(paths::root().join(".gitignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();
    assert!(contents.contains(r#"Cargo.lock"#));
}

#[cargo_test]
fn cargo_lock_not_gitignored_if_bin1() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    cargo_process("init --vcs git --bin").run();

    assert!(paths::root().join(".gitignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();
    assert!(!contents.contains(r#"Cargo.lock"#));
}

#[cargo_test]
fn cargo_lock_not_gitignored_if_bin2() {
    fs::create_dir(&paths::root().join(".git")).unwrap();

    fs::write(&paths::root().join("main.rs"), "").unwrap();

    cargo_process("init --vcs git").run();

    assert!(paths::root().join(".gitignore").is_file());

    let contents = fs::read_to_string(&paths::root().join(".gitignore")).unwrap();
    assert!(!contents.contains(r#"Cargo.lock"#));
}

#[cargo_test]
fn with_argument() {
    cargo_process("init foo --vcs none").run();
    assert!(paths::root().join("foo/Cargo.toml").is_file());
}

#[cargo_test]
fn unknown_flags() {
    cargo_process("init foo --flag")
        .with_status(1)
        .with_stderr_contains(
            "error: Found argument '--flag' which wasn't expected, or isn't valid in this context",
        )
        .run();
}

#[cfg(not(windows))]
#[cargo_test]
fn no_filename() {
    cargo_process("init /")
        .with_status(101)
        .with_stderr(
            "[ERROR] cannot auto-detect package name from path \"/\" ; use --name to override"
                .to_string(),
        )
        .run();
}

#[cargo_test]
fn formats_source() {
    if !command_is_available("rustfmt") {
        return;
    }

    fs::write(&paths::root().join("rustfmt.toml"), "tab_spaces = 2").unwrap();

    cargo_process("init --lib")
        .with_stderr("[CREATED] library package")
        .run();

    assert_eq!(
        fs::read_to_string(paths::root().join("src/lib.rs")).unwrap(),
        r#"#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
"#
    );
}

#[cargo_test]
fn ignores_failure_to_format_source() {
    cargo_process("init --lib")
        .env("PATH", "") // pretend that `rustfmt` is missing
        .with_stderr("[CREATED] library package")
        .run();

    assert_eq!(
        fs::read_to_string(paths::root().join("src/lib.rs")).unwrap(),
        r#"#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
"#
    );
}

#[cargo_test]
fn creates_binary_when_instructed_and_has_lib_file_no_warning() {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("foo.rs"), "fn not_main() {}").unwrap();
    cargo_process("init --bin")
        .cwd(&path)
        .with_stderr(
            "\
[WARNING] file `foo.rs` seems to be a library file
[CREATED] binary (application) package
",
        )
        .run();

    let cargo_toml = fs::read_to_string(path.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains("[[bin]]"));
    assert!(!cargo_toml.contains("[lib]"));
}

#[cargo_test]
fn creates_library_when_instructed_and_has_bin_file() {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("foo.rs"), "fn main() {}").unwrap();
    cargo_process("init --lib")
        .cwd(&path)
        .with_stderr(
            "\
[WARNING] file `foo.rs` seems to be a binary (application) file
[CREATED] library package
",
        )
        .run();

    let cargo_toml = fs::read_to_string(path.join("Cargo.toml")).unwrap();
    assert!(!cargo_toml.contains("[[bin]]"));
    assert!(cargo_toml.contains("[lib]"));
}

#[cargo_test]
fn creates_binary_when_both_binlib_present() {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("foo.rs"), "fn main() {}").unwrap();
    fs::write(path.join("lib.rs"), "fn notmain() {}").unwrap();
    cargo_process("init --bin")
        .cwd(&path)
        .with_stderr("[CREATED] binary (application) package")
        .run();

    let cargo_toml = fs::read_to_string(path.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains("[[bin]]"));
    assert!(cargo_toml.contains("[lib]"));
}

#[cargo_test]
fn cant_create_library_when_both_binlib_present() {
    let path = paths::root().join("foo");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("foo.rs"), "fn main() {}").unwrap();
    fs::write(path.join("lib.rs"), "fn notmain() {}").unwrap();
    cargo_process("init --lib")
        .cwd(&path)
        .with_status(101)
        .with_stderr(
            "[ERROR] cannot have a package with multiple libraries, found both `foo.rs` and `lib.rs`"
            )
        .run();
}
