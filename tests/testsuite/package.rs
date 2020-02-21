//! Tests for the `cargo package` command.

use std::fs::{read_to_string, File};
use std::io::prelude::*;
use std::path::Path;

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::Package;
use cargo_test_support::{
    basic_manifest, cargo_process, git, path2url, paths, project, publish::validate_crate_contents,
    registry, symlink_supported, t,
};

#[cargo_test]
fn simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            exclude = ["*.txt"]
            license = "MIT"
            description = "foo"
        "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file("src/bar.txt", "") // should be ignored when packaging
        .build();

    p.cargo("package")
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
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
        )
        .run();
    p.cargo("package").with_stdout("").run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        &[],
    );
}

#[cargo_test]
fn metadata_warning() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("package")
        .with_stderr(
            "\
warning: manifest has no description, license, license-file, documentation, \
homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("package")
        .with_stderr(
            "\
warning: manifest has no description, documentation, homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            repository = "bar"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("package")
        .with_stderr(
            "\
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn package_verbose() {
    let root = paths::root().join("all");
    let repo = git::repo(&root)
        .file("Cargo.toml", &basic_manifest("foo", "0.0.1"))
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();
    cargo_process("build").cwd(repo.root()).run();

    println!("package main repo");
    cargo_process("package -v --no-verify")
        .cwd(repo.root())
        .with_stderr(
            "\
[WARNING] manifest has no description[..]
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] foo v0.0.1 ([..])
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] Cargo.lock
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] src/main.rs
",
        )
        .run();

    let f = File::open(&repo.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let vcs_contents = format!(
        r#"{{
  "git": {{
    "sha1": "{}"
  }}
}}
"#,
        repo.revparse_head()
    );
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            ".cargo_vcs_info.json",
        ],
        &[(".cargo_vcs_info.json", &vcs_contents)],
    );

    println!("package sub-repo");
    cargo_process("package -v --no-verify")
        .cwd(repo.root().join("a"))
        .with_stderr(
            "\
[WARNING] manifest has no description[..]
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] a v0.0.1 ([..])
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] src/lib.rs
",
        )
        .run();
}

#[cargo_test]
fn package_verification() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("build").run();
    p.cargo("package")
        .with_stderr(
            "\
[WARNING] manifest has no description[..]
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn vcs_file_collision() {
    let p = project().build();
    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            description = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            documentation = "foo"
            homepage = "foo"
            repository = "foo"
            exclude = ["*.no-existe"]
        "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {}
        "#,
        )
        .file(".cargo_vcs_info.json", "foo")
        .build();
    p.cargo("package")
        .arg("--no-verify")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid inclusion of reserved file name .cargo_vcs_info.json \
in package source
",
        )
        .run();
}

#[cargo_test]
fn path_dependency_no_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"

            [dependencies.bar]
            path = "bar"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] manifest has no documentation, homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[ERROR] all path dependencies must have a version specified when packaging.
dependency `bar` does not specify a version.
",
        )
        .run();
}

