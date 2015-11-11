use std::fmt;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use cargo::util::{process, ProcessBuilder};
use hamcrest::{assert_that, existing_file, is_not, Matcher, MatchResult};

use support::{project, execs, cargo_dir};
use support::{UPDATING, DOWNLOADING, COMPILING, INSTALLING, REMOVING};
use support::paths;
use support::registry::Package;
use support::git;

use self::InstalledExe as has_installed_exe;

fn setup() {
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

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut p = process(&cargo_dir().join("cargo")).unwrap();
    p.arg(s).cwd(&paths::root())
     .env("HOME", &paths::home())
     .env_remove("CARGO_HOME");
    return p;
}

fn exe(name: &str) -> String {
    if cfg!(windows) {format!("{}.exe", name)} else {name.to_string()}
}

fn cargo_home() -> PathBuf {
    paths::home().join(".cargo")
}

struct InstalledExe(&'static str);

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
{updating} registry `[..]`
{downloading} foo v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 (registry file://[..])
{installing} {home}[..]bin[..]foo[..]
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        installing = INSTALLING,
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));

    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(0).with_stdout(&format!("\
{removing} {home}[..]bin[..]foo[..]
",
        removing = REMOVING,
        home = cargo_home().display())));
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));
});

test!(pick_max_version {
    pkg("foo", "0.0.1");
    pkg("foo", "0.0.2");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{downloading} foo v0.0.2 (registry file://[..])
{compiling} foo v0.0.2 (registry file://[..])
{installing} {home}[..]bin[..]foo[..]
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        installing = INSTALLING,
        home = cargo_home().display())));
    assert_that(cargo_home(), has_installed_exe("foo"));
});

test!(missing {
    pkg("foo", "0.0.1");
    assert_that(cargo_process("install").arg("bar"),
                execs().with_status(101).with_stderr("\
could not find `bar` in `registry file://[..]`
"));
});

test!(bad_version {
    pkg("foo", "0.0.1");
    assert_that(cargo_process("install").arg("foo").arg("--vers=0.2.0"),
                execs().with_status(101).with_stderr("\
could not find `foo` in `registry file://[..]` with version `0.2.0`
"));
});

test!(no_crate {
    assert_that(cargo_process("install"),
                execs().with_status(101).with_stderr("\
must specify a crate to install from crates.io
"));
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
                execs().with_status(101).with_stderr("\
binary `foo[..]` already exists in destination as part of `foo v0.1.0 [..]`
"));
});

test!(multiple_crates_error {
    let p = project("foo")
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

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(101).with_stderr("\
multiple packages with binaries found: bar, foo
"));
});

test!(multiple_crates_select {
    let p = project("foo")
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

    assert_that(cargo_process("install").arg("--path").arg(p.root()).arg("foo"),
                execs().with_status(0));
    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), is_not(has_installed_exe("bar")));

    assert_that(cargo_process("install").arg("--path").arg(p.root()).arg("bar"),
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
                execs().with_status(101).with_stderr("\
no packages found with binaries or examples
"));
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
                execs().with_status(101).with_stderr("\
specified package has no binaries
"));
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
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(0));
    assert_that(cargo_process("install").arg("--path").arg(p.root()),
                execs().with_status(101).with_stderr("\
binary `foo[..]` already exists in destination as part of `foo v0.1.0 ([..])`
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
                execs().with_status(101).with_stderr("\
error: main function not found
error: aborting due to previous error
failed to compile `foo v0.1.0 (file://[..])`, intermediate artifacts can be \
    found at `[..]target-install`

Caused by:
  Could not compile `foo`.

To learn more, run the command again with --verbose.
"));
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
{updating} git repository `[..]`
{compiling} foo v0.1.0 ([..])
{installing} {home}[..]bin[..]foo[..]
",
        updating = UPDATING,
        compiling = COMPILING,
        installing = INSTALLING,
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
                execs().with_status(101).with_stderr("\
package id specification `foo` matched no packages
"));
});

test!(uninstall_bin_does_not_exist {
    pkg("foo", "0.0.1");

    assert_that(cargo_process("install").arg("foo"),
                execs().with_status(0));
    assert_that(cargo_process("uninstall").arg("foo").arg("--bin=bar"),
                execs().with_status(101).with_stderr("\
binary `bar[..]` not installed as part of `foo v0.0.1 ([..])`
"));
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
                execs().with_status(0).with_stdout(&format!("\
{removing} [..]bar[..]
", removing = REMOVING)));

    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), is_not(has_installed_exe("bar")));

    assert_that(cargo_process("uninstall").arg("foo").arg("--bin=foo"),
                execs().with_status(0).with_stdout(&format!("\
{removing} [..]foo[..]
", removing = REMOVING)));
    assert_that(cargo_home(), is_not(has_installed_exe("foo")));

    assert_that(cargo_process("uninstall").arg("foo"),
                execs().with_status(101).with_stderr("\
package id specification `foo` matched no packages
"));
});
