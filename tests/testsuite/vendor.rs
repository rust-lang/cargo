//! Tests for the `cargo vendor` command.
//!
//! Note that every test here uses `--respect-source-config` so that the
//! "fake" crates.io is used. Otherwise `vendor` would download the crates.io
//! index from the network.

use std::fs;

use cargo_test_support::git;
use cargo_test_support::registry::{self, Package, RegistryBuilder};
use cargo_test_support::{basic_lib_manifest, basic_manifest, paths, project, Project};

#[cargo_test]
fn vendor_simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                log = "0.3.5"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("log", "0.3.5").publish();

    p.cargo("vendor --respect-source-config").run();
    let lock = p.read_file("vendor/log/Cargo.toml");
    assert!(lock.contains("version = \"0.3.5\""));

    add_vendor_config(&p);
    p.cargo("check").run();
}

#[cargo_test]
fn vendor_sample_config() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                log = "0.3.5"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("log", "0.3.5").publish();

    p.cargo("vendor --respect-source-config")
        .with_stdout(
            r#"[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
"#,
        )
        .run();
}

#[cargo_test]
fn vendor_sample_config_alt_registry() {
    let registry = RegistryBuilder::new().alternative().http_index().build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                log = { version = "0.3.5", registry = "alternative" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("log", "0.3.5").alternative(true).publish();

    p.cargo("vendor --respect-source-config")
        .with_stdout(format!(
            r#"[source."{0}"]
registry = "{0}"
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
"#,
            registry.index_url()
        ))
        .run();
}

#[cargo_test]
fn vendor_path_specified() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                log = "0.3.5"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("log", "0.3.5").publish();

    let path = if cfg!(windows) {
        r#"deps\.vendor"#
    } else {
        "deps/.vendor"
    };

    let output = p
        .cargo("vendor --respect-source-config")
        .arg(path)
        .exec_with_output()
        .unwrap();
    // Assert against original output to ensure that
    // path is normalized by `ops::vendor` on Windows.
    assert_eq!(
        &String::from_utf8(output.stdout).unwrap(),
        r#"[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "deps/.vendor"
"#
    );

    let lock = p.read_file("deps/.vendor/log/Cargo.toml");
    assert!(lock.contains("version = \"0.3.5\""));
}

fn add_vendor_config(p: &Project) {
    p.change_file(
        ".cargo/config",
        r#"
            [source.crates-io]
            replace-with = 'vendor'

            [source.vendor]
            directory = 'vendor'
        "#,
    );
}

#[cargo_test]
fn package_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("bar", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                exclude = [".*", "!.include", "!.dotdir/include"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(".exclude", "")
        .file(".include", "")
        .file(".dotdir/exclude", "")
        .file(".dotdir/include", "")
        .publish();

    p.cargo("vendor --respect-source-config").run();
    let csum = p.read_file("vendor/bar/.cargo-checksum.json");
    assert!(csum.contains(".include"));
    assert!(!csum.contains(".exclude"));
    assert!(!csum.contains(".dotdir/exclude"));
    // Gitignore doesn't re-include a file in an excluded parent directory,
    // even if negating it explicitly.
    assert!(!csum.contains(".dotdir/include"));
}

#[cargo_test]
fn two_versions() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bitflags = "0.8.0"
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"

                [dependencies]
                bitflags = "0.7.0"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    Package::new("bitflags", "0.7.0").publish();
    Package::new("bitflags", "0.8.0").publish();

    p.cargo("vendor --respect-source-config").run();

    let lock = p.read_file("vendor/bitflags/Cargo.toml");
    assert!(lock.contains("version = \"0.8.0\""));
    let lock = p.read_file("vendor/bitflags-0.7.0/Cargo.toml");
    assert!(lock.contains("version = \"0.7.0\""));

    add_vendor_config(&p);
    p.cargo("check").run();
}

#[cargo_test]
fn two_explicit_versions() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bitflags = "0.8.0"
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"

                [dependencies]
                bitflags = "0.7.0"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    Package::new("bitflags", "0.7.0").publish();
    Package::new("bitflags", "0.8.0").publish();

    p.cargo("vendor --respect-source-config --versioned-dirs")
        .run();

    let lock = p.read_file("vendor/bitflags-0.8.0/Cargo.toml");
    assert!(lock.contains("version = \"0.8.0\""));
    let lock = p.read_file("vendor/bitflags-0.7.0/Cargo.toml");
    assert!(lock.contains("version = \"0.7.0\""));

    add_vendor_config(&p);
    p.cargo("check").run();
}