#[cargo_test]
fn exclude() {
    let root = paths::root().join("exclude");
    let repo = git::repo(&root)
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            exclude = [
                "*.txt",
                # file in root
                "file_root_1",       # NO_CHANGE (ignored)
                "/file_root_2",      # CHANGING (packaged -> ignored)
                "file_root_3/",      # NO_CHANGE (packaged)
                "file_root_4/*",     # NO_CHANGE (packaged)
                "file_root_5/**",    # NO_CHANGE (packaged)
                # file in sub-dir
                "file_deep_1",       # CHANGING (packaged -> ignored)
                "/file_deep_2",      # NO_CHANGE (packaged)
                "file_deep_3/",      # NO_CHANGE (packaged)
                "file_deep_4/*",     # NO_CHANGE (packaged)
                "file_deep_5/**",    # NO_CHANGE (packaged)
                # dir in root
                "dir_root_1",        # CHANGING (packaged -> ignored)
                "/dir_root_2",       # CHANGING (packaged -> ignored)
                "dir_root_3/",       # CHANGING (packaged -> ignored)
                "dir_root_4/*",      # NO_CHANGE (ignored)
                "dir_root_5/**",     # NO_CHANGE (ignored)
                # dir in sub-dir
                "dir_deep_1",        # CHANGING (packaged -> ignored)
                "/dir_deep_2",       # NO_CHANGE
                "dir_deep_3/",       # CHANGING (packaged -> ignored)
                "dir_deep_4/*",      # CHANGING (packaged -> ignored)
                "dir_deep_5/**",     # CHANGING (packaged -> ignored)
            ]
        "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file("bar.txt", "")
        .file("src/bar.txt", "")
        // File in root.
        .file("file_root_1", "")
        .file("file_root_2", "")
        .file("file_root_3", "")
        .file("file_root_4", "")
        .file("file_root_5", "")
        // File in sub-dir.
        .file("some_dir/file_deep_1", "")
        .file("some_dir/file_deep_2", "")
        .file("some_dir/file_deep_3", "")
        .file("some_dir/file_deep_4", "")
        .file("some_dir/file_deep_5", "")
        // Dir in root.
        .file("dir_root_1/some_dir/file", "")
        .file("dir_root_2/some_dir/file", "")
        .file("dir_root_3/some_dir/file", "")
        .file("dir_root_4/some_dir/file", "")
        .file("dir_root_5/some_dir/file", "")
        // Dir in sub-dir.
        .file("some_dir/dir_deep_1/some_dir/file", "")
        .file("some_dir/dir_deep_2/some_dir/file", "")
        .file("some_dir/dir_deep_3/some_dir/file", "")
        .file("some_dir/dir_deep_4/some_dir/file", "")
        .file("some_dir/dir_deep_5/some_dir/file", "")
        .build();

    cargo_process("package --no-verify -v")
        .cwd(repo.root())
        .with_stdout("")
        .with_stderr(
            "\
[WARNING] manifest has no description[..]
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] foo v0.0.1 ([..])
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] Cargo.lock
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] file_root_3
[ARCHIVING] file_root_4
[ARCHIVING] file_root_5
[ARCHIVING] some_dir/dir_deep_2/some_dir/file
[ARCHIVING] some_dir/dir_deep_4/some_dir/file
[ARCHIVING] some_dir/dir_deep_5/some_dir/file
[ARCHIVING] some_dir/file_deep_2
[ARCHIVING] some_dir/file_deep_3
[ARCHIVING] some_dir/file_deep_4
[ARCHIVING] some_dir/file_deep_5
[ARCHIVING] src/main.rs
",
        )
        .run();

    assert!(repo.root().join("target/package/foo-0.0.1.crate").is_file());

    cargo_process("package -l")
        .cwd(repo.root())
        .with_stdout(
            "\
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
file_root_3
file_root_4
file_root_5
some_dir/dir_deep_2/some_dir/file
some_dir/dir_deep_4/some_dir/file
some_dir/dir_deep_5/some_dir/file
some_dir/file_deep_2
some_dir/file_deep_3
some_dir/file_deep_4
some_dir/file_deep_5
src/main.rs
",
        )
        .run();
}

#[cargo_test]
fn include() {
    let root = paths::root().join("include");
    let repo = git::repo(&root)
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            exclude = ["*.txt"]
            include = ["foo.txt", "**/*.rs", "Cargo.toml", ".dotfile"]
        "#,
        )
        .file("foo.txt", "")
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file(".dotfile", "")
        // Should be ignored when packaging.
        .file("src/bar.txt", "")
        .build();

    cargo_process("package --no-verify -v")
        .cwd(repo.root())
        .with_stderr(
            "\
[WARNING] manifest has no description[..]
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[WARNING] both package.include and package.exclude are specified; the exclude list will be ignored
[PACKAGING] foo v0.0.1 ([..])
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] .dotfile
[ARCHIVING] Cargo.lock
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] foo.txt
[ARCHIVING] src/main.rs
",
        )
        .run();
}

