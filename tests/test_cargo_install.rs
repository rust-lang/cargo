use std::fmt;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use support::paths::CargoPathExt;

use cargo::util::ProcessBuilder;
use hamcrest::{assert_that, existing_file, is_not, Matcher, MatchResult};

use support::{project, execs};
use support::paths;
use support::registry::Package;
use support::git;

pub use self::InstalledExe as has_installed_exe;

fn setup() {
}

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut p = ::cargo_process();
    p.arg(s);
    return p
}

fn pkg(name: &str, vers: &str) {
    Package::new(name, vers)
        .file("src/lib.rs", "")
        .file("src/main.rs", &format!("
            extern crate {};
            fn main() {{}}
        ", name))
        .publish()
}

fn exe(name: &str) -> String {
    if cfg!(windows) {format!("{}.exe", name)} else {name.to_string()}
}

pub fn cargo_home() -> PathBuf {
    paths::home().join(".cargo")
}

pub struct InstalledExe(pub &'static str);

impl<P: AsRef<Path>> Matcher<P> for InstalledExe {
    fn matches(&self, path: P) -> MatchResult {
        let path = path.as_ref().join("bin").join(exe(self.0));
        existing_file().matches(&path)
    }
}

impl fmt::Display for InstalledExe {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "installed exe `{}`", self.0)
    }
}

test!(simple {
    pkg("foo", "0.0.1");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0).with_stdout(&format!("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.1 (registry file://[..])
[COMPILING] foo v0.0.1 (registry file://[..])
[INSTALLING] {home}[..]bin[..]foo[..]
",
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));

    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(0).with_stdout(&format!("\
[REMOVING] {home}[..]bin[..]foo[..]
",
        home = cargo_home().display())));
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));
});

test!(pick_max_version {
    pkg("foo", "0.0.1");
    pkg("foo", "0.0.2");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0).with_stdout(&format!("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.2 (registry file://[..])
[COMPILING] foo v0.0.2 (registry file://[..])
[INSTALLING] {home}[..]bin[..]foo[..]
",
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));
});

