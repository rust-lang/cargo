//! Tests for the `cargo package` command.

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::publish::validate_crate_contents;
use cargo_test_support::registry::{self, Package};
use cargo_test_support::{
    basic_manifest, cargo_process, git, path2url, paths, project, symlink_supported, t,
};
use flate2::read::GzDecoder;
use std::fs::{self, read_to_string, File};
use std::path::Path;
use tar::Archive;

#[cargo_test]
fn simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
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
[PACKAGED] 4 files, [..] ([..] compressed)
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
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
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
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
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
[PACKAGED] [..] files, [..] ([..] compressed)
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
        .file("a/a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/a/src/lib.rs", "")
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
[PACKAGED] 5 files, [..] ([..] compressed)
",
        )
        .run();

    let f = File::open(&repo.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let vcs_contents = format!(
        r#"{{
  "git": {{
    "sha1": "{}"
  }},
  "path_in_vcs": ""
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
        .cwd(repo.root().join("a/a"))
        .with_stderr(
            "\
[WARNING] manifest has no description[..]
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] a v0.0.1 ([..])
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] src/lib.rs
[PACKAGED] 4 files, [..] ([..] compressed)
",
        )
        .run();

    let f = File::open(&repo.root().join("a/a/target/package/a-0.0.1.crate")).unwrap();
    let vcs_contents = format!(
        r#"{{
  "git": {{
    "sha1": "{}"
  }},
  "path_in_vcs": "a/a"
}}
"#,
        repo.revparse_head()
    );
    validate_crate_contents(
        f,
        "a-0.0.1.crate",
        &[
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            ".cargo_vcs_info.json",
        ],
        &[(".cargo_vcs_info.json", &vcs_contents)],
    );
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
[PACKAGED] [..] files, [..] ([..] compressed)
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
                [package]
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
fn orig_file_collision() {
    let p = project().build();
    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [package]
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
        .file("Cargo.toml.orig", "oops")
        .build();
    p.cargo("package")
        .arg("--no-verify")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid inclusion of reserved file name Cargo.toml.orig \
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
                [package]
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
[ERROR] all dependencies must have a version specified when packaging.
dependency `bar` does not specify a version\n\
Note: The packaged dependency will use the version from crates.io,
the `path` specification will be removed from the dependency declaration.
",
        )
        .run();
}

#[cargo_test]
fn git_dependency_no_version() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"

                [dependencies.foo]
                git = "git://path/to/nowhere"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] manifest has no documentation, homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[ERROR] all dependencies must have a version specified when packaging.
dependency `foo` does not specify a version
Note: The packaged dependency will use the version from crates.io,
the `git` specification will be removed from the dependency declaration.
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
                [package]
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
[PACKAGED] 15 files, [..] ([..] compressed)
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
                [package]
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
[PACKAGED] 7 files, [..] ([..] compressed)
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
                    [package]
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
    let p = git::new("all", |p| p.file("src/main.rs", "fn main() {}"));
    p.change_file("src/main.rs", r#"fn main() { println!("A change!"); }"#);
    p.cargo("build").run();
    p.cargo("package --list --allow-dirty")
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
            [package]
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
[PACKAGED] 4 files, [..] ([..] compressed)
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
    p.change_file("src/foo.rs", r#"fn main() { println!("foo"); }"#);

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
[PACKAGED] 5 files, [..] ([..] compressed)
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
                [package]
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
[ERROR] failed to prepare local package for uploading

Caused by:
  failed to open for archiving: `[..]foo.rs`

Caused by:
  [..]
",
        )
        .run();
}