#[cargo_test]
fn package_lib_with_bin() {
    let p = project()
        .file("src/main.rs", "extern crate foo; fn main() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("package -v").run();
}

#[cargo_test]
fn package_git_submodule() {
    let project = git::new("foo", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [project]
                    name = "foo"
                    version = "0.0.1"
                    authors = ["foo@example.com"]
                    license = "MIT"
                    description = "foo"
                    repository = "foo"
                "#,
            )
            .file("src/lib.rs", "pub fn foo() {}")
    });
    let library = git::new("bar", |library| {
        library.no_manifest().file("Makefile", "all:")
    });

    let repository = git2::Repository::open(&project.root()).unwrap();
    let url = path2url(library.root()).to_string();
    git::add_submodule(&repository, &url, Path::new("bar"));
    git::commit(&repository);

    let repository = git2::Repository::open(&project.root().join("bar")).unwrap();
    repository
        .reset(
            &repository.revparse_single("HEAD").unwrap(),
            git2::ResetType::Hard,
            None,
        )
        .unwrap();

    project
        .cargo("package --no-verify -v")
        .with_stderr_contains("[ARCHIVING] bar/Makefile")
        .run();
}

#[cargo_test]
/// Tests if a symlink to a git submodule is properly handled.
///
/// This test requires you to be able to make symlinks.
/// For windows, this may require you to enable developer mode.
fn package_symlink_to_submodule() {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir as symlink;

    if !symlink_supported() {
        return;
    }

    let project = git::new("foo", |project| {
        project.file("src/lib.rs", "pub fn foo() {}")
    });

    let library = git::new("submodule", |library| {
        library.no_manifest().file("Makefile", "all:")
    });

    let repository = git2::Repository::open(&project.root()).unwrap();
    let url = path2url(library.root()).to_string();
    git::add_submodule(&repository, &url, Path::new("submodule"));
    t!(symlink(
        &project.root().join("submodule"),
        &project.root().join("submodule-link")
    ));
    git::add(&repository);
    git::commit(&repository);

    let repository = git2::Repository::open(&project.root().join("submodule")).unwrap();
    repository
        .reset(
            &repository.revparse_single("HEAD").unwrap(),
            git2::ResetType::Hard,
            None,
        )
        .unwrap();

    project
        .cargo("package --no-verify -v")
        .with_stderr_contains("[ARCHIVING] submodule/Makefile")
        .run();
}

#[cargo_test]
fn no_duplicates_from_modified_tracked_files() {
    let root = paths::root().join("all");
    let p = git::repo(&root)
        .file("Cargo.toml", &basic_manifest("foo", "0.0.1"))
        .file("src/main.rs", "fn main() {}")
        .build();
    File::create(p.root().join("src/main.rs"))
        .unwrap()
        .write_all(br#"fn main() { println!("A change!"); }"#)
        .unwrap();
    cargo_process("build").cwd(p.root()).run();
    cargo_process("package --list --allow-dirty")
        .cwd(p.root())
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
        )
        .run();
}

#[cargo_test]
fn ignore_nested() {
    let cargo_toml = r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
        "#;
    let main_rs = r#"
            fn main() { println!("hello"); }
        "#;
    let p = project()
        .file("Cargo.toml", cargo_toml)
        .file("src/main.rs", main_rs)
        // If a project happens to contain a copy of itself, we should
        // ignore it.
        .file("a_dir/foo/Cargo.toml", cargo_toml)
        .file("a_dir/foo/src/main.rs", main_rs)
        .build();

    p.cargo("package")
        .with_stderr(
            "\
[WARNING] manifest has no documentation[..]
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
        )
        .run();
    p.cargo("package").with_stdout("").run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        &[],
    );
}

// Windows doesn't allow these characters in filenames.
#[cfg(unix)]
#[cargo_test]
fn package_weird_characters() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file("src/:foo", "")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr(
            "\
warning: [..]
See [..]
[ERROR] cannot package a filename with a special character `:`: src/:foo
",
        )
        .run();
}

#[cargo_test]
fn repackage_on_source_change() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("package").run();

    // Add another source file
    let mut file = File::create(p.root().join("src").join("foo.rs")).unwrap_or_else(|e| {
        panic!(
            "could not create file {}: {}",
            p.root().join("src/foo.rs").display(),
            e
        )
    });

    file.write_all(br#"fn main() { println!("foo"); }"#)
        .unwrap();
    std::mem::drop(file);

    // Check that cargo rebuilds the tarball
    p.cargo("package")
        .with_stderr(
            "\
[WARNING] [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // Check that the tarball contains the added file
    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "src/foo.rs",
        ],
        &[],
    );
}

