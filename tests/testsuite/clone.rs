use std::fs;
use std::path::PathBuf;

use cargotest::cargo_process;
use cargotest::support::{paths, git, project_in_home, execs,
                         Project, registry::Package};
use hamcrest::{assert_that, existing_dir};


const FOO_CRATE_NAME: &str = "foo";
const BAR_CRATE_NAME: &str = "bar";

fn pkg(name: &str, vers: &str) {
    Package::new(name, vers)
        .file("src/lib.rs", "")
        .file(
            "src/main.rs",
            &format!("extern crate {};
                      fn main() {{}}",
                     name),
        )
        .publish();
}

fn proj(name: &str, vers: &str) -> Project {
    project_in_home(name)
        .file("Cargo.toml",
              &format!("\
                        [package]
                        name = \"{name}\"
                        version = \"{vers}\"
                        authors = []",
                       name = name, vers = vers))
        .file("src/main.rs", "fn main() {}")
        .build()
}

fn cwd_clone_path(name: &str) -> PathBuf {
    paths::root().join(name)
}

#[test]
fn simple() {
    pkg(FOO_CRATE_NAME, "0.0.1");

    assert_that(
        cargo_process().arg("clone").arg(FOO_CRATE_NAME),
        execs().with_status(0).with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.1 (registry [..])
[CLONING] foo v0.0.1"
        )
    );
    assert_that(cwd_clone_path(FOO_CRATE_NAME), existing_dir());
}

#[test]
fn pick_max_version() {
    pkg(FOO_CRATE_NAME, "0.0.1");
    pkg(FOO_CRATE_NAME, "0.0.2");

    assert_that(
        cargo_process().arg("clone").arg(FOO_CRATE_NAME),
        execs().with_status(0).with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.2 (registry [..])
[CLONING] foo v0.0.2"
        )
    );
    assert_that(cwd_clone_path(FOO_CRATE_NAME), existing_dir());
}

#[test]
fn missing() {
    pkg(FOO_CRATE_NAME, "0.0.1");
    assert_that(
        cargo_process().arg("clone").arg(BAR_CRATE_NAME),
        execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[ERROR] could not find `bar` in registry `[..]`"
        )
    );
}

#[test]
fn bad_version() {
    pkg(FOO_CRATE_NAME, "0.0.1");
    assert_that(
        cargo_process().arg("clone").arg(FOO_CRATE_NAME).arg("--vers=0.2.0"),
        execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[ERROR] could not find `foo` in registry `[..]` with version `=0.2.0`"
        )
    );
}

#[test]
fn no_crate() {
    pkg(FOO_CRATE_NAME, "0.0.1");
    assert_that(
        cargo_process().arg("clone"),
        execs().with_status(101).with_stderr("\
[UPDATING] registry [..]
[ERROR] must specify a crate to clone [..]"
        )
    );
}

#[test]
fn path_crate() {
    let p = proj(FOO_CRATE_NAME, "0.1.0");

    assert_that(
        cargo_process().arg("clone").arg("--path").arg(p.root()),
        execs().with_status(0),
    );
    assert_that(cwd_clone_path(FOO_CRATE_NAME), existing_dir());
}

#[test]
fn twice() {
    let p = proj(FOO_CRATE_NAME, "0.1.0");

    assert_that(
        cargo_process().arg("clone").arg("--path").arg(p.root()),
        execs().with_status(0),
    );
    assert_that(
        cargo_process().arg("clone").arg("--path").arg(p.root()),
        execs().with_status(101).with_stderr("\
[CLONING] foo v0.1.0 [..]
[ERROR] Directory `[..]` already exists. Add --force to overwrite"
        )
    );
}

#[test]
fn force() {
    let p = proj(FOO_CRATE_NAME, "0.1.0");

    assert_that(
        cargo_process().arg("clone").arg("--path").arg(p.root()),
        execs().with_status(0),
    );

    let p = proj(FOO_CRATE_NAME, "0.2.0");

    assert_that(
        cargo_process().arg("clone")
            .arg("--force")
            .arg("--path")
            .arg(p.root()),
        execs().with_status(0).with_stderr("\
[CLONING] foo v0.2.0 [..]
[REPLACING] [..]foo[..]"
        )
    );
}

#[test]
fn git_repo() {
    let p = git::repo(&paths::home().join(FOO_CRATE_NAME))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // use `--locked` to test that we don't even try to write a lockfile
    assert_that(
        cargo_process().arg("clone")
            .arg("--locked")
            .arg("--git")
            .arg(p.url().to_string()),
        execs().with_status(0).with_stderr("\
[UPDATING] git repository `[..]`
[CLONING] foo v0.1.0 ([..])"
        )
    );
    assert_that(cwd_clone_path(FOO_CRATE_NAME), existing_dir());
}

#[test]
fn vers_precise() {
    pkg(FOO_CRATE_NAME, "0.1.1");
    pkg(FOO_CRATE_NAME, "0.1.2");

    assert_that(
        cargo_process().arg("clone")
            .arg(FOO_CRATE_NAME)
            .arg("--vers")
            .arg("0.1.1"),
        execs().with_status(0).with_stderr_contains("\
[DOWNLOADING] foo v0.1.1 (registry [..])"
        )
    );
}

#[test]
fn version_precise() {
    pkg(FOO_CRATE_NAME, "0.1.1");
    pkg(FOO_CRATE_NAME, "0.1.2");

    assert_that(
        cargo_process().arg("clone")
            .arg(FOO_CRATE_NAME)
            .arg("--version")
            .arg("0.1.1"),
        execs().with_status(0).with_stderr_contains("\
[DOWNLOADING] foo v0.1.1 (registry [..])"
        )
    );
}

#[test]
fn not_both_vers_and_version() {
    pkg(FOO_CRATE_NAME, "0.1.1");
    pkg(FOO_CRATE_NAME, "0.1.2");

    assert_that(
        cargo_process().arg("clone")
            .arg(FOO_CRATE_NAME)
            .arg("--version")
            .arg("0.1.1")
            .arg("--vers")
            .arg("0.1.2"),
        execs().with_status(1).with_stderr_contains("\
error: The argument '--version <VERSION>' was provided more than once, \
but cannot be used multiple times"
        ),
    );
}

#[test]
fn prefix() {
    pkg(FOO_CRATE_NAME, "0.0.1");

    assert_that(
        cargo_process().arg("clone").arg(FOO_CRATE_NAME)
            .arg(format!("--prefix={}", paths::root().join("test_prefix").display())),
        execs().with_status(0).with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.1 (registry [..])
[CLONING] foo v0.0.1"
        )
    );
    assert_that(paths::root().join("test_prefix").join(FOO_CRATE_NAME),
                existing_dir());
}

#[test]
fn prefix_already_exists() {
    pkg(FOO_CRATE_NAME, "0.0.1");

    let prefix = paths::root().join("test_prefix");
    fs::create_dir_all(&prefix.join(FOO_CRATE_NAME)).unwrap();

    assert_that(
        cargo_process().arg("clone").arg(FOO_CRATE_NAME)
            .arg(&format!("--prefix={}", prefix.display())),
        execs().with_status(101).with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.0.1 (registry [..])
[CLONING] foo v0.0.1
[ERROR] Directory `[..]` already exists. Add --force to overwrite"
        )
    );
}
