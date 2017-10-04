extern crate cargo;
extern crate cargotest;
extern crate hamcrest;

use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;

use cargo::util::ProcessBuilder;
use cargotest::install::{cargo_home, has_installed_exe};
use cargotest::support::git;
use cargotest::support::paths;
use cargotest::support::registry::Package;
use cargotest::support::{project, execs};
use hamcrest::{assert_that, is_not};

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut p = cargotest::cargo_process();
    p.arg(s);
    p
}

fn pkg(name: &str, vers: &str) {
    Package::new(name, vers)
        .file("src/lib.rs", "")
        .file("src/main.rs", &format!("
            extern crate {};
            fn main() {{}}
        ", name))
        .publish();
}

#[test]
fn simple() {
    pkg("foo", "0.0.1");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0).with_stderr(&format!("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.1 (registry [..])
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] {home}[..]bin[..]foo[..]
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
",
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));

    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(0).with_stderr(&format!("\
[REMOVING] {home}[..]bin[..]foo[..]
",
        home = cargo_home().display())));
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));
}

#[test]
fn multiple_pkgs() {
    pkg("foo", "0.0.1");
    pkg("bar", "0.0.2");

    assert_that(cargo_process("install").args(&["foo", "bar", "baz"]),
                execs().with_status(101).with_stderr(&format!("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.1 (registry `file://[..]`)
[INSTALLING] foo v0.0.1
[COMPILING] foo v0.0.1
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] {home}[..]bin[..]foo[..]
[DOWNLOADING] bar v0.0.2 (registry `file://[..]`)
[INSTALLING] bar v0.0.2
[COMPILING] bar v0.0.2
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] {home}[..]bin[..]bar[..]
error: could not find `baz` in registry `[..]`
   
Summary: Successfully installed foo, bar! Failed to install baz (see error(s) above).
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
error: some crates failed to install
",
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), has_installed_exe("bar"));

    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(0).with_stderr(&format!("\
[REMOVING] {home}[..]bin[..]foo[..]
",
        home = cargo_home().display())));
    assert_that(cargo_process("uninstall").arg("bar"),
                execs().with_status(0).with_stderr(&format!("\
[REMOVING] {home}[..]bin[..]bar[..]
",
        home = cargo_home().display())));
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));
    assert_that(cargo_home(), is_not(has_installed_exe("bar")));
}

#[test]
fn pick_max_version() {
    pkg("foo", "0.0.1");
    pkg("foo", "0.0.2");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0).with_stderr(&format!("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.2 (registry [..])
[INSTALLING] foo v0.0.2
[COMPILING] foo v0.0.2
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] {home}[..]bin[..]foo[..]
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
",
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));
}

#[test]
fn missing() {
    pkg("foo", "0.0.1");
    assert_that(cargo_process("install").arg("bar"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[ERROR] could not find `bar` in registry `[..]`
"));
}

#[test]
fn bad_version() {
    pkg("foo", "0.0.1");
    assert_that(cargo_process("install").arg("foo").arg("--vers=0.2.0"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[ERROR] could not find `foo` in registry `[..]` with version `=0.2.0`
"));
}

#[test]
fn no_crate() {
    assert_that(cargo_process("install"),
                execs().with_status(101).with_stderr("\
[ERROR] `[..]` is not a crate root; specify a crate to install [..]

Caused by:
  failed to read `[..]Cargo.toml`

Caused by:
  [..] (os error [..])
"));
}

#[test]
fn install_location_precedence() {
    pkg("foo", "0.0.1");

    let root = paths::root();
    let t1 = root.join("t1");
    let t2 = root.join("t2");
    let t3 = root.join("t3");
    let t4 = cargo_home();

    fs::create_dir(root.join(".cargo")).unwrap();
    File::create(root.join(".cargo/config")).unwrap().write_all(format!("\
        [install]
        root = '{}'
    ", t3.display()).as_bytes()).unwrap();

    println!("install --root");

    assert_that(cargo_process("install").arg("foo")
                            .arg("--root").arg(&t1)
                            .env("CARGO_INSTALL_ROOT", &t2),
                execs().with_status(0));
    assert_that(&t1, has_installed_exe("foo"));
    assert_that(&t2, is_not(has_installed_exe("foo")));

    println!("install CARGO_INSTALL_ROOT");

    assert_that(cargo_process("install").arg("foo")
                            .env("CARGO_INSTALL_ROOT", &t2),
                execs().with_status(0));
    assert_that(&t2, has_installed_exe("foo"));
    assert_that(&t3, is_not(has_installed_exe("foo")));

    println!("install install.root");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0));
    assert_that(&t3, has_installed_exe("foo"));
    assert_that(&t4, is_not(has_installed_exe("foo")));

    fs::remove_file(root.join(".cargo/config")).unwrap();

    println!("install cargo home");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0));
    assert_that(&t4, has_installed_exe("foo"));
}

#[test]
fn install_path() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_process("install").arg("--path").arg(".").cwd(p.root()),
                execs().with_status(101).with_stderr("\
[INSTALLING] foo v0.1.0 [..]
[ERROR] binary `foo[..]` already exists in destination as part of `foo v0.1.0 [..]`
Add --force to overwrite
"));
}

#[test]
fn multiple_crates_error() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("a/src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--git").arg(p.url().to_string()),
                execs().with_status(101).with_stderr("\
[UPDATING] git repository [..]
[ERROR] multiple packages with binaries found: bar, foo
"));
}

#[test]
fn multiple_crates_select() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("a/src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--git").arg(p.url().to_string())
                                        .arg("foo"),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), is_not(has_installed_exe("bar")));

    assert_that(cargo_process("install").arg("--git").arg(p.url().to_string())
                                        .arg("bar"),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("bar"));
}

#[test]
fn multiple_crates_auto_binaries() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            bar = { path = "a" }
        "#)
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
}