#[cargo_test]
/// Tests if a broken symlink is properly handled when packaging.
///
/// This test requires you to be able to make symlinks.
/// For windows, this may require you to enable developer mode.
fn broken_symlink() {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir as symlink;

    if !symlink_supported() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = 'foo'
            documentation = 'foo'
            homepage = 'foo'
            repository = 'foo'
        "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();
    t!(symlink("nowhere", &p.root().join("src/foo.rs")));

    p.cargo("package -v")
        .with_status(101)
        .with_stderr_contains(
            "\
error: failed to prepare local package for uploading

Caused by:
  failed to open for archiving: `[..]foo.rs`

Caused by:
  [..]
",
        )
        .run();
}

#[cargo_test]
/// Tests if a symlink to a directory is properly included.
///
/// This test requires you to be able to make symlinks.
/// For windows, this may require you to enable developer mode.
fn package_symlink_to_dir() {
    if !symlink_supported() {
        return;
    }

    project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file("bla/Makefile", "all:")
        .symlink_dir("bla", "foo")
        .build()
        .cargo("package -v")
        .with_stderr_contains("[ARCHIVING] foo/Makefile")
        .run();
}

#[cargo_test]
fn do_not_package_if_repository_is_dirty() {
    let p = project().build();

    // Create a Git repository containing a minimal Rust project.
    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            license = "MIT"
            description = "foo"
            documentation = "foo"
            homepage = "foo"
            repository = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Modify Cargo.toml without committing the change.
    p.change_file(
        "Cargo.toml",
        r#"
            [project]
            name = "foo"
            version = "0.0.1"
            license = "MIT"
            description = "foo"
            documentation = "foo"
            homepage = "foo"
            repository = "foo"
            # change
    "#,
    );

    p.cargo("package")
        .with_status(101)
        .with_stderr(
            "\
error: 1 files in the working directory contain changes that were not yet \
committed into git:

Cargo.toml

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag
",
        )
        .run();
}

#[cargo_test]
fn generated_manifest() {
    Package::new("abc", "1.0.0").publish();
    Package::new("def", "1.0.0").alternative(true).publish();
    Package::new("ghi", "1.0.0").publish();
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            exclude = ["*.txt"]
            license = "MIT"
            description = "foo"

            [project.metadata]
            foo = 'bar'

            [workspace]

            [dependencies]
            bar = { path = "bar", version = "0.1" }
            def = { version = "1.0", registry = "alternative" }
            ghi = "1.0"
            abc = "1.0"
        "#,
        )
        .file("src/main.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("package --no-verify").run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let rewritten_toml = format!(
        r#"# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies
#
# If you believe there's an error in this file please file an
# issue against the rust-lang/cargo repository. If you're
# editing this file be aware that the upstream Cargo.toml
# will likely look very different (and much more reasonable)

[package]
name = "foo"
version = "0.0.1"
authors = []
exclude = ["*.txt"]
description = "foo"
license = "MIT"

[package.metadata]
foo = "bar"
[dependencies.abc]
version = "1.0"

[dependencies.bar]
version = "0.1"

[dependencies.def]
version = "1.0"
registry-index = "{}"

[dependencies.ghi]
version = "1.0"
"#,
        registry::alt_registry_url()
    );

    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        &[("Cargo.toml", &rewritten_toml)],
    );
}

#[cargo_test]
fn ignore_workspace_specifier() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"

            authors = []

            [workspace]

            [dependencies]
            bar = { path = "bar", version = "0.1" }
        "#,
        )
        .file("src/main.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
            workspace = ".."
        "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("package --no-verify").cwd("bar").run();

    let f = File::open(&p.root().join("target/package/bar-0.1.0.crate")).unwrap();
    let rewritten_toml = r#"# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies
#
# If you believe there's an error in this file please file an
# issue against the rust-lang/cargo repository. If you're
# editing this file be aware that the upstream Cargo.toml
# will likely look very different (and much more reasonable)

[package]
name = "bar"
version = "0.1.0"
authors = []
"#;
    validate_crate_contents(
        f,
        "bar-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[("Cargo.toml", rewritten_toml)],
    );
}