#[cargo_test]
/// Tests if a broken but excluded symlink is ignored.
/// See issue rust-lang/cargo#10917
///
/// This test requires you to be able to make symlinks.
/// For windows, this may require you to enable developer mode.
fn broken_but_excluded_symlink() {
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
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = 'foo'
                documentation = 'foo'
                homepage = 'foo'
                repository = 'foo'
                exclude = ["src/foo.rs"]
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();
    t!(symlink("nowhere", &p.root().join("src/foo.rs")));

    p.cargo("package -v --list")
        // `src/foo.rs` is excluded.
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
#[cfg(not(windows))] // https://github.com/libgit2/libgit2/issues/6250
/// Test that /dir and /dir/ matches symlinks to directories.
fn gitignore_symlink_dir() {
    if !symlink_supported() {
        return;
    }

    let (p, _repo) = git::new_repo("foo", |p| {
        p.file("src/main.rs", r#"fn main() { println!("hello"); }"#)
            .symlink_dir("src", "src1")
            .symlink_dir("src", "src2")
            .symlink_dir("src", "src3")
            .symlink_dir("src", "src4")
            .file(".gitignore", "/src1\n/src2/\nsrc3\nsrc4/")
    });

    p.cargo("package -l --no-metadata")
        .with_stderr("")
        .with_stdout(
            "\
.cargo_vcs_info.json
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
        )
        .run();
}

#[cargo_test]
#[cfg(not(windows))] // https://github.com/libgit2/libgit2/issues/6250
/// Test that /dir and /dir/ matches symlinks to directories in dirty working directory.
fn gitignore_symlink_dir_dirty() {
    if !symlink_supported() {
        return;
    }

    let (p, _repo) = git::new_repo("foo", |p| {
        p.file("src/main.rs", r#"fn main() { println!("hello"); }"#)
            .file(".gitignore", "/src1\n/src2/\nsrc3\nsrc4/")
    });

    p.symlink("src", "src1");
    p.symlink("src", "src2");
    p.symlink("src", "src3");
    p.symlink("src", "src4");

    p.cargo("package -l --no-metadata")
        .with_stderr("")
        .with_stdout(
            "\
.cargo_vcs_info.json
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
        )
        .run();

    p.cargo("package -l --no-metadata --allow-dirty")
        .with_stderr("")
        .with_stdout(
            "\
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
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
/// Tests if a symlink to ancestor causes filesystem loop error.
///
/// This test requires you to be able to make symlinks.
/// For windows, this may require you to enable developer mode.
fn filesystem_loop() {
    if !symlink_supported() {
        return;
    }

    project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .symlink_dir("a/b", "a/b/c/d/foo")
        .build()
        .cargo("package -v")
        .with_stderr_contains(
            "[WARNING] File system loop found: [..]/a/b/c/d/foo points to an ancestor [..]/a/b",
        )
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
                [package]
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
            [package]
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
fn dirty_ignored() {
    // Cargo warns about an ignored file that will be published.
    let (p, repo) = git::new_repo("foo", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                description = "foo"
                license = "foo"
                documentation = "foo"
                include = ["src", "build"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(".gitignore", "build")
    });
    // Example of adding a file that is confusingly ignored by an overzealous
    // gitignore rule.
    p.change_file("src/build/mod.rs", "");
    p.cargo("package --list")
        .with_status(101)
        .with_stderr(
            "\
error: 1 files in the working directory contain changes that were not yet committed into git:

src/build/mod.rs

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag
",
        )
        .run();
    // Add the ignored file and make sure it is included.
    let mut index = t!(repo.index());
    t!(index.add_path(Path::new("src/build/mod.rs")));
    t!(index.write());
    git::commit(&repo);
    p.cargo("package --list")
        .with_stderr("")
        .with_stdout(
            "\
.cargo_vcs_info.json
Cargo.toml
Cargo.toml.orig
src/build/mod.rs
src/lib.rs
",
        )
        .run();
}

#[cargo_test]
fn generated_manifest() {
    let registry = registry::alt_init();
    Package::new("abc", "1.0.0").publish();
    Package::new("def", "1.0.0").alternative(true).publish();
    Package::new("ghi", "1.0.0").publish();
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                exclude = ["*.txt"]
                license = "MIT"
                description = "foo"

                [package.metadata]
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
        r#"{}
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
        cargo::core::package::MANIFEST_PREAMBLE,
        registry.index_url()
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
                [package]
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
    let rewritten_toml = format!(
        r#"{}
[package]
name = "bar"
version = "0.1.0"
authors = []
"#,
        cargo::core::package::MANIFEST_PREAMBLE
    );
    validate_crate_contents(
        f,
        "bar-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[("Cargo.toml", &rewritten_toml)],
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
                [package]
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

    p.cargo("check -v")
        .with_stderr_contains(
            "\
[CHECKING] foo v0.0.1 ([..])
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

    p.cargo("check -v")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  failed to parse the `edition` key

Caused by:
  supported edition values are `2015`, `2018`, `2021`, or `2024`, but `chicken` is unknown
"
            .to_string(),
        )
        .run();
}

#[cargo_test]
fn test_edition_from_the_future() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"[package]
                edition = "2038"
                name = "foo"
                version = "99.99.99"
                authors = []
            "#,
        )
        .file("src/main.rs", r#""#)
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  failed to parse the `edition` key

Caused by:
  this version of Cargo is older than the `2038` edition, and only supports `2015`, `2018`, `2021`, and `2024` editions.
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
                [package]
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
                [package]
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
                [package]
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
fn empty_readme_path() {
    // Warn but don't fail if `readme` is empty.
    // Issue #11522.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            readme = ""
            license = "MIT"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_stderr(
            "\
[WARNING] readme `` does not appear to exist (relative to `[..]/foo`).
Please update the readme setting in the manifest at `[..]/foo/Cargo.toml`
This may become a hard error in the future.
[PACKAGING] foo v1.0.0 ([..]/foo)
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();
}

#[cargo_test]
fn invalid_readme_path() {
    // Warn but don't fail if `readme` path is invalid.
    // Issue #11522.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            readme = "DOES-NOT-EXIST"
            license = "MIT"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_stderr(
            "\
[WARNING] readme `DOES-NOT-EXIST` does not appear to exist (relative to `[..]/foo`).
Please update the readme setting in the manifest at `[..]/foo/Cargo.toml`
This may become a hard error in the future.
[PACKAGING] foo v1.0.0 ([..]/foo)
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();
}

#[cargo_test]
fn readme_or_license_file_is_dir() {
    // Test warning when `readme` or `license-file` is a directory, not a file.
    // Issue #11522.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            readme = "./src"
            license-file = "./src"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_stderr(
            "\
[WARNING] license-file `./src` does not appear to exist (relative to `[..]/foo`).
Please update the license-file setting in the manifest at `[..]/foo/Cargo.toml`
This may become a hard error in the future.
[WARNING] readme `./src` does not appear to exist (relative to `[..]/foo`).
Please update the readme setting in the manifest at `[..]/foo/Cargo.toml`
This may become a hard error in the future.
[PACKAGING] foo v1.0.0 ([..]/foo)
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();
}

#[cargo_test]
fn empty_license_file_path() {
    // Warn but don't fail if license-file is empty.
    // Issue #11522.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            license-file = ""
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_stderr(
            "\
[WARNING] manifest has no license or license-file.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[WARNING] license-file `` does not appear to exist (relative to `[..]/foo`).
Please update the license-file setting in the manifest at `[..]/foo/Cargo.toml`
This may become a hard error in the future.
[PACKAGING] foo v1.0.0 ([..]/foo)
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();
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
[PACKAGED] [..] files, [..] ([..] compressed)
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
[PACKAGED] 5 files, [..] ([..] compressed)
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
[PACKAGED] 4 files, [..] ([..] compressed)
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
[PACKAGED] 4 files, [..] ([..] compressed)
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

#[cargo_test]
#[cfg(not(windows))] // Don't want to create invalid files on Windows.
fn package_restricted_windows() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            license = "MIT"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "pub mod con;\npub mod aux;")
        .file("src/con.rs", "pub fn f() {}")
        .file("src/aux/mod.rs", "pub fn f() {}")
        .build();

    p.cargo("package")
        // use unordered here because the order of the warning is different on each platform.
        .with_stderr_unordered(
            "\
[WARNING] file src/aux/mod.rs is a reserved Windows filename, it will not work on Windows platforms
[WARNING] file src/con.rs is a reserved Windows filename, it will not work on Windows platforms
[PACKAGING] foo [..]
[VERIFYING] foo [..]
[COMPILING] foo [..]
[PACKAGED] [..] files, [..] ([..] compressed)
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn finds_git_in_parent() {
    // Test where `Cargo.toml` is not in the root of the git repo.
    let repo_path = paths::root().join("repo");
    fs::create_dir(&repo_path).unwrap();
    let p = project()
        .at("repo/foo")
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "")
        .build();
    let repo = git::init(&repo_path);
    git::add(&repo);
    git::commit(&repo);
    p.change_file("ignoreme", "");
    p.change_file("ignoreme2", "");
    p.cargo("package --list --allow-dirty")
        .with_stdout(
            "\
Cargo.toml
Cargo.toml.orig
ignoreme
ignoreme2
src/lib.rs
",
        )
        .run();

    p.change_file(".gitignore", "ignoreme");
    p.cargo("package --list --allow-dirty")
        .with_stdout(
            "\
.gitignore
Cargo.toml
Cargo.toml.orig
ignoreme2
src/lib.rs
",
        )
        .run();

    fs::write(repo_path.join(".gitignore"), "ignoreme2").unwrap();
    p.cargo("package --list --allow-dirty")
        .with_stdout(
            "\
.gitignore
Cargo.toml
Cargo.toml.orig
src/lib.rs
",
        )
        .run();
}

#[cargo_test]
#[cfg(windows)]
fn reserved_windows_name() {
    // If we are running on a version of Windows that allows these reserved filenames,
    // skip this test.
    if paths::windows_reserved_names_are_allowed() {
        return;
    }

    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "pub mod aux;")
        .file("src/aux.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"

                [dependencies]
                bar = "1.0.0"
            "#,
        )
        .file("src/main.rs", "extern crate bar;\nfn main() {  }")
        .build();
    p.cargo("package")
        .with_status(101)
        .with_stderr_contains(
            "\
error: failed to verify package tarball

Caused by:
  failed to download replaced source registry `[..]`

Caused by:
  failed to unpack package `[..] `[..]`)`

Caused by:
  failed to unpack entry at `[..]aux.rs`

Caused by:
  `[..]aux.rs` appears to contain a reserved Windows path, it cannot be extracted on Windows

Caused by:
  failed to unpack `[..]aux.rs`

Caused by:
  failed to unpack `[..]aux.rs` into `[..]aux.rs`",
        )
        .run();
}

#[cargo_test]
fn list_with_path_and_lock() {
    // Allow --list even for something that isn't packageable.

    // Init an empty registry because a versionless path dep will search for
    // the package on crates.io.
    registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            license = "MIT"
            description = "foo"
            homepage = "foo"

            [dependencies]
            bar = {path="bar"}
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("package --list")
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
        )
        .run();

    p.cargo("package")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] all dependencies must have a version specified when packaging.
dependency `bar` does not specify a version
Note: The packaged dependency will use the version from crates.io,
the `path` specification will be removed from the dependency declaration.
",
        )
        .run();
}

#[cargo_test]
fn long_file_names() {
    // Filenames over 100 characters require a GNU extension tarfile.
    // See #8453.

    registry::init();
    let long_name = concat!(
        "012345678901234567890123456789012345678901234567890123456789",
        "012345678901234567890123456789012345678901234567890123456789",
        "012345678901234567890123456789012345678901234567890123456789"
    );
    if cfg!(windows) {
        // Long paths on Windows require a special registry entry that is
        // disabled by default (even on Windows 10).
        // https://docs.microsoft.com/en-us/windows/win32/fileio/naming-a-file
        // If the directory where Cargo runs happens to be more than 80 characters
        // long, then it will bump into this limit.
        //
        // First create a directory to account for various paths Cargo will
        // be using in the target directory (such as "target/package/foo-0.1.0").
        let test_path = paths::root().join("test-dir-probe-long-path-support");
        test_path.mkdir_p();
        let test_path = test_path.join(long_name);
        if let Err(e) = File::create(&test_path) {
            // write to stderr directly to avoid output from being captured
            // and always display text, even without --nocapture
            use std::io::Write;
            writeln!(
                std::io::stderr(),
                "\nSkipping long_file_names test, this OS or filesystem does not \
                appear to support long file paths: {:?}\n{:?}",
                e,
                test_path
            )
            .unwrap();
            return;
        }
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            license = "MIT"
            description = "foo"
            homepage = "foo"

            [dependencies]
            "#,
        )
        .file(long_name, "something")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("package").run();
    p.cargo("package --list")
        .with_stdout(&format!(
            "\
{}
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
            long_name
        ))
        .run();
}

#[cargo_test]
fn reproducible_output() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                exclude = ["*.txt"]
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("package").run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let decoder = GzDecoder::new(f);
    let mut archive = Archive::new(decoder);
    for ent in archive.entries().unwrap() {
        let ent = ent.unwrap();
        println!("checking {:?}", ent.path());
        let header = ent.header();
        assert_eq!(header.mode().unwrap(), 0o644);
        assert!(header.mtime().unwrap() != 0);
        assert_eq!(header.username().unwrap().unwrap(), "");
        assert_eq!(header.groupname().unwrap().unwrap(), "");
    }
}

#[cargo_test]
fn package_with_resolver_and_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                resolver = '2'

                [package.metadata.docs.rs]
                all-features = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package").run();
}

#[cargo_test]
fn deleted_git_working_tree() {
    // When deleting a file, but not staged, cargo should ignore the file.
    let (p, repo) = git::new_repo("foo", |p| {
        p.file("src/lib.rs", "").file("src/main.rs", "fn main() {}")
    });
    p.root().join("src/lib.rs").rm_rf();
    p.cargo("package --allow-dirty --list")
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
        )
        .run();
    p.cargo("package --allow-dirty").run();
    let mut index = t!(repo.index());
    t!(index.remove(Path::new("src/lib.rs"), 0));
    t!(index.write());
    p.cargo("package --allow-dirty --list")
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs
",
        )
        .run();
    p.cargo("package --allow-dirty").run();
}

#[cargo_test]
fn in_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "bar"
                workspace = ".."
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("package --workspace")
        .with_stderr(
            "\
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] bar v0.0.1 ([CWD]/bar)
[VERIFYING] bar v0.0.1 ([CWD]/bar)
[COMPILING] bar v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[PACKAGED] [..] files, [..] ([..] compressed)
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();

    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    assert!(p.root().join("target/package/bar-0.0.1.crate").is_file());
}

#[cargo_test]
fn workspace_noconflict_readme() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("README.md", "workspace readme")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                repository = "https://github.com/bar/bar"
                authors = []
                license = "MIT"
                description = "bar"
                readme = "../README.md"
                workspace = ".."
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/example/README.md", "# example readmdBar")
        .build();

    p.cargo("package")
        .with_stderr(
            "\
[PACKAGING] bar v0.0.1 ([CWD]/bar)
[VERIFYING] bar v0.0.1 ([CWD]/bar)
[COMPILING] bar v0.0.1 ([CWD]/[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();
}

#[cargo_test]
fn workspace_conflict_readme() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("README.md", "workspace readme")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                repository = "https://github.com/bar/bar"
                authors = []
                license = "MIT"
                description = "bar"
                readme = "../README.md"
                workspace = ".."
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/README.md", "# workspace member: Bar")
        .build();

    p.cargo("package")
        .with_stderr(
            "\
warning: readme `../README.md` appears to be a path outside of the package, but there is already a file named `README.md` in the root of the package. The archived crate will contain the copy in the root of the package. Update the readme to point to the path relative to the root of the package to remove this warning.
[PACKAGING] bar v0.0.1 ([CWD]/bar)
[VERIFYING] bar v0.0.1 ([CWD]/bar)
[COMPILING] bar v0.0.1 ([CWD]/[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[PACKAGED] [..] files, [..] ([..] compressed)
",
        )
        .run();
}

#[cargo_test]
fn workspace_overrides_resolver() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2021"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                edition = "2015"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("package --no-verify -p bar -p baz").run();

    let f = File::open(&p.root().join("target/package/bar-0.1.0.crate")).unwrap();
    let rewritten_toml = format!(
        r#"{}
[package]
edition = "2021"
name = "bar"
version = "0.1.0"
resolver = "1"
"#,
        cargo::core::package::MANIFEST_PREAMBLE
    );
    validate_crate_contents(
        f,
        "bar-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[("Cargo.toml", &rewritten_toml)],
    );

    // When the crate has the same implicit resolver as the workspace it is not overridden
    let f = File::open(&p.root().join("target/package/baz-0.1.0.crate")).unwrap();
    let rewritten_toml = format!(
        r#"{}
[package]
edition = "2015"
name = "baz"
version = "0.1.0"
"#,
        cargo::core::package::MANIFEST_PREAMBLE
    );
    validate_crate_contents(
        f,
        "baz-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[("Cargo.toml", &rewritten_toml)],
    );
}

fn verify_packaged_status_line(
    output: std::process::Output,
    num_files: usize,
    uncompressed_size: u64,
    compressed_size: u64,
) {
    use cargo::util::human_readable_bytes;

    let stderr = String::from_utf8(output.stderr).unwrap();
    let mut packaged_lines = stderr
        .lines()
        .filter(|line| line.trim().starts_with("Packaged"));
    let packaged_line = packaged_lines
        .next()
        .expect("`Packaged` status line should appear in stderr");
    assert!(
        packaged_lines.next().is_none(),
        "Only one `Packaged` status line should appear in stderr"
    );
    let size_info = packaged_line.trim().trim_start_matches("Packaged").trim();
    let uncompressed = human_readable_bytes(uncompressed_size);
    let compressed = human_readable_bytes(compressed_size);
    let expected = format!(
        "{} files, {:.1}{} ({:.1}{} compressed)",
        num_files, uncompressed.0, uncompressed.1, compressed.0, compressed.1
    );
    assert_eq!(size_info, expected);
}

#[cargo_test]
fn basic_filesizes() {
    let cargo_toml_orig_contents = r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                exclude = ["*.txt"]
                license = "MIT"
                description = "foo"
            "#;
    let main_rs_contents = r#"fn main() { println!("🦀"); }"#;
    let cargo_toml_contents = format!(
        r#"{}
[package]
name = "foo"
version = "0.0.1"
authors = []
exclude = ["*.txt"]
description = "foo"
license = "MIT"
"#,
        cargo::core::package::MANIFEST_PREAMBLE
    );
    let cargo_lock_contents = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "foo"
version = "0.0.1"
"#;
    let p = project()
        .file("Cargo.toml", cargo_toml_orig_contents)
        .file("src/main.rs", main_rs_contents)
        .file("src/bar.txt", "Ignored text file contents") // should be ignored when packaging
        .build();

    let uncompressed_size = (cargo_toml_orig_contents.len()
        + main_rs_contents.len()
        + cargo_toml_contents.len()
        + cargo_lock_contents.len()) as u64;
    let output = p.cargo("package").exec_with_output().unwrap();

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
    let compressed_size = f.metadata().unwrap().len();
    verify_packaged_status_line(output, 4, uncompressed_size, compressed_size);
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        &[
            ("Cargo.lock", cargo_lock_contents),
            ("Cargo.toml", &cargo_toml_contents),
            ("Cargo.toml.orig", cargo_toml_orig_contents),
            ("src/main.rs", main_rs_contents),
        ],
    );
}

#[cargo_test]
fn larger_filesizes() {
    let cargo_toml_orig_contents = r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#;
    let lots_of_crabs = std::iter::repeat("🦀").take(1337).collect::<String>();
    let main_rs_contents = format!(r#"fn main() {{ println!("{}"); }}"#, lots_of_crabs);
    let bar_txt_contents = "This file is relatively uncompressible, to increase the compressed
        package size beyond 1KiB.
        Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt
        ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation
        ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in
        reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur
        sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est
        laborum.";
    let cargo_toml_contents = format!(
        r#"{}
[package]
name = "foo"
version = "0.0.1"
authors = []
description = "foo"
license = "MIT"
"#,
        cargo::core::package::MANIFEST_PREAMBLE
    );
    let cargo_lock_contents = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "foo"
version = "0.0.1"
"#;
    let p = project()
        .file("Cargo.toml", cargo_toml_orig_contents)
        .file("src/main.rs", &main_rs_contents)
        .file("src/bar.txt", bar_txt_contents)
        .build();

    let uncompressed_size = (cargo_toml_orig_contents.len()
        + main_rs_contents.len()
        + cargo_toml_contents.len()
        + cargo_lock_contents.len()
        + bar_txt_contents.len()) as u64;

    let output = p.cargo("package").exec_with_output().unwrap();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/bar.txt
src/main.rs
",
        )
        .run();
    p.cargo("package").with_stdout("").run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let compressed_size = f.metadata().unwrap().len();
    verify_packaged_status_line(output, 5, uncompressed_size, compressed_size);
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/bar.txt",
            "src/main.rs",
        ],
        &[
            ("Cargo.lock", cargo_lock_contents),
            ("Cargo.toml", &cargo_toml_contents),
            ("Cargo.toml.orig", cargo_toml_orig_contents),
            ("src/bar.txt", bar_txt_contents),
            ("src/main.rs", &main_rs_contents),
        ],
    );
}

#[cargo_test]
fn symlink_filesizes() {
    if !symlink_supported() {
        return;
    }

    let cargo_toml_orig_contents = r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#;
    let lots_of_crabs = std::iter::repeat("🦀").take(1337).collect::<String>();
    let main_rs_contents = format!(r#"fn main() {{ println!("{}"); }}"#, lots_of_crabs);
    let bar_txt_contents = "This file is relatively uncompressible, to increase the compressed
        package size beyond 1KiB.
        Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt
        ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation
        ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in
        reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur
        sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est
        laborum.";
    let cargo_toml_contents = format!(
        r#"{}
[package]
name = "foo"
version = "0.0.1"
authors = []
description = "foo"
license = "MIT"
"#,
        cargo::core::package::MANIFEST_PREAMBLE
    );
    let cargo_lock_contents = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "foo"
version = "0.0.1"
"#;

    let p = project()
        .file("Cargo.toml", cargo_toml_orig_contents)
        .file("src/main.rs", &main_rs_contents)
        .file("bla/bar.txt", bar_txt_contents)
        .symlink("src/main.rs", "src/main.rs.bak")
        .symlink_dir("bla", "foo")
        .build();

    let uncompressed_size = (cargo_toml_orig_contents.len()
        + main_rs_contents.len() * 2
        + cargo_toml_contents.len()
        + cargo_lock_contents.len()
        + bar_txt_contents.len() * 2) as u64;

    let output = p.cargo("package").exec_with_output().unwrap();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout(
            "\
Cargo.lock
Cargo.toml
Cargo.toml.orig
bla/bar.txt
foo/bar.txt
src/main.rs
src/main.rs.bak
",
        )
        .run();
    p.cargo("package").with_stdout("").run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let compressed_size = f.metadata().unwrap().len();
    verify_packaged_status_line(output, 7, uncompressed_size, compressed_size);
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "bla/bar.txt",
            "foo/bar.txt",
            "src/main.rs",
            "src/main.rs.bak",
        ],
        &[
            ("Cargo.lock", cargo_lock_contents),
            ("Cargo.toml", &cargo_toml_contents),
            ("Cargo.toml.orig", cargo_toml_orig_contents),
            ("bla/bar.txt", bar_txt_contents),
            ("foo/bar.txt", bar_txt_contents),
            ("src/main.rs", &main_rs_contents),
            ("src/main.rs.bak", &main_rs_contents),
        ],
    );
}

#[cargo_test]
#[cfg(windows)] // windows is the platform that is most consistently configured for case insensitive filesystems
fn normalize_case() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file("src/bar.txt", "") // should be ignored when packaging
        .build();
    // Workaround `project()` making a `Cargo.toml` on our behalf
    std::fs::remove_file(p.root().join("Cargo.toml")).unwrap();
    std::fs::write(
        p.root().join("cargo.toml"),
        r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                exclude = ["*.txt"]
                license = "MIT"
                description = "foo"
            "#,
    )
    .unwrap();

    p.cargo("package")
        .with_stderr(
            "\
[WARNING] manifest has no documentation[..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[PACKAGED] 4 files, [..] ([..] compressed)
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
#[cfg(target_os = "linux")] // linux is generally configured to be case sensitive
fn mixed_case() {
    let manifest = r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                exclude = ["*.txt"]
                license = "MIT"
                description = "foo"
            "#;
    let p = project()
        .file("Cargo.toml", manifest)
        .file("cargo.toml", manifest)
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
[PACKAGED] 4 files, [..] ([..] compressed)
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