#[test]
fn multiple_crates_auto_examples() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            bar = { path = "a" }
        "#)
        .file("src/lib.rs", "extern crate bar;")
        .file("examples/foo.rs", "
            extern crate bar;
            extern crate foo;
            fn main() {}
        ")
        .file("a/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root())
                                        .arg("--example=foo"),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
}

#[test]
fn no_binaries_or_examples() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            bar = { path = "a" }
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(101).with_stderr("\
[ERROR] no packages found with binaries or examples
"));
}

#[test]
fn no_binaries() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("examples/foo.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()).arg("foo"),
                execs().with_status(101).with_stderr("\
[INSTALLING] foo [..]
[ERROR] specified package has no binaries
"));
}

#[test]
fn examples() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("examples/foo.rs", "extern crate foo; fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root())
                                        .arg("--example=foo"),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
}

#[test]
fn install_twice() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/bin/foo-bin1.rs", "fn main() {}")
        .file("src/bin/foo-bin2.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0));
    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(101).with_stderr("\
[INSTALLING] foo v0.1.0 [..]
[ERROR] binary `foo-bin1[..]` already exists in destination as part of `foo v0.1.0 ([..])`
binary `foo-bin2[..]` already exists in destination as part of `foo v0.1.0 ([..])`
Add --force to overwrite
"));
}

#[test]
fn install_force() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0));

    let p = project("foo2")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.2.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--force").arg("--path").arg(p.root()),
                execs().with_status(0).with_stderr(&format!("\
[INSTALLING] foo v0.2.0 ([..])
[COMPILING] foo v0.2.0 ([..])
[FINISHED] release [optimized] target(s) in [..]
[REPLACING] {home}[..]bin[..]foo[..]
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
",
        home = cargo_home().display())));

    assert_that(cargo_process("install").arg("--list"),
                execs().with_status(0).with_stdout("\
foo v0.2.0 ([..]):
    foo[..]
"));
}

#[test]
fn install_force_partial_overlap() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/bin/foo-bin1.rs", "fn main() {}")
        .file("src/bin/foo-bin2.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0));

    let p = project("foo2")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.2.0"
            authors = []
        "#)
        .file("src/bin/foo-bin2.rs", "fn main() {}")
        .file("src/bin/foo-bin3.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--force").arg("--path").arg(p.root()),
                execs().with_status(0).with_stderr(&format!("\
[INSTALLING] foo v0.2.0 ([..])
[COMPILING] foo v0.2.0 ([..])
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] {home}[..]bin[..]foo-bin3[..]
[REPLACING] {home}[..]bin[..]foo-bin2[..]
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
",
        home = cargo_home().display())));

    assert_that(cargo_process("install").arg("--list"),
                execs().with_status(0).with_stdout("\
foo v0.1.0 ([..]):
    foo-bin1[..]
foo v0.2.0 ([..]):
    foo-bin2[..]
    foo-bin3[..]
"));
}

#[test]
fn install_force_bin() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/bin/foo-bin1.rs", "fn main() {}")
        .file("src/bin/foo-bin2.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0));

    let p = project("foo2")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.2.0"
            authors = []
        "#)
        .file("src/bin/foo-bin1.rs", "fn main() {}")
        .file("src/bin/foo-bin2.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--force")
                    .arg("--bin")
                    .arg("foo-bin2")
                    .arg("--path")
                    .arg(p.root()),
                execs().with_status(0).with_stderr(&format!("\
[INSTALLING] foo v0.2.0 ([..])
[COMPILING] foo v0.2.0 ([..])
[FINISHED] release [optimized] target(s) in [..]
[REPLACING] {home}[..]bin[..]foo-bin2[..]
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
",
        home = cargo_home().display())));

    assert_that(cargo_process("install").arg("--list"),
                execs().with_status(0).with_stdout("\
foo v0.1.0 ([..]):
    foo-bin1[..]
foo v0.2.0 ([..]):
    foo-bin2[..]
"));
}

#[test]
fn compile_failure() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(101).with_stderr_contains("\
[ERROR] failed to compile `foo v0.1.0 ([..])`, intermediate artifacts can be \
    found at `[..]target`

Caused by:
  Could not compile `foo`.

To learn more, run the command again with --verbose.
"));
}

#[test]
fn git_repo() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    // use `--locked` to test that we don't even try to write a lockfile
    assert_that(cargo_process("install").arg("--locked").arg("--git").arg(p.url().to_string()),
                execs().with_status(0).with_stderr(&format!("\
[UPDATING] git repository `[..]`
[INSTALLING] foo v0.1.0 ([..])
[COMPILING] foo v0.1.0 ([..])
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] {home}[..]bin[..]foo[..]
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
",
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), has_installed_exe("foo"));
}

#[test]
fn list() {
    pkg("foo", "0.0.1");
    pkg("bar", "0.2.1");
    pkg("bar", "0.2.2");

    assert_that(cargo_process("install").arg("--list"),
                execs().with_status(0).with_stdout(""));

    assert_that(cargo_process("install").arg("bar").arg("--vers").arg("=0.2.1"),
                execs().with_status(0));
    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0));
    assert_that(cargo_process("install").arg("--list"),
                execs().with_status(0).with_stdout("\
bar v0.2.1:
    bar[..]
foo v0.0.1:
    foo[..]
"));
}

#[test]
fn list_error() {
    pkg("foo", "0.0.1");
    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0));
    assert_that(cargo_process("install").arg("--list"),
                execs().with_status(0).with_stdout("\
foo v0.0.1:
    foo[..]
"));
    let mut worldfile_path = cargo_home();
    worldfile_path.push(".crates.toml");
    let mut worldfile = OpenOptions::new()
                            .write(true)
                            .open(worldfile_path)
                            .expect(".crates.toml should be there");
    worldfile.write_all(b"\x00").unwrap();
    drop(worldfile);
    assert_that(cargo_process("install").arg("--list").arg("--verbose"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse crate metadata at `[..]`

Caused by:
  invalid TOML found for metadata

Caused by:
  unexpected character[..]
"));
}

#[test]
fn uninstall_pkg_does_not_exist() {
    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(101).with_stderr("\
[ERROR] package id specification `foo` matched no packages
"));
}

#[test]
fn uninstall_bin_does_not_exist() {
    pkg("foo", "0.0.1");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0));
    assert_that(cargo_process("uninstall").arg("foo").arg("--bin=bar"),
                execs().with_status(101).with_stderr("\
[ERROR] binary `bar[..]` not installed as part of `foo v0.0.1`
"));
}

#[test]
fn uninstall_piecemeal() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/bin/foo.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), has_installed_exe("bar"));

    assert_that(cargo_process("uninstall").arg("foo").arg("--bin=bar"),
                execs().with_status(0).with_stderr("\
[REMOVING] [..]bar[..]
"));

    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), is_not(has_installed_exe("bar")));

    assert_that(cargo_process("uninstall").arg("foo").arg("--bin=foo"),
                execs().with_status(0).with_stderr("\
[REMOVING] [..]foo[..]
"));
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));

    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(101).with_stderr("\
[ERROR] package id specification `foo` matched no packages
"));
}

#[test]
fn subcommand_works_out_of_the_box() {
    Package::new("cargo-foo", "1.0.0")
        .file("src/main.rs", r#"
            fn main() {
                println!("bar");
            }
        "#)
        .publish();
    assert_that(cargo_process("install").arg("cargo-foo"),
                execs().with_status(0));
    assert_that(cargo_process("foo"),
                execs().with_status(0).with_stdout("bar\n"));
    assert_that(cargo_process("--list"),
                execs().with_status(0).with_stdout_contains("    foo\n"));
}

#[test]
fn installs_from_cwd_by_default() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").cwd(p.root()),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
}

#[test]
fn do_not_rebuilds_on_local_install() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("--release"),
                execs().with_status(0));
    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0).with_stderr("[INSTALLING] [..]
[FINISHED] release [optimized] target(s) in [..]
[INSTALLING] [..]
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
"));

    assert!(p.build_dir().exists());
    assert!(p.release_bin("foo").exists());
    assert_that(cargo_home(), has_installed_exe("foo"));
}