#[cargo_test]
fn package_two_kinds_of_deps() {
    Package::new("other", "1.0.0").publish();
    Package::new("other1", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            other = "1.0"
            other1 = { version = "1.0" }
        "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("package --no-verify").run();
}

#[cargo_test]
fn test_edition() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["edition"]
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            edition = "2018"
        "#,
        )
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("build -v")
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..]--edition=2018 [..]
",
        )
        .run();
}

#[cargo_test]
fn edition_with_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2018"

                [package.metadata.docs.rs]
                features = ["foobar"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package").run();
}

#[cargo_test]
fn test_edition_malformed() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "chicken"
            "#,
        )
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  failed to parse the `edition` key

Caused by:
  supported edition values are `2015` or `2018`, but `chicken` is unknown
"
            .to_string(),
        )
        .run();
}

#[cargo_test]
fn do_not_package_if_src_was_modified() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file("dir/foo.txt", "")
        .file("bar.txt", "")
        .file(
            "build.rs",
            r#"
            use std::fs;

            fn main() {
                fs::write("src/generated.txt",
                    "Hello, world of generated files."
                ).expect("failed to create file");
                fs::remove_file("dir/foo.txt").expect("failed to remove file");
                fs::remove_dir("dir").expect("failed to remove dir");
                fs::write("bar.txt", "updated content").expect("failed to update");
                fs::create_dir("new-dir").expect("failed to create dir");
            }
        "#,
        )
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr_contains(
            "\
error: failed to verify package tarball

Caused by:
  Source directory was modified by build.rs during cargo publish. \
Build scripts should not modify anything outside of OUT_DIR.
Changed: [CWD]/target/package/foo-0.0.1/bar.txt
Added: [CWD]/target/package/foo-0.0.1/new-dir
<tab>[CWD]/target/package/foo-0.0.1/src/generated.txt
Removed: [CWD]/target/package/foo-0.0.1/dir
<tab>[CWD]/target/package/foo-0.0.1/dir/foo.txt

To proceed despite this, pass the `--no-verify` flag.",
        )
        .run();

    p.cargo("package --no-verify").run();
}

#[cargo_test]
fn package_with_select_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"

            [features]
            required = []
            optional = []
        "#,
        )
        .file(
            "src/main.rs",
            "#[cfg(not(feature = \"required\"))]
             compile_error!(\"This crate requires `required` feature!\");
             fn main() {}",
        )
        .build();

    p.cargo("package --features required").run();
}

#[cargo_test]
fn package_with_all_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"

            [features]
            required = []
            optional = []
        "#,
        )
        .file(
            "src/main.rs",
            "#[cfg(not(feature = \"required\"))]
             compile_error!(\"This crate requires `required` feature!\");
             fn main() {}",
        )
        .build();

    p.cargo("package --all-features").run();
}

#[cargo_test]
fn package_no_default_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"

            [features]
            default = ["required"]
            required = []
        "#,
        )
        .file(
            "src/main.rs",
            "#[cfg(not(feature = \"required\"))]
             compile_error!(\"This crate requires `required` feature!\");
             fn main() {}",
        )
        .build();

    p.cargo("package --no-default-features")
        .with_stderr_contains("error: This crate requires `required` feature!")
        .with_status(101)
        .run();
}

#[cargo_test]
fn include_cargo_toml_implicit() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            include = ["src/lib.rs"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --list")
        .with_stdout("Cargo.toml\nCargo.toml.orig\nsrc/lib.rs\n")
        .run();
}

fn include_exclude_test(include: &str, exclude: &str, files: &[&str], expected: &str) {
    let mut pb = project().file(
        "Cargo.toml",
        &format!(
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            license = "MIT"
            description = "foo"
            documentation = "foo"
            homepage = "foo"
            repository = "foo"
            include = {}
            exclude = {}
            "#,
            include, exclude
        ),
    );
    for file in files {
        pb = pb.file(file, "");
    }
    let p = pb.build();

    p.cargo("package --list")
        .with_stderr("")
        .with_stdout(expected)
        .run();
    p.root().rm_rf();
}