#[cargo_test]
fn help() {
    let p = project().build();
    p.cargo("vendor -h").run();
}

#[cargo_test]
fn update_versions() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bitflags = "0.7.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("bitflags", "0.7.0").publish();
    Package::new("bitflags", "0.8.0").publish();

    p.cargo("vendor --respect-source-config").run();

    let lock = p.read_file("vendor/bitflags/Cargo.toml");
    assert!(lock.contains("version = \"0.7.0\""));

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bitflags = "0.8.0"
        "#,
    );
    p.cargo("vendor --respect-source-config").run();

    let lock = p.read_file("vendor/bitflags/Cargo.toml");
    assert!(lock.contains("version = \"0.8.0\""));
}

#[cargo_test]
fn two_lockfiles() {
    let p = project()
        .no_manifest()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bitflags = "=0.7.0"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"

                [dependencies]
                bitflags = "=0.8.0"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    Package::new("bitflags", "0.7.0").publish();
    Package::new("bitflags", "0.8.0").publish();

    p.cargo("vendor --respect-source-config -s bar/Cargo.toml --manifest-path foo/Cargo.toml")
        .run();

    let lock = p.read_file("vendor/bitflags/Cargo.toml");
    assert!(lock.contains("version = \"0.8.0\""));
    let lock = p.read_file("vendor/bitflags-0.7.0/Cargo.toml");
    assert!(lock.contains("version = \"0.7.0\""));

    add_vendor_config(&p);
    p.cargo("check").cwd("foo").run();
    p.cargo("check").cwd("bar").run();
}

#[cargo_test]
fn test_sync_argument() {
    let p = project()
        .no_manifest()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bitflags = "=0.7.0"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"

                [dependencies]
                bitflags = "=0.8.0"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"

                [dependencies]
                bitflags = "=0.8.0"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    Package::new("bitflags", "0.7.0").publish();
    Package::new("bitflags", "0.8.0").publish();

    p.cargo("vendor --respect-source-config --manifest-path foo/Cargo.toml -s bar/Cargo.toml baz/Cargo.toml test_vendor")
        .with_stderr("\
error: unexpected argument 'test_vendor' found

Usage: cargo[EXE] vendor [OPTIONS] [path]

For more information, try '--help'.",
        )
        .with_status(1)
        .run();

    p.cargo("vendor --respect-source-config --manifest-path foo/Cargo.toml -s bar/Cargo.toml -s baz/Cargo.toml test_vendor")
        .run();

    let lock = p.read_file("test_vendor/bitflags/Cargo.toml");
    assert!(lock.contains("version = \"0.8.0\""));
    let lock = p.read_file("test_vendor/bitflags-0.7.0/Cargo.toml");
    assert!(lock.contains("version = \"0.7.0\""));
}

#[cargo_test]
fn delete_old_crates() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bitflags = "=0.7.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("bitflags", "0.7.0").publish();
    Package::new("log", "0.3.5").publish();

    p.cargo("vendor --respect-source-config").run();
    p.read_file("vendor/bitflags/Cargo.toml");

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            log = "=0.3.5"
        "#,
    );

    p.cargo("vendor --respect-source-config").run();
    let lock = p.read_file("vendor/log/Cargo.toml");
    assert!(lock.contains("version = \"0.3.5\""));
    assert!(!p.root().join("vendor/bitflags/Cargo.toml").exists());
}

#[cargo_test]
fn ignore_files() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                url = "1.4.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("url", "1.4.1")
        .file("src/lib.rs", "")
        .file("foo.orig", "")
        .file(".gitignore", "")
        .file(".gitattributes", "")
        .file("foo.rej", "")
        .publish();

    p.cargo("vendor --respect-source-config").run();
    let csum = p.read_file("vendor/url/.cargo-checksum.json");
    assert!(!csum.contains("foo.orig"));
    assert!(!csum.contains(".gitignore"));
    assert!(!csum.contains(".gitattributes"));
    assert!(!csum.contains(".cargo-ok"));
    assert!(!csum.contains("foo.rej"));
}