#[test]
fn reports_unsuccessful_subcommand_result() {
    Package::new("cargo-fail", "1.0.0")
        .file("src/main.rs", r#"
            fn main() {
                panic!();
            }
        "#)
        .publish();
    assert_that(cargo_process("install").arg("cargo-fail"),
                execs().with_status(0));
    assert_that(cargo_process("--list"),
                execs().with_status(0).with_stdout_contains("    fail\n"));
    assert_that(cargo_process("fail"),
                execs().with_status(101).with_stderr_contains("\
thread '[..]' panicked at 'explicit panic', [..]
"));
}

#[test]
fn git_with_lockfile() {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            bar = { path = "bar" }
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("bar/src/lib.rs", "fn main() {}")
        .file("Cargo.lock", r#"
            [root]
            name = "foo"
            version = "0.1.0"
            dependencies = [ "bar 0.1.0" ]

            [[package]]
            name = "bar"
            version = "0.1.0"
        "#);
    p.build();

    assert_that(cargo_process("install").arg("--git").arg(p.url().to_string()),
                execs().with_status(0));
}

#[test]
fn q_silences_warnings() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("-q").arg("--path").arg(p.root()),
                execs().with_status(0).with_stderr(""));
}

#[test]
fn readonly_dir() {
    pkg("foo", "0.0.1");

    let root = paths::root();
    let dir = &root.join("readonly");
    fs::create_dir(root.join("readonly")).unwrap();
    let mut perms = fs::metadata(dir).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(dir, perms).unwrap();

    assert_that(cargo_process("install").arg("foo").cwd(dir),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
}

#[test]
fn use_path_workspace() {
    Package::new("foo", "1.0.0").publish();
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []

            [workspace]
            members = ["baz"]
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("baz/Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = "1"
        "#)
        .file("baz/src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build"), execs().with_status(0));
    let lock = p.read_lockfile();
    assert_that(p.cargo("install"), execs().with_status(0));
    let lock2 = p.read_lockfile();
    assert!(lock == lock2, "different lockfiles");
}

#[test]
fn vers_precise() {
    pkg("foo", "0.1.1");
    pkg("foo", "0.1.2");

    assert_that(cargo_process("install").arg("foo").arg("--vers").arg("0.1.1"),
                execs().with_status(0).with_stderr_contains("\
[DOWNLOADING] foo v0.1.1 (registry [..])
"));
}

#[test]
fn legacy_version_requirement() {
    pkg("foo", "0.1.1");

    assert_that(cargo_process("install").arg("foo").arg("--vers").arg("0.1"),
                execs().with_status(0).with_stderr_contains("\
warning: the `--vers` provided, `0.1`, is not a valid semver version

historically Cargo treated this as a semver version requirement accidentally
and will continue to do so, but this behavior will be removed eventually
"));
}

#[test]
fn test_install_git_cannot_be_a_base_url() {
    assert_that(cargo_process("install").arg("--git").arg("github.com:rust-lang-nursery/rustfmt.git"),
                execs().with_status(101).with_stderr("\
error: invalid url `github.com:rust-lang-nursery/rustfmt.git`: cannot-be-a-base-URLs are not supported
"));
}
