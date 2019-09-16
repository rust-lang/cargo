use cargo_test_support::git;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, project, Project};

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
    p.cargo("build").run();
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
    p.cargo("build").run();
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
    p.cargo("build").cwd("foo").run();
    p.cargo("build").cwd("bar").run();
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
    p.cargo("build").run();
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