#[cargo_test]
fn included_files_only() {
    let git = git::new("a", |p| {
        p.file("Cargo.toml", &basic_lib_manifest("a"))
            .file("src/lib.rs", "")
            .file(".gitignore", "a")
            .file("a/b.md", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("vendor --respect-source-config").run();
    let csum = p.read_file("vendor/a/.cargo-checksum.json");
    assert!(!csum.contains("a/b.md"));
}

#[cargo_test]
fn dependent_crates_in_crates() {
    let git = git::new("a", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"

                [dependencies]
                b = { path = 'b' }
            "#,
        )
        .file("src/lib.rs", "")
        .file("b/Cargo.toml", &basic_lib_manifest("b"))
        .file("b/src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("vendor --respect-source-config").run();
    p.read_file("vendor/a/.cargo-checksum.json");
    p.read_file("vendor/b/.cargo-checksum.json");
}

#[cargo_test]
fn vendoring_git_crates() {
    let git = git::new("git", |p| {
        p.file("Cargo.toml", &basic_lib_manifest("serde_derive"))
            .file("src/lib.rs", "")
            .file("src/wut.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies.serde]
                    version = "0.5.0"

                    [dependencies.serde_derive]
                    version = "0.5.0"

                    [patch.crates-io]
                    serde_derive = {{ git = '{}' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();
    Package::new("serde", "0.5.0")
        .dep("serde_derive", "0.5")
        .publish();
    Package::new("serde_derive", "0.5.0").publish();

    p.cargo("vendor --respect-source-config").run();
    p.read_file("vendor/serde_derive/src/wut.rs");

    add_vendor_config(&p);
    p.cargo("check").run();
}

#[cargo_test]
fn git_simple() {
    let git = git::new("git", |p| {
        p.file("Cargo.toml", &basic_lib_manifest("a"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("vendor --respect-source-config").run();
    let csum = p.read_file("vendor/a/.cargo-checksum.json");
    assert!(csum.contains("\"package\":null"));
}

#[cargo_test]
fn git_diff_rev() {
    let (git_project, git_repo) = git::new_repo("git", |p| {
        p.file("Cargo.toml", &basic_manifest("a", "0.1.0"))
            .file("src/lib.rs", "")
    });
    let url = git_project.url();
    let ref_1 = "v0.1.0";
    let ref_2 = "v0.2.0";

    git::tag(&git_repo, ref_1);

    git_project.change_file("Cargo.toml", &basic_manifest("a", "0.2.0"));
    git::add(&git_repo);
    git::commit(&git_repo);
    git::tag(&git_repo, ref_2);

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    a_1 = {{ package = "a", git = '{url}', rev = '{ref_1}' }}
                    a_2 = {{ package = "a", git = '{url}', rev = '{ref_2}' }}
                "#
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("vendor --respect-source-config")
        .with_stdout(
            r#"[source."git+file://[..]/git?rev=v0.1.0"]
git = [..]
rev = "v0.1.0"
replace-with = "vendored-sources"

[source."git+file://[..]/git?rev=v0.2.0"]
git = [..]
rev = "v0.2.0"
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
"#,
        )
        .run();
}

#[cargo_test]
fn git_duplicate() {
    let git = git::new("a", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"

                [dependencies]
                b = { path = 'b' }
            "#,
        )
        .file("src/lib.rs", "")
        .file("b/Cargo.toml", &basic_lib_manifest("b"))
        .file("b/src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    a = {{ git = '{}' }}
                    b = '0.5.0'

                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();
    Package::new("b", "0.5.0").publish();

    p.cargo("vendor --respect-source-config")
        .with_stderr(
            "\
[UPDATING] [..]
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
error: failed to sync

Caused by:
  found duplicate version of package `b v0.5.0` vendored from two sources:

  <tab>source 1: [..]
  <tab>source 2: [..]
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn git_complex() {
    let git_b = git::new("git_b", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"

                [dependencies]
                dep_b = { path = 'dep_b' }
            "#,
        )
        .file("src/lib.rs", "")
        .file("dep_b/Cargo.toml", &basic_lib_manifest("dep_b"))
        .file("dep_b/src/lib.rs", "")
    });

    let git_a = git::new("git_a", |p| {
        p.file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "a"
                    version = "0.1.0"

                    [dependencies]
                    b = {{ git = '{}' }}
                    dep_a = {{ path = 'dep_a' }}
                "#,
                git_b.url()
            ),
        )
        .file("src/lib.rs", "")
        .file("dep_a/Cargo.toml", &basic_lib_manifest("dep_a"))
        .file("dep_a/src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                git_a.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    let output = p
        .cargo("vendor --respect-source-config")
        .exec_with_output()
        .unwrap();
    let output = String::from_utf8(output.stdout).unwrap();
    p.change_file(".cargo/config", &output);

    p.cargo("check -v")
        .with_stderr_contains("[..]foo/vendor/a/src/lib.rs[..]")
        .with_stderr_contains("[..]foo/vendor/dep_a/src/lib.rs[..]")
        .with_stderr_contains("[..]foo/vendor/b/src/lib.rs[..]")
        .with_stderr_contains("[..]foo/vendor/dep_b/src/lib.rs[..]")
        .run();
}

#[cargo_test]
fn depend_on_vendor_dir_not_deleted() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                libc = "0.2.30"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("libc", "0.2.30").publish();

    p.cargo("vendor --respect-source-config").run();
    assert!(p.root().join("vendor/libc").is_dir());

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            libc = "0.2.30"

            [patch.crates-io]
            libc = { path = 'vendor/libc' }
        "#,
    );

    p.cargo("vendor --respect-source-config").run();
    assert!(p.root().join("vendor/libc").is_dir());
}

#[cargo_test]
fn ignore_hidden() {
    // Don't delete files starting with `.`
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            [dependencies]
            bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("vendor --respect-source-config").run();
    // Add a `.git` directory.
    let repo = git::init(&p.root().join("vendor"));
    git::add(&repo);
    git::commit(&repo);
    assert!(p.root().join("vendor/.git").exists());
    // Vendor again, shouldn't change anything.
    p.cargo("vendor --respect-source-config").run();
    // .git should not be removed.
    assert!(p.root().join("vendor/.git").exists());
    // And just for good measure, make sure no files changed.
    let mut opts = git2::StatusOptions::new();
    assert!(repo
        .statuses(Some(&mut opts))
        .unwrap()
        .iter()
        .all(|status| status.status() == git2::Status::CURRENT));
}

#[cargo_test]
fn config_instructions_works() {
    // Check that the config instructions work for all dependency kinds.
    registry::alt_init();
    Package::new("dep", "0.1.0").publish();
    Package::new("altdep", "0.1.0").alternative(true).publish();
    let git_project = git::new("gitdep", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("gitdep"))
            .file("src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                dep = "0.1"
                altdep = {{version="0.1", registry="alternative"}}
                gitdep = {{git='{}'}}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();
    let output = p
        .cargo("vendor --respect-source-config")
        .exec_with_output()
        .unwrap();
    let output = String::from_utf8(output.stdout).unwrap();
    p.change_file(".cargo/config", &output);

    p.cargo("check -v")
        .with_stderr_contains("[..]foo/vendor/dep/src/lib.rs[..]")
        .with_stderr_contains("[..]foo/vendor/altdep/src/lib.rs[..]")
        .with_stderr_contains("[..]foo/vendor/gitdep/src/lib.rs[..]")
        .run();
}

#[cargo_test]
fn git_crlf_preservation() {
    // Check that newlines don't get changed when you vendor
    // (will only fail if your system is setup with core.autocrlf=true on windows)
    let input = "hello \nthere\nmy newline\nfriends";
    let git_project = git::new("git", |p| {
        p.file("Cargo.toml", &basic_lib_manifest("a"))
            .file("src/lib.rs", input)
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    fs::write(
        paths::home().join(".gitconfig"),
        r#"
            [core]
            autocrlf = true
        "#,
    )
    .unwrap();

    p.cargo("vendor --respect-source-config").run();
    let output = p.read_file("vendor/a/src/lib.rs");
    assert_eq!(input, output);
}

#[cargo_test]
#[cfg(unix)]
fn vendor_preserves_permissions() {
    use std::os::unix::fs::MetadataExt;

    Package::new("bar", "1.0.0")
        .file_with_mode("example.sh", 0o755, "#!/bin/sh")
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("vendor --respect-source-config").run();

    let umask = cargo::util::get_umask();
    let metadata = fs::metadata(p.root().join("vendor/bar/src/lib.rs")).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o644 & !umask);
    let metadata = fs::metadata(p.root().join("vendor/bar/example.sh")).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o755 & !umask);
}

#[cargo_test]
fn no_remote_dependency_no_vendor() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("vendor")
        .with_stderr("There is no dependency to vendor in this project.")
        .run();
    assert!(!p.root().join("vendor").exists());
}

#[cargo_test]
fn vendor_crate_with_ws_inherit() {
    let git = git::new("ws", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
                [workspace.package]
                version = "0.1.0"
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version.workspace = true
            "#,
        )
        .file("bar/src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    bar = {{ git = '{}' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("vendor --respect-source-config").run();
    p.change_file(
        ".cargo/config",
        &format!(
            r#"
                [source."{}"]
                git = "{}"
                replace-with = "vendor"

                [source.vendor]
                directory = "vendor"
            "#,
            git.url(),
            git.url()
        ),
    );

    p.cargo("check -v")
        .with_stderr_contains("[..]foo/vendor/bar/src/lib.rs[..]")
        .run();
}