#[cargo_test]
fn package_include_ignore_only() {
    // Test with a gitignore pattern that fails to parse with glob.
    // This is a somewhat nonsense pattern, but is an example of something git
    // allows and glob does not.
    assert!(glob::Pattern::new("src/abc**").is_err());

    include_exclude_test(
        r#"["Cargo.toml", "src/abc**", "src/lib.rs"]"#,
        "[]",
        &["src/lib.rs", "src/abc1.rs", "src/abc2.rs", "src/abc/mod.rs"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         src/abc/mod.rs\n\
         src/abc1.rs\n\
         src/abc2.rs\n\
         src/lib.rs\n\
         ",
    )
}

#[cargo_test]
fn gitignore_patterns() {
    include_exclude_test(
        r#"["Cargo.toml", "foo"]"#, // include
        "[]",
        &["src/lib.rs", "foo", "a/foo", "a/b/foo", "x/foo/y", "bar"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         a/b/foo\n\
         a/foo\n\
         foo\n\
         x/foo/y\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "/foo"]"#, // include
        "[]",
        &["src/lib.rs", "foo", "a/foo", "a/b/foo", "x/foo/y", "bar"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         foo\n\
         ",
    );

    include_exclude_test(
        "[]",
        r#"["foo/"]"#, // exclude
        &["src/lib.rs", "foo", "a/foo", "x/foo/y", "bar"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         a/foo\n\
         bar\n\
         foo\n\
         src/lib.rs\n\
         ",
    );

    include_exclude_test(
        "[]",
        r#"["*.txt", "[ab]", "[x-z]"]"#, // exclude
        &[
            "src/lib.rs",
            "foo.txt",
            "bar/foo.txt",
            "other",
            "a",
            "b",
            "c",
            "x",
            "y",
            "z",
        ],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         c\n\
         other\n\
         src/lib.rs\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "**/foo/bar"]"#, // include
        "[]",
        &["src/lib.rs", "a/foo/bar", "foo", "bar"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         a/foo/bar\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "foo/**"]"#, // include
        "[]",
        &["src/lib.rs", "a/foo/bar", "foo/x/y/z"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         foo/x/y/z\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "a/**/b"]"#, // include
        "[]",
        &["src/lib.rs", "a/b", "a/x/b", "a/x/y/b"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         a/b\n\
         a/x/b\n\
         a/x/y/b\n\
         ",
    );
}

#[cargo_test]
fn gitignore_negate() {
    include_exclude_test(
        r#"["Cargo.toml", "*.rs", "!foo.rs", "\\!important"]"#, // include
        "[]",
        &["src/lib.rs", "foo.rs", "!important"],
        "!important\n\
         Cargo.toml\n\
         Cargo.toml.orig\n\
         src/lib.rs\n\
         ",
    );

    // NOTE: This is unusual compared to git. Git treats `src/` as a
    // short-circuit which means rules like `!src/foo.rs` would never run.
    // However, because Cargo only works by iterating over *files*, it doesn't
    // short-circuit.
    include_exclude_test(
        r#"["Cargo.toml", "src/", "!src/foo.rs"]"#, // include
        "[]",
        &["src/lib.rs", "src/foo.rs"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         src/lib.rs\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "src/*.rs", "!foo.rs"]"#, // include
        "[]",
        &["src/lib.rs", "foo.rs", "src/foo.rs", "src/bar/foo.rs"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         src/lib.rs\n\
         ",
    );

    include_exclude_test(
        "[]",
        r#"["*.rs", "!foo.rs", "\\!important"]"#, // exclude
        &["src/lib.rs", "foo.rs", "!important"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         foo.rs\n\
         ",
    );
}

#[cargo_test]
fn exclude_dot_files_and_directories_by_default() {
    include_exclude_test(
        "[]",
        "[]",
        &["src/lib.rs", ".dotfile", ".dotdir/file"],
        "Cargo.toml\n\
         Cargo.toml.orig\n\
         src/lib.rs\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "src/lib.rs", ".dotfile", ".dotdir/file"]"#,
        "[]",
        &["src/lib.rs", ".dotfile", ".dotdir/file"],
        ".dotdir/file\n\
         .dotfile\n\
         Cargo.toml\n\
         Cargo.toml.orig\n\
         src/lib.rs\n\
         ",
    );
}

#[cargo_test]
fn invalid_license_file_path() {
    // Test warning when license-file points to a non-existent file.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            license-file = "does-not-exist"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_stderr(
            "\
[WARNING] license-file `does-not-exist` does not appear to exist (relative to `[..]/foo`).
Please update the license-file setting in the manifest at `[..]/foo/Cargo.toml`
This may become a hard error in the future.
[PACKAGING] foo v1.0.0 ([..]/foo)
",
        )
        .run();
}

#[cargo_test]
fn license_file_implicit_include() {
    // license-file should be automatically included even if not listed.
    let p = git::new("foo", |p| {
        p.file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            license-file = "subdir/LICENSE"
            description = "foo"
            homepage = "foo"
            include = ["src"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("subdir/LICENSE", "license text")
    });

    p.cargo("package --list")
        .with_stdout(
            "\
.cargo_vcs_info.json
Cargo.toml
Cargo.toml.orig
src/lib.rs
subdir/LICENSE
",
        )
        .with_stderr("")
        .run();

    p.cargo("package --no-verify -v")
        .with_stderr(
            "\
[PACKAGING] foo v1.0.0 [..]
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] src/lib.rs
[ARCHIVING] subdir/LICENSE
",
        )
        .run();
    let f = File::open(&p.root().join("target/package/foo-1.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-1.0.0.crate",
        &[
            ".cargo_vcs_info.json",
            "Cargo.toml",
            "Cargo.toml.orig",
            "subdir/LICENSE",
            "src/lib.rs",
        ],
        &[("subdir/LICENSE", "license text")],
    );
}

#[cargo_test]
fn relative_license_included() {
    // license-file path outside of package will copy into root.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            license-file = "../LICENSE"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .file("../LICENSE", "license text")
        .build();

    p.cargo("package --list")
        .with_stdout(
            "\
Cargo.toml
Cargo.toml.orig
LICENSE
src/lib.rs
",
        )
        .with_stderr("")
        .run();

    p.cargo("package")
        .with_stderr(
            "\
[PACKAGING] foo v1.0.0 [..]
[VERIFYING] foo v1.0.0 [..]
[COMPILING] foo v1.0.0 [..]
[FINISHED] [..]
",
        )
        .run();
    let f = File::open(&p.root().join("target/package/foo-1.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-1.0.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "LICENSE", "src/lib.rs"],
        &[("LICENSE", "license text")],
    );
    let manifest =
        std::fs::read_to_string(p.root().join("target/package/foo-1.0.0/Cargo.toml")).unwrap();
    assert!(manifest.contains("license-file = \"LICENSE\""));
    let orig =
        std::fs::read_to_string(p.root().join("target/package/foo-1.0.0/Cargo.toml.orig")).unwrap();
    assert!(orig.contains("license-file = \"../LICENSE\""));
}

#[cargo_test]
fn relative_license_include_collision() {
    // Can't copy a relative license-file if there is a file with that name already.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            license-file = "../LICENSE"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .file("../LICENSE", "outer license")
        .file("LICENSE", "inner license")
        .build();

    p.cargo("package --list")
        .with_stdout(
            "\
Cargo.toml
Cargo.toml.orig
LICENSE
src/lib.rs
",
        )
        .with_stderr("[WARNING] license-file `../LICENSE` appears to be [..]")
        .run();

    p.cargo("package")
        .with_stderr(
            "\
[WARNING] license-file `../LICENSE` appears to be [..]
[PACKAGING] foo v1.0.0 [..]
[VERIFYING] foo v1.0.0 [..]
[COMPILING] foo v1.0.0 [..]
[FINISHED] [..]
",
        )
        .run();
    let f = File::open(&p.root().join("target/package/foo-1.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-1.0.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "LICENSE", "src/lib.rs"],
        &[("LICENSE", "inner license")],
    );
    let manifest = read_to_string(p.root().join("target/package/foo-1.0.0/Cargo.toml")).unwrap();
    assert!(manifest.contains("license-file = \"LICENSE\""));
    let orig = read_to_string(p.root().join("target/package/foo-1.0.0/Cargo.toml.orig")).unwrap();
    assert!(orig.contains("license-file = \"../LICENSE\""));
}