test!(missing {
    pkg("foo", "0.0.1");
    assert_that(cargo_process("install").arg("bar"),
                execs().with_status(101).with_stderr(&format!("\
[ERROR] could not find `bar` in `registry file://[..]`
")));
});

test!(bad_version {
    pkg("foo", "0.0.1");
    assert_that(cargo_process("install").arg("foo").arg("--vers=0.2.0"),
                execs().with_status(101).with_stderr(&format!("\
[ERROR] could not find `foo` in `registry file://[..]` with version `0.2.0`
")));
});

test!(no_crate {
    assert_that(cargo_process("install"),
                execs().with_status(101).with_stderr(&format!("\
[ERROR] `[..]` is not a crate root; specify a crate to install [..]

Caused by:
  failed to read `[..]Cargo.toml`

Caused by:
  [..] (os error [..])
")));
});

test!(install_location_precedence {
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
});

test!(install_path {
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
                execs().with_status(101).with_stderr(&format!("\
[ERROR] binary `foo[..]` already exists in destination as part of `foo v0.1.0 [..]`
Add --force to overwrite
")));
});

test!(multiple_crates_error {
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
                execs().with_status(101).with_stderr(&format!("\
[ERROR] multiple packages with binaries found: bar, foo
")));
});

test!(multiple_crates_select {
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
});

test!(multiple_crates_auto_binaries {
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
});

test!(multiple_crates_auto_examples {
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
});

test!(no_binaries_or_examples {
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
                execs().with_status(101).with_stderr(&format!("\
[ERROR] no packages found with binaries or examples
")));
});

test!(no_binaries {
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
                execs().with_status(101).with_stderr(&format!("\
[ERROR] specified package has no binaries
")));
});

test!(examples {
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
});

test!(install_twice {
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
                execs().with_status(101).with_stderr(&format!("\
[ERROR] binary `foo-bin1[..]` already exists in destination as part of `foo v0.1.0 ([..])`
binary `foo-bin2[..]` already exists in destination as part of `foo v0.1.0 ([..])`
Add --force to overwrite
")));
});

test!(install_force {
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
                execs().with_status(0).with_stdout(&format!("\
[COMPILING] foo v0.2.0 ([..])
[REPLACING] {home}[..]bin[..]foo[..]
",
        home = cargo_home().display())));

    assert_that(cargo_process("install").arg("--list"),
                execs().with_status(0).with_stdout("\
foo v0.2.0 ([..]):
    foo[..]
"));
});

test!(install_force_partial_overlap {
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
                execs().with_status(0).with_stdout(&format!("\
[COMPILING] foo v0.2.0 ([..])
[INSTALLING] {home}[..]bin[..]foo-bin3[..]
[REPLACING] {home}[..]bin[..]foo-bin2[..]
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
});

test!(install_force_bin {
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
                execs().with_status(0).with_stdout(&format!("\
[COMPILING] foo v0.2.0 ([..])
[REPLACING] {home}[..]bin[..]foo-bin2[..]
",
        home = cargo_home().display())));

    assert_that(cargo_process("install").arg("--list"),
                execs().with_status(0).with_stdout("\
foo v0.1.0 ([..]):
    foo-bin1[..]
foo v0.2.0 ([..]):
    foo-bin2[..]
"));
});

test!(compile_failure {
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
                execs().with_status(101).with_stderr(&format!("\
error: main function not found
error: aborting due to previous error
[ERROR] failed to compile `foo v0.1.0 (file://[..])`, intermediate artifacts can be \
    found at `[..]target`

Caused by:
  Could not compile `foo`.

To learn more, run the command again with --verbose.
")));
});

test!(git_repo {
    let p = git::repo(&paths::root().join("foo"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--git").arg(p.url().to_string()),
                execs().with_status(0).with_stdout(&format!("\
[UPDATING] git repository `[..]`
[COMPILING] foo v0.1.0 ([..])
[INSTALLING] {home}[..]bin[..]foo[..]
",
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), has_installed_exe("foo"));
});

test!(list {
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
bar v0.2.1 (registry [..]):
    bar[..]
foo v0.0.1 (registry [..]):
    foo[..]
"));
});

test!(uninstall_pkg_does_not_exist {
    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(101).with_stderr(&format!("\
[ERROR] package id specification `foo` matched no packages
")));
});

test!(uninstall_bin_does_not_exist {
    pkg("foo", "0.0.1");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0));
    assert_that(cargo_process("uninstall").arg("foo").arg("--bin=bar"),
                execs().with_status(101).with_stderr(&format!("\
[ERROR] binary `bar[..]` not installed as part of `foo v0.0.1 ([..])`
")));
});

test!(uninstall_piecemeal {
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
                execs().with_status(0).with_stdout("\
[REMOVING] [..]bar[..]
"));

    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), is_not(has_installed_exe("bar")));

    assert_that(cargo_process("uninstall").arg("foo").arg("--bin=foo"),
                execs().with_status(0).with_stdout("\
[REMOVING] [..]foo[..]
"));
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));

    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(101).with_stderr(&format!("\
[ERROR] package id specification `foo` matched no packages
")));
});

test!(subcommand_works_out_of_the_box {
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
});

test!(installs_from_cwd_by_default {
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
});

test!(do_not_rebuilds_on_local_install {
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
                execs().with_status(0).with_stdout("\
[INSTALLING] [..]
").with_stderr("\
warning: be sure to add `[..]` to your PATH to be able to run the installed binaries
"));

    assert!(p.build_dir().c_exists());
    assert!(p.release_bin("foo").c_exists());
    assert_that(cargo_home(), has_installed_exe("foo"));
});

test!(reports_unsuccessful_subcommand_result {
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
thread '<main>' panicked at 'explicit panic', [..]
"));
});

test!(git_with_lockfile {
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
            dependencies = [ "b 0.1.0" ]

            [[package]]
            name = "bar"
            version = "0.1.0"
        "#);
    p.build();

    assert_that(cargo_process("install").arg("--git").arg(p.url().to_string()),
                execs().with_status(0));
});

test!(q_silences_warnings {
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
});

test!(readonly_dir {
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
});
