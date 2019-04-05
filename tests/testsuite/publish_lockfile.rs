use std;
use std::fs::File;

use crate::support::{git, paths, project, publish::validate_crate_contents};

#[test]
fn package_lockfile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["publish-lockfile"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            publish-lockfile = true
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[WARNING] manifest has no documentation[..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
src/main.rs
",
        )
        .run();
    p.cargo("package")
        .masquerade_as_nightly_cargo()
        .with_stdout("")
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.toml", "Cargo.toml.orig", "Cargo.lock", "src/main.rs"],
        &[],
    );
}

#[test]
fn package_lockfile_git_repo() {
    let p = project().build();

    // Create a Git repository containing a minimal Rust project.
    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["publish-lockfile"]

            [project]
            name = "foo"
            version = "0.0.1"
            license = "MIT"
            description = "foo"
            documentation = "foo"
            homepage = "foo"
            repository = "foo"
            publish-lockfile = true
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("package -l")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
src/main.rs
",
        )
        .run();
}

#[test]
fn no_lock_file_with_library() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["publish-lockfile"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            publish-lockfile = true
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package").masquerade_as_nightly_cargo().run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[],
    );
}

#[test]
fn lock_file_and_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["foo"]
        "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
            cargo-features = ["publish-lockfile"]

            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            publish-lockfile = true
        "#,
        )
        .file("foo/src/main.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .cwd("foo")
        .masquerade_as_nightly_cargo()
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/main.rs", "Cargo.lock"],
        &[],
    );
}
