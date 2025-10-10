//! Tests for the `cargo package` command.

use std::fs::{self, File, read_to_string};
use std::path::Path;

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::publish::validate_crate_contents;
use cargo_test_support::registry::{self, Package};
use cargo_test_support::{
    Project, ProjectBuilder, basic_manifest, git, paths, project, rustc_host, str,
    symlink_supported, t,
};
use flate2::read::GzDecoder;
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "Cargo.lock",
        ],
        (),
    );
}

#[cargo_test]
fn metadata_warning() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                repository = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/all)
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] Cargo.lock
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] src/main.rs
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    let f = File::open(&repo.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let vcs_contents = format!(
        r#"{{
  "git": {{
    "sha1": "{}"
  }},
  "path_in_vcs": ""
}}"#,
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
        [(".cargo_vcs_info.json", &vcs_contents)],
    );

    println!("package sub-repo");
    cargo_process("package -v --no-verify")
        .cwd(repo.root().join("a/a"))
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] a v0.0.1 ([ROOT]/all/a/a)
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] Cargo.lock
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] src/lib.rs
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    let f = File::open(&repo.root().join("a/a/target/package/a-0.0.1.crate")).unwrap();
    let vcs_contents = format!(
        r#"{{
  "git": {{
    "sha1": "{}"
  }},
  "path_in_vcs": "a/a"
}}"#,
        repo.revparse_head()
    );
    validate_crate_contents(
        f,
        "a-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            ".cargo_vcs_info.json",
        ],
        [(".cargo_vcs_info.json", &vcs_contents)],
    );
}

#[cargo_test]
fn package_verification() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("build").run();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] invalid inclusion of reserved file name .cargo_vcs_info.json in package source

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] invalid inclusion of reserved file name Cargo.toml.orig in package source

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[ERROR] failed to verify manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  all dependencies must have a version requirement specified when packaging.
  dependency `bar` does not specify a version
  Note: The packaged dependency will use the version from crates.io,
  the `path` specification will be removed from the dependency declaration.

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[ERROR] failed to verify manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  all dependencies must have a version requirement specified when packaging.
  dependency `foo` does not specify a version
  Note: The packaged dependency will use the version from crates.io,
  the `git` specification will be removed from the dependency declaration.

"#]])
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
                edition = "2015"
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
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/exclude)
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
[PACKAGED] 15 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    assert!(repo.root().join("target/package/foo-0.0.1.crate").is_file());

    cargo_process("package -l")
        .cwd(repo.root())
        .with_stdout_data(str![[r#"
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

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[WARNING] both package.include and package.exclude are specified; the exclude list will be ignored
[PACKAGING] foo v0.0.1 ([ROOT]/include)
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] .dotfile
[ARCHIVING] Cargo.lock
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] foo.txt
[ARCHIVING] src/main.rs
[PACKAGED] 7 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
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
                edition = "2015"
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
    let url = library.root().to_url().to_string();
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
        .with_stderr_data(str![[r#"
...
[ARCHIVING] bar/Makefile
...
"#]])
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
    let url = library.root().to_url().to_string();
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
        .with_stderr_does_not_contain("[ARCHIVING] submodule-link/.git/config")
        .run();
}

#[cargo_test]
fn no_duplicates_from_modified_tracked_files() {
    let p = git::new("all", |p| p.file("src/main.rs", "fn main() {}"));
    p.change_file("src/main.rs", r#"fn main() { println!("A change!"); }"#);
    p.cargo("build").run();
    p.cargo("package --list --allow-dirty")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
}

#[cargo_test]
fn ignore_nested() {
    let cargo_toml = r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "foo"
            homepage = "https://example.com/"
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
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        (),
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[ERROR] cannot package a filename with a special character `:`: src/:foo

"#]])
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        (),
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
...
[ERROR] failed to prepare local package for uploading

Caused by:
  failed to open for archiving: `[ROOT]/foo/src/foo.rs`

Caused by:
  [NOT_FOUND]

"#]])
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
                edition = "2015"
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
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
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
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
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
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();

    p.cargo("package -l --no-metadata --allow-dirty")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
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
        .with_stderr_data(str![[r#"
...
[ARCHIVING] foo/Makefile
...
"#]])
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
        .with_stderr_data(str![[r#"
...
[WARNING] File system loop found: [ROOT]/foo/a/b/c/d/foo points to an ancestor [ROOT]/foo/a/b
...
"#]])
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
                edition = "2015"
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
            edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

Cargo.toml

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
        .run();

    // cd to `src` and cargo report relative paths.
    p.cargo("package")
        .cwd(p.root().join("src"))
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

../Cargo.toml

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

src/build/mod.rs

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
        .run();
    // Add the ignored file and make sure it is included.
    let mut index = t!(repo.index());
    t!(index.add_path(Path::new("src/build/mod.rs")));
    t!(index.write());
    git::commit(&repo);
    p.cargo("package --list")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/build/mod.rs
src/lib.rs

"#]])
        .run();
}

#[cargo_test]
fn vcs_status_check_for_each_workspace_member() {
    // Cargo checks VCS status separately for each workspace member.
    // This ensure one file changed in a package won't affect the other.
    // Since the dirty bit in .cargo_vcs_info.json is just for advisory purpose,
    // We may change the meaning of it in the future.
    let (p, repo) = git::new_repo("foo", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["isengard", "mordor"]
            "#,
        )
        .file("hobbit", "...")
        .file(
            "isengard/Cargo.toml",
            r#"
                [package]
                name = "isengard"
                edition = "2015"
                homepage = "saruman"
                description = "saruman"
                license = "MIT"
            "#,
        )
        .file("isengard/src/lib.rs", "")
        .file(
            "mordor/Cargo.toml",
            r#"
                [package]
                name = "mordor"
                edition = "2015"
                homepage = "sauron"
                description = "sauron"
                license = "MIT"
            "#,
        )
        .file("mordor/src/lib.rs", "")
    });
    git::commit(&repo);

    // Dirty file outside won't affect packaging.
    p.change_file("hobbit", "changed!");
    p.change_file("mordor/src/lib.rs", "changed!");
    p.change_file("mordor/src/main.rs", "fn main() {}");

    // Ensure dirty files be reported only for one affected package.
    p.cargo("package --workspace --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[PACKAGING] isengard v0.0.0 ([ROOT]/foo/isengard)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[ERROR] 2 files in the working directory contain changes that were not yet committed into git:

mordor/src/lib.rs
mordor/src/main.rs

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
        .run();

    // Ensure only dirty package be recorded as dirty.
    p.cargo("package --workspace --no-verify --allow-dirty")
        .with_stderr_data(str![[r#"
[PACKAGING] isengard v0.0.0 ([ROOT]/foo/isengard)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] mordor v0.0.0 ([ROOT]/foo/mordor)
[PACKAGED] 6 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/isengard-0.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "isengard-0.0.0.crate",
        &[
            ".cargo_vcs_info.json",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "Cargo.lock",
        ],
        [(
            ".cargo_vcs_info.json",
            // No change within `isengard/`, so not dirty at all.
            str![[r#"
{
  "git": {
    "sha1": "[..]"
  },
  "path_in_vcs": "isengard"
}
"#]]
            .is_json(),
        )],
    );

    let f = File::open(&p.root().join("target/package/mordor-0.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "mordor-0.0.0.crate",
        &[
            ".cargo_vcs_info.json",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "src/main.rs",
            "Cargo.lock",
        ],
        [(
            ".cargo_vcs_info.json",
            // Dirty bit is recorded.
            str![[r#"
{
  "git": {
    "dirty": true,
    "sha1": "[..]"
  },
  "path_in_vcs": "mordor"
}
"#]]
            .is_json(),
        )],
    );
}

#[cargo_test]
fn dirty_file_outside_pkg_root_considered_dirty() {
    if !symlink_supported() {
        return;
    }
    let main_outside_pkg_root = paths::root().join("main.rs");
    let (p, repo) = git::new_repo("foo", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["isengard"]
                resolver = "2"
                [workspace.package]
                edition = "2015"
            "#,
        )
        .file("lib.rs", r#"compile_error!("you shall not pass")"#)
        .file("LICENSE", "before")
        .file("README.md", "before")
        .file(
            "isengard/Cargo.toml",
            r#"
                [package]
                name = "isengard"
                edition.workspace = true
                homepage = "saruman"
                description = "saruman"
                license-file = "../LICENSE"
            "#,
        )
        .file("original-dir/file", "before")
        .symlink("lib.rs", "isengard/src/lib.rs")
        .symlink("README.md", "isengard/README.md")
        .file(&main_outside_pkg_root, "fn main() {}")
        .symlink(&main_outside_pkg_root, "isengard/src/main.rs")
        .symlink_dir("original-dir", "isengard/symlink-dir")
    });
    git::commit(&repo);

    // Changing files outside pkg root under situations below should be treated
    // as dirty. `cargo package` is expected to fail on VCS status check.
    //
    // * Changes in files outside package root that source files symlink to
    p.change_file("README.md", "after");
    p.change_file("lib.rs", "pub fn after() {}");
    p.change_file("original-dir/file", "after");
    // * Changes in files outside pkg root that `license-file`/`readme` point to
    p.change_file("LICENSE", "after");
    // * When workspace root manifest has changed,
    //   no matter whether workspace inheritance is involved.
    p.change_file(
        "Cargo.toml",
        r#"
            [workspace]
            members = ["isengard"]
            resolver = "2"
            [workspace.package]
            edition = "2021"
        "#,
    );
    // Changes in files outside git workdir won't affect VCS status check
    p.change_file(
        &main_outside_pkg_root,
        r#"fn main() { eprintln!("after"); }"#,
    );

    // Ensure dirty files be reported.
    p.cargo("package --workspace --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] 5 files in the working directory contain changes that were not yet committed into git:

Cargo.toml
LICENSE
README.md
lib.rs
original-dir/file

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
        .run();

    p.cargo("package --workspace --no-verify --allow-dirty")
        .with_stderr_data(str![[r#"
[PACKAGING] isengard v0.0.0 ([ROOT]/foo/isengard)
[PACKAGED] 9 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    let cargo_toml = str![[r##"
...
[package]
edition = "2021"
...

"##]];

    let f = File::open(&p.root().join("target/package/isengard-0.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "isengard-0.0.0.crate",
        &[
            ".cargo_vcs_info.json",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "src/main.rs",
            "symlink-dir/file",
            "Cargo.lock",
            "LICENSE",
            "README.md",
        ],
        [
            ("src/lib.rs", str!["pub fn after() {}"]),
            ("src/main.rs", str![r#"fn main() { eprintln!("after"); }"#]),
            ("symlink-dir/file", str!["after"]),
            ("README.md", str!["after"]),
            ("LICENSE", str!["after"]),
            ("Cargo.toml", cargo_toml),
        ],
    );
}

#[cargo_test]
fn dirty_file_outside_pkg_root_inside_submodule() {
    if !symlink_supported() {
        return;
    }
    let (p, repo) = git::new_repo("foo", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["isengard"]
                resolver = "2"
            "#,
        )
        .file(
            "isengard/Cargo.toml",
            r#"
                [package]
                name = "isengard"
                edition = "2015"
                homepage = "saruman"
                description = "saruman"
                license = "ISC"
            "#,
        )
        .file("isengard/src/lib.rs", "")
    });
    let submodule = git::new("submodule", |p| {
        p.no_manifest().file("file.txt", "from-submodule")
    });
    git::add_submodule(
        &repo,
        submodule.root().to_url().as_ref(),
        Path::new("submodule"),
    );
    p.symlink("submodule/file.txt", "isengard/src/file.txt");
    git::add(&repo);
    git::commit(&repo);
    p.change_file("submodule/file.txt", "changed");

    p.cargo("package --workspace --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

isengard/src/file.txt

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
        .run();
}

#[cargo_test]
fn issue_13695_allow_dirty_vcs_info() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            description = "foo"
            license = "foo"
            documentation = "foo"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    let repo = git::init(&p.root());
    // Initial commit, with no files added.
    git::commit(&repo);

    // Allowing a dirty worktree results in the vcs file still being included.
    p.cargo("package --allow-dirty").run();

    let f = File::open(&p.root().join("target/package/foo-0.1.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.1.0.crate",
        &[
            ".cargo_vcs_info.json",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "Cargo.lock",
        ],
        [(
            ".cargo_vcs_info.json",
            str![[r#"
{
  "git": {
    "dirty": true,
    "sha1": "[..]"
  },
  "path_in_vcs": ""
}
"#]]
            .is_json(),
        )],
    );

    // Listing provides a consistent result.
    p.cargo("package --list --allow-dirty")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/lib.rs

"#]])
        .run();
}

#[cargo_test]
fn issue_13695_allowing_dirty_vcs_info_but_clean() {
    let p = project().build();
    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            description = "foo"
            license = "foo"
            documentation = "foo"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Allowing a dirty worktree despite it being clean.
    p.cargo("package --allow-dirty").run();

    let f = File::open(&p.root().join("target/package/foo-0.1.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.1.0.crate",
        &[
            ".cargo_vcs_info.json",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "Cargo.lock",
        ],
        [(
            ".cargo_vcs_info.json",
            str![[r#"
{
  "git": {
    "sha1": "[..]"
  },
  "path_in_vcs": ""
}
"#]]
            .is_json(),
        )],
    );
}

#[cargo_test]
fn issue_14354_allowing_dirty_bare_commit() {
    let p = project().build();
    // Init a bare commit git repo
    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            description = "foo"
            license = "foo"
            documentation = "foo"
        "#,
        )
        .file("src/lib.rs", "");

    p.cargo("package --allow-dirty").run();

    let f = File::open(&p.root().join("target/package/foo-0.1.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        (),
    );
}

#[cargo_test]
fn generated_manifest() {
    registry::alt_init();
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
                edition = "2015"
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
    let rewritten_toml = str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
exclude = ["*.txt"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
readme = false
license = "MIT"

[package.metadata]
foo = "bar"

[[bin]]
name = "foo"
path = "src/main.rs"

[dependencies.abc]
version = "1.0"

[dependencies.bar]
version = "0.1"

[dependencies.def]
version = "1.0"
registry-index = "[ROOTURL]/alternative-registry"

[dependencies.ghi]
version = "1.0"

"##]];

    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        [("Cargo.toml", rewritten_toml)],
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
                edition = "2015"

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
                edition = "2015"
                authors = []
                workspace = ".."
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("package --no-verify").cwd("bar").run();

    let f = File::open(&p.root().join("target/package/bar-0.1.0.crate")).unwrap();
    let rewritten_toml = str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "bar"
version = "0.1.0"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
readme = false

[lib]
name = "bar"
path = "src/lib.rs"

"##]];
    validate_crate_contents(
        f,
        "bar-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        [("Cargo.toml", rewritten_toml)],
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
                edition = "2015"
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
fn package_should_use_build_cache() {
    Package::new("other", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                other = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Build once so that the build cache is populated
    p.cargo("build").run();

    // Run package and verify we do not rebuild the `other` crate
    p.cargo("package")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] `dummy-registry` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn package_public_dep() {
    Package::new("bar", "1.0.0").publish();
    Package::new("baz", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            &format! {
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = {{ version = "1.0.0", public = true }}

                [target.{host}.dependencies]
                baz = {{ version = "1.0.0", public = true }}
            "#,
                host = rustc_host()
            },
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    let rewritten_toml = str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
readme = false

[[bin]]
name = "foo"
path = "src/main.rs"

[dependencies.bar]
version = "1.0.0"

[target.[HOST_TARGET].dependencies.baz]
version = "1.0.0"

"##]];
    verify(&p, "package", rewritten_toml);

    let rewritten_toml = str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
readme = false

[[bin]]
name = "foo"
path = "src/main.rs"

[dependencies.bar]
version = "1.0.0"
public = true

[target.[HOST_TARGET].dependencies.baz]
version = "1.0.0"
public = true

"##]];
    verify(&p, "package -Zpublic-dependency", rewritten_toml);

    fn verify(p: &cargo_test_support::Project, cmd: &str, rewritten_toml: impl IntoData) {
        p.cargo(cmd)
            .masquerade_as_nightly_cargo(&["public-dependency"])
            .run();
        let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
        validate_crate_contents(
            f,
            "foo-0.0.1.crate",
            &["Cargo.toml", "Cargo.toml.orig", "Cargo.lock", "src/main.rs"],
            [("Cargo.toml", rewritten_toml)],
        );
    }
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
        .with_stderr_data(str![[r#"
...
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]--edition=2018 [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  failed to parse the `edition` key

Caused by:
  supported edition values are `2015`, `2018`, `2021`, or `2024`, but `chicken` is unknown

"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  failed to parse the `edition` key

Caused by:
  this version of Cargo is older than the `2038` edition, and only supports `2015`, `2018`, `2021`, and `2024` editions.

"#]])
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
        .with_stderr_data(str![[r#"
...
[ERROR] failed to verify package tarball

Caused by:
  Source directory was modified by build.rs during cargo publish. Build scripts should not modify anything outside of OUT_DIR.
  Changed: [ROOT]/foo/target/package/foo-0.0.1/bar.txt
  Added: [ROOT]/foo/target/package/foo-0.0.1/new-dir
  	[ROOT]/foo/target/package/foo-0.0.1/src/generated.txt
  Removed: [ROOT]/foo/target/package/foo-0.0.1/dir
  	[ROOT]/foo/target/package/foo-0.0.1/dir/foo.txt

  To proceed despite this, pass the `--no-verify` flag.

"#]])
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
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
...
[ERROR] This crate requires `required` feature!
...
"#]])
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
            edition = "2015"
            include = ["src/lib.rs"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --list")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/lib.rs

"#]])
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
            edition = "2015"
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
        .with_stderr_data("")
        .with_stdout_data(expected)
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
        "Cargo.lock\n\
         Cargo.toml\n\
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
        "Cargo.lock\n\
         Cargo.toml\n\
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
        "Cargo.lock\n\
         Cargo.toml\n\
         Cargo.toml.orig\n\
         foo\n\
         ",
    );

    include_exclude_test(
        "[]",
        r#"["foo/"]"#, // exclude
        &["src/lib.rs", "foo", "a/foo", "x/foo/y", "bar"],
        "Cargo.lock\n\
         Cargo.toml\n\
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
        "Cargo.lock\n\
         Cargo.toml\n\
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
        "Cargo.lock\n\
         Cargo.toml\n\
         Cargo.toml.orig\n\
         a/foo/bar\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "foo/**"]"#, // include
        "[]",
        &["src/lib.rs", "a/foo/bar", "foo/x/y/z"],
        "Cargo.lock\n\
         Cargo.toml\n\
         Cargo.toml.orig\n\
         foo/x/y/z\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "a/**/b"]"#, // include
        "[]",
        &["src/lib.rs", "a/b", "a/x/b", "a/x/y/b"],
        "Cargo.lock\n\
         Cargo.toml\n\
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
         Cargo.lock\n\
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
        "Cargo.lock\n\
         Cargo.toml\n\
         Cargo.toml.orig\n\
         src/lib.rs\n\
         ",
    );

    include_exclude_test(
        r#"["Cargo.toml", "src/*.rs", "!foo.rs"]"#, // include
        "[]",
        &["src/lib.rs", "foo.rs", "src/foo.rs", "src/bar/foo.rs"],
        "Cargo.lock\n\
         Cargo.toml\n\
         Cargo.toml.orig\n\
         src/lib.rs\n\
         ",
    );

    include_exclude_test(
        "[]",
        r#"["*.rs", "!foo.rs", "\\!important"]"#, // exclude
        &["src/lib.rs", "foo.rs", "!important"],
        "Cargo.lock\n\
         Cargo.toml\n\
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
        "Cargo.lock\n\
         Cargo.toml\n\
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
         Cargo.lock\n\
         Cargo.toml\n\
         Cargo.toml.orig\n\
         src/lib.rs\n\
         ",
    );
}

#[cargo_test]
fn empty_readme_path() {
    // fail if `readme` is empty.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2015"
            readme = ""
            license = "MIT"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] readme `` does not appear to exist (relative to `[ROOT]/foo`).
Please update the readme setting in the manifest at `[ROOT]/foo/Cargo.toml`.

"#]])
        .run();
}

#[cargo_test]
fn invalid_readme_path() {
    // fail if `readme` path is invalid.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2015"
            readme = "DOES-NOT-EXIST"
            license = "MIT"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] readme `DOES-NOT-EXIST` does not appear to exist (relative to `[ROOT]/foo`).
Please update the readme setting in the manifest at `[ROOT]/foo/Cargo.toml`.

"#]])
        .run();
}

#[cargo_test]
fn readme_or_license_file_is_dir() {
    // Test error when `readme` or `license-file` is a directory, not a file.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2015"
            readme = "./src"
            license-file = "./src"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] license-file `./src` does not appear to exist (relative to `[ROOT]/foo`).
Please update the license-file setting in the manifest at `[ROOT]/foo/Cargo.toml`.
readme `./src` does not appear to exist (relative to `[ROOT]/foo`).
Please update the readme setting in the manifest at `[ROOT]/foo/Cargo.toml`.

"#]])
        .run();
}

#[cargo_test]
fn empty_license_file_path() {
    // fail if license-file is empty.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            edition = "2015"
            license-file = ""
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] manifest has no license or license-file
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[ERROR] license-file `` does not appear to exist (relative to `[ROOT]/foo`).
Please update the license-file setting in the manifest at `[ROOT]/foo/Cargo.toml`.

"#]])
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
            edition = "2015"
            license-file = "does-not-exist"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] license-file `does-not-exist` does not appear to exist (relative to `[ROOT]/foo`).
Please update the license-file setting in the manifest at `[ROOT]/foo/Cargo.toml`.

"#]])
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
            edition = "2015"
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
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/lib.rs
subdir/LICENSE

"#]])
        .with_stderr_data("")
        .run();

    p.cargo("package --no-verify -v")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v1.0.0 ([ROOT]/foo)
[ARCHIVING] .cargo_vcs_info.json
[ARCHIVING] Cargo.lock
[ARCHIVING] Cargo.toml
[ARCHIVING] Cargo.toml.orig
[ARCHIVING] src/lib.rs
[ARCHIVING] subdir/LICENSE
[PACKAGED] 6 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();
    let f = File::open(&p.root().join("target/package/foo-1.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-1.0.0.crate",
        &[
            ".cargo_vcs_info.json",
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "subdir/LICENSE",
            "src/lib.rs",
        ],
        [("subdir/LICENSE", "license text")],
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
            edition = "2015"
            license-file = "../LICENSE"
            description = "foo"
            homepage = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .file("../LICENSE", "license text")
        .build();

    p.cargo("package --list")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
LICENSE
src/lib.rs

"#]])
        .with_stderr_data("")
        .run();

    p.cargo("package")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v1.0.0 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v1.0.0 ([ROOT]/foo)
[COMPILING] foo v1.0.0 ([ROOT]/foo/target/package/foo-1.0.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    let f = File::open(&p.root().join("target/package/foo-1.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-1.0.0.crate",
        &[
            "Cargo.toml",
            "Cargo.toml.orig",
            "LICENSE",
            "src/lib.rs",
            "Cargo.lock",
        ],
        [("LICENSE", "license text")],
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
            edition = "2015"
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
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
LICENSE
src/lib.rs

"#]])
        .with_stderr_data(str![[r#"
[WARNING] license-file `../LICENSE` appears to be a path outside of the package, but there is already a file named `LICENSE` in the root of the package. The archived crate will contain the copy in the root of the package. Update the license-file to point to the path relative to the root of the package to remove this warning.

"#]])
        .run();

    p.cargo("package").with_stderr_data(str![[r#"
[WARNING] license-file `../LICENSE` appears to be a path outside of the package, but there is already a file named `LICENSE` in the root of the package. The archived crate will contain the copy in the root of the package. Update the license-file to point to the path relative to the root of the package to remove this warning.
[PACKAGING] foo v1.0.0 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v1.0.0 ([ROOT]/foo)
[COMPILING] foo v1.0.0 ([ROOT]/foo/target/package/foo-1.0.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
    let f = File::open(&p.root().join("target/package/foo-1.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-1.0.0.crate",
        &[
            "Cargo.toml",
            "Cargo.toml.orig",
            "LICENSE",
            "src/lib.rs",
            "Cargo.lock",
        ],
        [("LICENSE", "inner license")],
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
            edition = "2015"
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
        .with_stderr_data(
            str![[r#"
[WARNING] file src/con.rs is a reserved Windows filename, it will not work on Windows platforms
[WARNING] file src/aux/mod.rs is a reserved Windows filename, it will not work on Windows platforms
[PACKAGING] foo v0.1.0 ([ROOT]/foo)
[PACKAGED] 6 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.1.0 ([ROOT]/foo)
[COMPILING] foo v0.1.0 ([ROOT]/foo/target/package/foo-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
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
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
ignoreme
ignoreme2
src/lib.rs

"#]])
        .run();

    p.change_file(".gitignore", "ignoreme");
    p.cargo("package --list --allow-dirty")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
ignoreme2
src/lib.rs

"#]])
        .run();

    fs::write(repo_path.join(".gitignore"), "ignoreme2").unwrap();
    p.cargo("package --list --allow-dirty")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/lib.rs

"#]])
        .run();
}

#[cargo_test(
    nightly,
    reason = "temporarily due to flakiness: https://rust-lang.zulipchat.com/#narrow/channel/246057-t-cargo/topic/reserved_windows_name.20test.20failing/with/543085230"
)]
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
...
[ERROR] failed to verify package tarball

Caused by:
  failed to download replaced source registry `crates-io`

Caused by:
  failed to unpack package `bar v1.0.0 (registry `dummy-registry`)`

Caused by:
  failed to unpack entry at `bar-1.0.0/src/aux.rs`

Caused by:
  `bar-1.0.0/src/aux.rs` appears to contain a reserved Windows path, it cannot be extracted on Windows

Caused by:
  failed to unpack `[ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.0/src/aux.rs`

Caused by:
  failed to unpack `bar-1.0.0/src/aux.rs` into `[ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.0/src/aux.rs`

Caused by:
  [NOT_FOUND]

"#]])
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
            edition = "2015"
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
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();

    p.cargo("package")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to verify manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  all dependencies must have a version requirement specified when packaging.
  dependency `bar` does not specify a version
  Note: The packaged dependency will use the version from crates.io,
  the `path` specification will be removed from the dependency declaration.

"#]])
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
            // and always display text, even without --no-capture
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
            edition = "2015"
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
    p.cargo("package --list").with_stdout_data(str![[r#"
012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]]).run();
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
                edition = "2015"
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
                edition = "2015"
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
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
    p.cargo("package --allow-dirty").run();
    let mut index = t!(repo.index());
    t!(index.remove(Path::new("src/lib.rs"), 0));
    t!(index.write());
    p.cargo("package --allow-dirty --list")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
    p.cargo("package --allow-dirty").run();
}

#[cargo_test]
fn package_in_workspace_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "bar"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "baz"
            "#,
        )
        .file("baz/src/main.rs", "fn main() {}")
        .build();

    p.cargo("package -p doesnt-exist")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `doesnt-exist` did not match any packages

"#]])
        .run();
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
                edition = "2015"
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
                edition = "2015"
                authors = []
                license = "MIT"
                description = "bar"
                workspace = ".."
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("package --workspace")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] bar v0.0.1 ([ROOT]/foo/bar)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] bar v0.0.1 ([ROOT]/foo/target/package/bar-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[PACKAGING] bar v0.0.1 ([ROOT]/foo/bar)
[PACKAGED] 6 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] bar v0.0.1 ([ROOT]/foo/target/package/bar-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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

    p.cargo("package").with_stderr_data(str![[r#"
[WARNING] readme `../README.md` appears to be a path outside of the package, but there is already a file named `README.md` in the root of the package. The archived crate will contain the copy in the root of the package. Update the readme to point to the path relative to the root of the package to remove this warning.
[PACKAGING] bar v0.0.1 ([ROOT]/foo/bar)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] bar v0.0.1 ([ROOT]/foo/target/package/bar-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
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
    let rewritten_toml = str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2021"
name = "bar"
version = "0.1.0"
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
readme = false
resolver = "1"

[lib]
name = "bar"
path = "src/lib.rs"

"##]];
    validate_crate_contents(
        f,
        "bar-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        [("Cargo.toml", rewritten_toml)],
    );

    // When the crate has the same implicit resolver as the workspace it is not overridden
    let f = File::open(&p.root().join("target/package/baz-0.1.0.crate")).unwrap();
    let rewritten_toml = str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "baz"
version = "0.1.0"
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
readme = false

[lib]
name = "baz"
path = "src/lib.rs"

"##]];
    validate_crate_contents(
        f,
        "baz-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        [("Cargo.toml", rewritten_toml)],
    );
}

fn verify_packaged_status_line(
    output: cargo_test_support::RawOutput,
    num_files: usize,
    uncompressed_size: u64,
    compressed_size: u64,
) {
    use cargo::util::HumanBytes;

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
    let uncompressed = HumanBytes(uncompressed_size);
    let compressed = HumanBytes(compressed_size);
    let expected = format!("{num_files} files, {uncompressed:.1} ({compressed:.1} compressed)");
    assert_eq!(size_info, expected);
}

#[cargo_test]
fn basic_filesizes() {
    let cargo_toml_orig_contents = r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                exclude = ["*.txt"]
                license = "MIT"
                description = "foo"
                homepage = "https://example.com/"
            "#;
    let main_rs_contents = r#"fn main() { println!("🦀"); }"#;
    let cargo_toml_contents = format!(
        r#"{}
[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
exclude = ["*.txt"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
homepage = "https://example.com/"
readme = false
license = "MIT"

[[bin]]
name = "foo"
path = "src/main.rs"
"#,
        cargo::core::manifest::MANIFEST_PREAMBLE
    );
    let cargo_lock_contents = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

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
    let output = p.cargo("package").run();

    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    let compressed_size = f.metadata().unwrap().len();
    verify_packaged_status_line(output, 4, uncompressed_size, compressed_size);
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        [
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
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "https://example.com/"
            "#;
    let lots_of_crabs = "🦀".repeat(1337);
    let main_rs_contents = format!(r#"fn main() {{ println!("{}"); }}"#, lots_of_crabs);
    let bar_txt_contents = "This file is relatively incompressible, to increase the compressed
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
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "https://example.com/"
readme = false
license = "MIT"

[[bin]]
name = "foo"
path = "src/main.rs"
"#,
        cargo::core::manifest::MANIFEST_PREAMBLE
    );
    let cargo_lock_contents = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

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

    let output = p.cargo("package").run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/bar.txt
src/main.rs

"#]])
        .run();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

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
        [
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
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                homepage = "https://example.com/"
            "#;
    let lots_of_crabs = "🦀".repeat(1337);
    let main_rs_contents = format!(r#"fn main() {{ println!("{}"); }}"#, lots_of_crabs);
    let bar_txt_contents = "This file is relatively incompressible, to increase the compressed
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
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
homepage = "https://example.com/"
readme = false
license = "MIT"

[[bin]]
name = "foo"
path = "src/main.rs"
"#,
        cargo::core::manifest::MANIFEST_PREAMBLE
    );
    let cargo_lock_contents = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

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

    let output = p.cargo("package").run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
bla/bar.txt
foo/bar.txt
src/main.rs
src/main.rs.bak

"#]])
        .run();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 7 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

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
        [
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
        .file("Build.rs", r#"fn main() { println!("hello"); }"#)
        .file("src/Main.rs", r#"fn main() { println!("hello"); }"#)
        .file("src/lib.rs", "")
        .file("src/bar.txt", "") // should be ignored when packaging
        .file("Examples/ExampleFoo.rs", "")
        .file("Tests/ExplicitPath.rs", "")
        .build();
    // Workaround `project()` making a `Cargo.toml` on our behalf
    std::fs::remove_file(p.root().join("Cargo.toml")).unwrap();
    std::fs::write(
        p.root().join("cargo.toml"),
        r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2018"
                authors = []
                exclude = ["*.txt"]
                license = "MIT"
                description = "foo"

                [[test]]
                name = "explicitpath"
                path = "tests/explicitpath.rs"
            "#,
    )
    .unwrap();

    p.cargo("package").with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[WARNING] ignoring `package.build` entry `build.rs` as it is not included in the published package
[WARNING] ignoring binary `foo` as `src/main.rs` is not included in the published package
[WARNING] ignoring example `ExampleFoo` as `examples/ExampleFoo.rs` is not included in the published package
[WARNING] ignoring test `ExplicitPath` as `tests/ExplicitPath.rs` is not included in the published package
[WARNING] ignoring test `explicitpath` as `tests/explicitpath.rs` is not included in the published package
[PACKAGED] 8 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Build.rs
Cargo.lock
Cargo.toml
Cargo.toml.orig
Examples/ExampleFoo.rs
Tests/ExplicitPath.rs
src/Main.rs
src/lib.rs

"#]])
        .run();
    p.cargo("package").with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[WARNING] ignoring `package.build` entry `build.rs` as it is not included in the published package
[WARNING] ignoring binary `foo` as `src/main.rs` is not included in the published package
[WARNING] ignoring example `ExampleFoo` as `examples/ExampleFoo.rs` is not included in the published package
[WARNING] ignoring test `ExplicitPath` as `tests/ExplicitPath.rs` is not included in the published package
[WARNING] ignoring test `explicitpath` as `tests/explicitpath.rs` is not included in the published package
[PACKAGED] 8 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "Build.rs",
            "src/Main.rs",
            "src/lib.rs",
            "Examples/ExampleFoo.rs",
            "Tests/ExplicitPath.rs",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2018"
name = "foo"
version = "0.0.1"
authors = []
build = false
exclude = ["*.txt"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

"##]],
        )],
    );
}

#[cargo_test]
#[cfg(target_os = "linux")] // linux is generally configured to be case sensitive
fn mixed_case() {
    let manifest = r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert!(p.root().join("target/package/foo-0.0.1.crate").is_file());
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
    p.cargo("package")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        (),
    );
}

#[cargo_test]
fn versionless_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                description = "foo"
                edition = "2015"
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("package")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.0 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.0 ([ROOT]/foo)
[COMPILING] foo v0.0.0 ([ROOT]/foo/target/package/foo-0.0.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.0.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        (),
    );
}

#[cargo_test]
fn include_files_called_target_project() {
    // https://github.com/rust-lang/cargo/issues/12790
    // files and folders called "target" should be included, unless they're the actual target directory
    let p = init_and_add_inner_target(project())
        .file("target/foo.txt", "")
        .build();

    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
data/not_target
data/target
derp/not_target/foo.txt
derp/target/foo.txt
src/main.rs

"#]])
        .run();
}

#[cargo_test]
fn include_files_called_target_git() {
    // https://github.com/rust-lang/cargo/issues/12790
    // files and folders called "target" should be included, unless they're the actual target directory
    let (p, repo) = git::new_repo("foo", |p| init_and_add_inner_target(p));
    // add target folder but not committed.
    _ = fs::create_dir(p.build_dir()).unwrap();
    _ = fs::write(p.build_dir().join("foo.txt"), "").unwrap();
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
data/not_target
data/target
derp/not_target/foo.txt
derp/target/foo.txt
src/main.rs

"#]])
        .run();

    // if target is committed, it should be included.
    git::add(&repo);
    git::commit(&repo);
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
data/not_target
data/target
derp/not_target/foo.txt
derp/target/foo.txt
src/main.rs
target/foo.txt

"#]])
        .run();

    // Untracked files shouldn't be included, if they are also ignored.
    _ = fs::write(repo.workdir().unwrap().join(".gitignore"), "target/").unwrap();
    git::add(&repo);
    git::commit(&repo);
    _ = fs::write(p.build_dir().join("untracked.txt"), "").unwrap();
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
.gitignore
Cargo.lock
Cargo.toml
Cargo.toml.orig
data/not_target
data/target
derp/not_target/foo.txt
derp/target/foo.txt
src/main.rs
target/foo.txt

"#]])
        .run();
}

fn init_and_add_inner_target(p: ProjectBuilder) -> ProjectBuilder {
    p.file(
        "Cargo.toml",
        r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
    )
    .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
    // file called target, should be included
    .file("data/target", "")
    .file("data/not_target", "")
    // folder called target, should be included
    .file("derp/target/foo.txt", "")
    .file("derp/not_target/foo.txt", "")
}

#[cargo_test]
fn build_script_outside_pkg_root() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    authors = []
    build = "../t_custom_build/custom_build.rs"
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    // custom_build.rs does not exist
    p.cargo("package -l")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[ERROR] the source file of build script doesn't appear to exist.
This may cause issue during packaging, as modules resolution and resources included via macros are often relative to the path of source files.
Please update the `build` setting in the manifest at `[ROOT]/foo/Cargo.toml` and point to a path inside the root of the package.

"#]])
        .run();

    // custom_build.rs outside the package root
    let custom_build_root = paths::root().join("t_custom_build");
    _ = fs::create_dir(&custom_build_root).unwrap();
    _ = fs::write(&custom_build_root.join("custom_build.rs"), "fn main() {}");
    p.cargo("package -l")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[ERROR] the source file of build script doesn't appear to be a path inside of the package.
It is at `[ROOT]/t_custom_build/custom_build.rs`, whereas the root the package is `[ROOT]/foo`.
This may cause issue during packaging, as modules resolution and resources included via macros are often relative to the path of source files.
Please update the `build` setting in the manifest at `[ROOT]/foo/Cargo.toml` and point to a path inside the root of the package.

"#]])
        .run();
}

#[cargo_test]
fn symlink_manifest_path() {
    // Test `cargo install --manifest-path` pointing through a symlink.
    if !symlink_supported() {
        return;
    }
    let p = git::new("foo", |p| {
        p.file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
            .file("src/main.rs", "fn main() {}")
            // Triggers discover_git_and_list_files for detecting changed files.
            .file("build.rs", "fn main() {}")
    });
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir as symlink;

    let foo_symlink = paths::root().join("foo-symlink");
    t!(symlink(p.root(), &foo_symlink));

    cargo_process("package --no-verify --manifest-path")
        .arg(foo_symlink.join("Cargo.toml"))
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v1.0.0 ([ROOT]/foo-symlink)
[PACKAGED] 6 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();
}

#[cargo_test]
#[cfg(windows)]
fn normalize_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    readme = ".\\docs\\README.md"
    license-file = ".\\docs\\LICENSE"
    build = ".\\src\\build.rs"

    [lib]
    path = ".\\src\\lib.rs"

    [[bin]]
    name = "foo"
    path = ".\\src\\bin\\foo\\main.rs"

    [[example]]
    name = "example_foo"
    path = ".\\examples\\example_foo.rs"

    [[test]]
    name = "test_foo"
    path = ".\\tests\\test_foo.rs"

    [[bench]]
    name = "bench_foo"
    path = ".\\benches\\bench_foo.rs"
    "#,
        )
        .file("src/lib.rs", "")
        .file("docs/README.md", "")
        .file("docs/LICENSE", "")
        .file("src/build.rs", "fn main() {}")
        .file("src/bin/foo/main.rs", "fn main() {}")
        .file("examples/example_foo.rs", "fn main() {}")
        .file("tests/test_foo.rs", "fn main() {}")
        .file("benches/bench_foo.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 11 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "docs/README.md",
            "docs/LICENSE",
            "src/build.rs",
            "src/bin/foo/main.rs",
            "examples/example_foo.rs",
            "tests/test_foo.rs",
            "benches/bench_foo.rs",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = "src/build.rs"
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = "docs/README.md"
license-file = "docs/LICENSE"

[lib]
name = "foo"
path = "src/lib.rs"

[[bin]]
name = "foo"
path = "src/bin/foo/main.rs"

[[example]]
name = "example_foo"
path = "examples/example_foo.rs"

[[test]]
name = "test_foo"
path = "tests/test_foo.rs"

[[bench]]
name = "bench_foo"
path = "benches/bench_foo.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_inferred_build_rs_included() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/lib.rs", "build.rs"]
    "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "build.rs",
            "Cargo.lock",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = "build.rs"
include = [
    "src/lib.rs",
    "build.rs",
]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_inferred_build_rs_excluded() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/lib.rs"]
    "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[WARNING] ignoring `package.build` entry `build.rs` as it is not included in the published package
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = ["src/lib.rs"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_explicit_build_rs_included() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/lib.rs", "build.rs"]
    build = "build.rs"
    "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "build.rs",
            "Cargo.lock",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = "build.rs"
include = [
    "src/lib.rs",
    "build.rs",
]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_explicit_build_rs_excluded() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/lib.rs"]
    build = "build.rs"
    "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[WARNING] ignoring `package.build` entry `build.rs` as it is not included in the published package
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = ["src/lib.rs"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_inferred_lib_included() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/main.rs", "src/lib.rs"]
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "src/lib.rs",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = [
    "src/main.rs",
    "src/lib.rs",
]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

[[bin]]
name = "foo"
path = "src/main.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_inferred_lib_excluded() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/main.rs"]
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[WARNING] ignoring library `foo` as `src/lib.rs` is not included in the published package
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = ["src/main.rs"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[[bin]]
name = "foo"
path = "src/main.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_explicit_lib_included() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/main.rs", "src/lib.rs"]

    [lib]
    path = "src/lib.rs"
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "src/lib.rs",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = [
    "src/main.rs",
    "src/lib.rs",
]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

[[bin]]
name = "foo"
path = "src/main.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_explicit_lib_excluded() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/main.rs"]

    [lib]
    path = "src/lib.rs"
    "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[WARNING] ignoring library `foo` as `src/lib.rs` is not included in the published package
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = ["src/main.rs"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[[bin]]
name = "foo"
path = "src/main.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_inferred_other_included() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/lib.rs", "src/bin/foo/main.rs", "examples/example_foo.rs", "tests/test_foo.rs", "benches/bench_foo.rs"]
    "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/foo/main.rs", "fn main() {}")
        .file("examples/example_foo.rs", "fn main() {}")
        .file("tests/test_foo.rs", "fn main() {}")
        .file("benches/bench_foo.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 8 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "src/bin/foo/main.rs",
            "examples/example_foo.rs",
            "tests/test_foo.rs",
            "benches/bench_foo.rs",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = [
    "src/lib.rs",
    "src/bin/foo/main.rs",
    "examples/example_foo.rs",
    "tests/test_foo.rs",
    "benches/bench_foo.rs",
]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

[[bin]]
name = "foo"
path = "src/bin/foo/main.rs"

[[example]]
name = "example_foo"
path = "examples/example_foo.rs"

[[test]]
name = "test_foo"
path = "tests/test_foo.rs"

[[bench]]
name = "bench_foo"
path = "benches/bench_foo.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_inferred_other_excluded() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/lib.rs"]
    "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/foo/main.rs", "fn main() {}")
        .file("examples/example_foo.rs", "fn main() {}")
        .file("tests/test_foo.rs", "fn main() {}")
        .file("benches/bench_foo.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[WARNING] ignoring binary `foo` as `src/bin/foo/main.rs` is not included in the published package
[WARNING] ignoring example `example_foo` as `examples/example_foo.rs` is not included in the published package
[WARNING] ignoring test `test_foo` as `tests/test_foo.rs` is not included in the published package
[WARNING] ignoring benchmark `bench_foo` as `benches/bench_foo.rs` is not included in the published package
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = ["src/lib.rs"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_explicit_other_included() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/lib.rs", "src/bin/foo/main.rs", "examples/example_foo.rs", "tests/test_foo.rs", "benches/bench_foo.rs"]

    [[bin]]
    name = "foo"

    [[example]]
    name = "example_foo"

    [[test]]
    name = "test_foo"

    [[bench]]
    name = "bench_foo"
    "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/foo/main.rs", "fn main() {}")
        .file("examples/example_foo.rs", "fn main() {}")
        .file("tests/test_foo.rs", "fn main() {}")
        .file("benches/bench_foo.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 8 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "src/bin/foo/main.rs",
            "examples/example_foo.rs",
            "tests/test_foo.rs",
            "benches/bench_foo.rs",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = [
    "src/lib.rs",
    "src/bin/foo/main.rs",
    "examples/example_foo.rs",
    "tests/test_foo.rs",
    "benches/bench_foo.rs",
]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

[[bin]]
name = "foo"
path = "src/bin/foo/main.rs"

[[example]]
name = "example_foo"
path = "examples/example_foo.rs"

[[test]]
name = "test_foo"
path = "tests/test_foo.rs"

[[bench]]
name = "bench_foo"
path = "benches/bench_foo.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn discovery_explicit_other_excluded() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2015"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []
    include = ["src/lib.rs"]

    [[main]]
    name = "foo"

    [[example]]
    name = "example_foo"

    [[test]]
    name = "test_foo"

    [[bench]]
    name = "bench_foo"
    "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/foo/main.rs", "fn main() {}")
        .file("examples/example_foo.rs", "fn main() {}")
        .file("tests/test_foo.rs", "fn main() {}")
        .file("benches/bench_foo.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[WARNING] ignoring binary `foo` as `src/bin/foo/main.rs` is not included in the published package
[WARNING] ignoring example `example_foo` as `examples/example_foo.rs` is not included in the published package
[WARNING] ignoring test `test_foo` as `tests/test_foo.rs` is not included in the published package
[WARNING] ignoring benchmark `bench_foo` as `benches/bench_foo.rs` is not included in the published package
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.0.1"
authors = []
build = false
include = ["src/lib.rs"]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

"##]],
        )],
    );
}

#[cargo_test]
fn deterministic_build_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
    [package]
    name = "foo"
    version = "0.0.1"
    edition = "2021"
    license = "MIT"
    description = "foo"
    documentation = "docs.rs/foo"
    authors = []

    [[example]]
    name = "c"

    [[example]]
    name = "b"

    [[example]]
    name = "a"
    "#,
        )
        .file("src/lib.rs", "")
        .file("examples/z.rs", "fn main() {}")
        .file("examples/y.rs", "fn main() {}")
        .file("examples/x.rs", "fn main() {}")
        .file("examples/c.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 10 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            "examples/a.rs",
            "examples/b.rs",
            "examples/c.rs",
            "examples/x.rs",
            "examples/y.rs",
            "examples/z.rs",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2021"
name = "foo"
version = "0.0.1"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[lib]
name = "foo"
path = "src/lib.rs"

[[example]]
name = "a"
path = "examples/a.rs"

[[example]]
name = "b"
path = "examples/b.rs"

[[example]]
name = "c"
path = "examples/c.rs"

[[example]]
name = "x"
path = "examples/x.rs"

[[example]]
name = "y"
path = "examples/y.rs"

[[example]]
name = "z"
path = "examples/z.rs"

"##]],
        )],
    );
}

// A workspace with three projects that depend on one another (level1 -> level2 -> level3).
// level1 is a binary package, to test lockfile generation.
fn workspace_with_local_deps_project() -> Project {
    project()
            .file(
                "Cargo.toml",
                r#"
            [workspace]
            members = ["level1", "level2", "level3"]

            [workspace.dependencies]
            level2 = { path = "level2", version = "0.0.1" }
        "#
            )
            .file(
                "level1/Cargo.toml",
                r#"
            [package]
            name = "level1"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level1"
            repository = "bar"

            [dependencies]
            # Let one dependency also specify features, for the added test coverage when generating package files.
            level2 = { workspace = true, features = ["foo"] }
        "#,
            )
            .file("level1/src/main.rs", "fn main() {}")
            .file(
                "level2/Cargo.toml",
                r#"
            [package]
            name = "level2"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level2"
            repository = "bar"

            [features]
            foo = []

            [dependencies]
            level3 = { path = "../level3", version = "0.0.1" }
        "#
            )
            .file("level2/src/lib.rs", "")
            .file(
                "level3/Cargo.toml",
                r#"
            [package]
            name = "level3"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level3"
            repository = "bar"
        "#,
            )
            .file("level3/src/lib.rs", "")
            .build()
}

#[cargo_test]
fn workspace_with_local_deps() {
    let crates_io = registry::init();
    let p = workspace_with_local_deps_project();

    p.cargo("package")
        .replace_crates_io(crates_io.index_url())
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] level3 v0.0.1 ([ROOT]/foo/level3)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] level2 v0.0.1 ([ROOT]/foo/level2)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] level1 v0.0.1 ([ROOT]/foo/level1)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] level3 v0.0.1 ([ROOT]/foo/level3)
[COMPILING] level3 v0.0.1 ([ROOT]/foo/target/package/level3-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] level2 v0.0.1 ([ROOT]/foo/level2)
[UNPACKING] level3 v0.0.1 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] level3 v0.0.1
[COMPILING] level2 v0.0.1 ([ROOT]/foo/target/package/level2-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] level1 v0.0.1 ([ROOT]/foo/level1)
[UNPACKING] level2 v0.0.1 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] level2 v0.0.1
[COMPILING] level1 v0.0.1 ([ROOT]/foo/target/package/level1-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let generated_lock = str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "level1"
version = "0.0.1"
dependencies = [
 "level2",
]

[[package]]
name = "level2"
version = "0.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "[..]"
dependencies = [
 "level3",
]

[[package]]
name = "level3"
version = "0.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "[..]"

"##]];

    let generated_manifest = str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "level1"
version = "0.0.1"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "level1"
readme = false
license = "MIT"
repository = "bar"

[[bin]]
name = "level1"
path = "src/main.rs"

[dependencies.level2]
version = "0.0.1"
features = ["foo"]

"##]];

    let mut f = File::open(&p.root().join("target/package/level1-0.0.1.crate")).unwrap();

    validate_crate_contents(
        &mut f,
        "level1-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        [
            ("Cargo.lock", generated_lock),
            ("Cargo.toml", generated_manifest),
        ],
    );
}

#[cargo_test]
fn workspace_with_local_dev_deps() {
    let crates_io = registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["main", "dev_dep"]
            resolver = "3"

            [workspace.dependencies]
            dev_dep = { path = "dev_dep", version = "0.0.1" }
        "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2024"
            authors = []
            license = "MIT"
            description = "main"

            [dev-dependencies]
            dev_dep.workspace = true
        "#,
        )
        .file(
            "dev_dep/Cargo.toml",
            r#"
            [package]
            name = "dev_dep"
            version = "0.0.1"
            edition = "2024"
            authors = []
            license = "MIT"
            description = "main"
        "#,
        )
        .file("main/src/lib.rs", "")
        .file("dev_dep/src/lib.rs", "")
        .build();

    p.cargo("package")
        .replace_crates_io(crates_io.index_url())
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] dev_dep v0.0.1 ([ROOT]/foo/dev_dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dev_dep v0.0.1 ([ROOT]/foo/dev_dep)
[COMPILING] dev_dep v0.0.1 ([ROOT]/foo/target/package/dev_dep-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

fn workspace_with_local_deps_packaging_one_fails_project() -> Project {
    project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["level1", "level2"]
        "#,
        )
        .file(
            "level1/Cargo.toml",
            r#"
            [package]
            name = "level1"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level1"
            repository = "bar"

            [dependencies]
            level2 = { path = "../level2", version = "0.0.1" }
        "#,
        )
        .file("level1/src/lib.rs", "")
        .file(
            "level2/Cargo.toml",
            r#"
            [package]
            name = "level2"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level2"
            repository = "bar"
        "#,
        )
        .file("level2/src/lib.rs", "")
        .build()
}

#[cargo_test]
fn workspace_with_local_deps_packaging_one_fails() {
    let crates_io = registry::init();
    let p = workspace_with_local_deps_packaging_one_fails_project();

    // We can't package just level1, because there's a dependency on level2.
    p.cargo("package -p level1")
        .replace_crates_io(crates_io.index_url())
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] level1 v0.0.1 ([ROOT]/foo/level1)
[UPDATING] crates.io index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `level2` found
  location searched: crates.io index
  required by package `level1 v0.0.1 ([ROOT]/foo/level1)`

"#]])
        .run();
}

// Same as workspace_with_local_deps_packaging_one_fails except that we're
// packaging a bin. This fails during lock-file generation instead of during verification.
#[cargo_test]
fn workspace_with_local_deps_packaging_one_bin_fails() {
    let crates_io = registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["level1", "level2"]
        "#,
        )
        .file(
            "level1/Cargo.toml",
            r#"
            [package]
            name = "level1"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level1"
            repository = "bar"

            [dependencies]
            level2 = { path = "../level2", version = "0.0.1" }
        "#,
        )
        .file("level1/src/main.rs", "fn main() {}")
        .file(
            "level2/Cargo.toml",
            r#"
            [package]
            name = "level2"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level2"
            repository = "bar"
        "#,
        )
        .file("level2/src/lib.rs", "")
        .build();

    // We can't package just level1, because there's a dependency on level2.
    p.cargo("package -p level1")
        .replace_crates_io(crates_io.index_url())
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] level1 v0.0.1 ([ROOT]/foo/level1)
[UPDATING] crates.io index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `level2` found
  location searched: crates.io index
  required by package `level1 v0.0.1 ([ROOT]/foo/level1)`

"#]])
        .run();
}

// Here we don't package the whole workspace, but it succeeds because we package a
// dependency-closed subset.
#[cargo_test]
fn workspace_with_local_deps_packaging_one_with_needed_deps() {
    let crates_io = registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["level1", "level2", "level3"]
        "#,
        )
        .file(
            "level1/Cargo.toml",
            r#"
            [package]
            name = "level1"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level1"
            repository = "bar"

            [dependencies]
            level2 = { path = "../level2", version = "0.0.1" }
        "#,
        )
        .file("level1/src/main.rs", "fn main() {}")
        .file(
            "level2/Cargo.toml",
            r#"
            [package]
            name = "level2"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level2"
            repository = "bar"

            [dependencies]
            level3 = { path = "../level3", version = "0.0.1" }
        "#,
        )
        .file("level2/src/lib.rs", "")
        .file(
            "level3/Cargo.toml",
            r#"
            [package]
            name = "level3"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level3"
            repository = "bar"
        "#,
        )
        .file("level3/src/lib.rs", "")
        .build();

    p.cargo("package -p level2 -p level3")
        .replace_crates_io(crates_io.index_url())
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] level3 v0.0.1 ([ROOT]/foo/level3)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] level2 v0.0.1 ([ROOT]/foo/level2)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] level3 v0.0.1 ([ROOT]/foo/level3)
[COMPILING] level3 v0.0.1 ([ROOT]/foo/target/package/level3-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] level2 v0.0.1 ([ROOT]/foo/level2)
[UNPACKING] level3 v0.0.1 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] level3 v0.0.1
[COMPILING] level2 v0.0.1 ([ROOT]/foo/target/package/level2-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

// package --list in a workspace lists all the files in all the packages.
// The output is not very good, though. See https://github.com/rust-lang/cargo/issues/13953
#[cargo_test]
fn workspace_with_local_deps_list() {
    let crates_io = registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["level1", "level2"]
        "#,
        )
        .file(
            "level1/Cargo.toml",
            r#"
            [package]
            name = "level1"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level1"
            repository = "bar"

            [dependencies]
            level2 = { path = "../level2", version = "0.0.1" }
        "#,
        )
        .file("level1/src/main.rs", "fn main() {}")
        .file(
            "level2/Cargo.toml",
            r#"
            [package]
            name = "level2"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level2"
            repository = "bar"
        "#,
        )
        .file("level2/src/lib.rs", "")
        .build();

    p.cargo("package --list")
        .replace_crates_io(crates_io.index_url())
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/lib.rs
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .with_stderr_data("")
        .run();
}

#[cargo_test]
fn workspace_with_local_deps_index_mismatch() {
    registry::init();
    let alt_reg = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();
    // We're publishing to an alternate index, but the manifests don't specify it.
    // The intra-workspace deps won't be found.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["level1", "level2"]
        "#,
        )
        .file(
            "level1/Cargo.toml",
            r#"
            [package]
            name = "level1"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level1"
            repository = "bar"

            [dependencies]
            level2 = { path = "../level2", version = "0.0.1" }
        "#,
        )
        .file("level1/src/main.rs", "fn main() {}")
        .file(
            "level2/Cargo.toml",
            r#"
            [package]
            name = "level2"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level2"
            repository = "bar"
        "#,
        )
        .file("level2/src/lib.rs", "")
        .build();
    p.cargo(&format!("package --index {}", alt_reg.index_url()))
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] level2 v0.0.1 ([ROOT]/foo/level2)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] level1 v0.0.1 ([ROOT]/foo/level1)
[UPDATING] `dummy-registry` index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `level2` found
  location searched: `dummy-registry` index (which is replacing registry `crates-io`)
  required by package `level1 v0.0.1 ([ROOT]/foo/level1)`

"#]])
        .run();
}

#[cargo_test]
fn workspace_with_local_deps_alternative_index() {
    let alt_reg = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["level1", "level2"]
        "#,
        )
        .file(
            "level1/Cargo.toml",
            r#"
            [package]
            name = "level1"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level1"
            repository = "bar"

            [dependencies]
            level2 = { path = "../level2", version = "0.0.1", registry = "alternative" }
        "#,
        )
        .file("level1/src/main.rs", "fn main() {}")
        .file(
            "level2/Cargo.toml",
            r#"
            [package]
            name = "level2"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level2"
            repository = "bar"
        "#,
        )
        .file("level2/src/lib.rs", "")
        .build();

    p.cargo(&format!("package --index {}", alt_reg.index_url()))
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[PACKAGING] level2 v0.0.1 ([ROOT]/foo/level2)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] level1 v0.0.1 ([ROOT]/foo/level1)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] level2 v0.0.1 ([ROOT]/foo/level2)
[COMPILING] level2 v0.0.1 ([ROOT]/foo/target/package/level2-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] level1 v0.0.1 ([ROOT]/foo/level1)
[UPDATING] `alternative` index
[UNPACKING] level2 v0.0.1 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] level2 v0.0.1 (registry `alternative`)
[COMPILING] level1 v0.0.1 ([ROOT]/foo/target/package/level1-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let index = alt_reg.index_url();
    let generated_lock = format!(
        r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "level1"
version = "0.0.1"
dependencies = [
 "level2",
]

[[package]]
name = "level2"
version = "0.0.1"
source = "{index}"
checksum = "[..]"
"#
    );

    let mut f = File::open(&p.root().join("target/package/level1-0.0.1.crate")).unwrap();

    validate_crate_contents(
        &mut f,
        "level1-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        [("Cargo.lock", generated_lock)],
    );
}

fn workspace_with_local_dep_already_published_project() -> Project {
    Package::new("dep", "0.1.0").publish();

    project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
            dep = { path = "../dep", version = "0.1.0" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build()
}

#[cargo_test]
fn workspace_with_local_dep_already_published() {
    let reg = registry::init();
    let p = workspace_with_local_dep_already_published_project();

    p.cargo("package")
        .replace_crates_io(reg.index_url())
        .with_stderr_data(
            str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[UNPACKING] dep v0.1.0 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] dep v0.1.0
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn workspace_with_local_and_remote_deps() {
    let reg = registry::init();

    Package::new("dep", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
            dep = { path = "../dep", version = "0.1.0" }
            old_dep = { package = "dep", version = "0.0.1" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("package")
        .replace_crates_io(reg.index_url())
        .with_stderr_data(
            str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[DOWNLOADING] crates ...
[UNPACKING] dep v0.1.0 (registry `[ROOT]/foo/target/package/tmp-registry`)
[DOWNLOADED] dep v0.0.1
[COMPILING] dep v0.0.1
[COMPILING] dep v0.1.0
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn workspace_with_capitalized_member() {
    let reg = registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
            DEP = { path = "../dep", version = "0.1.0" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "DEP"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("package --no-verify")
        .replace_crates_io(reg.index_url())
        .with_stderr_data(
            str![[r#"
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] DEP v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn workspace_with_renamed_member() {
    let reg = registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["crates/*"]
            "#,
        )
        .file(
            "crates/val-json/Cargo.toml",
            r#"
            [package]
            name = "obeli-sk-val-json"
            version = "0.16.2"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
        "#,
        )
        .file("crates/val-json/src/lib.rs", "pub fn foo() {}")
        .file(
            "crates/concepts/Cargo.toml",
            r#"
            [package]
            name = "obeli-sk-concepts"
            version = "0.16.2"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
            val-json = { package = "obeli-sk-val-json", path = "../val-json", version = "0.16.2" }
        "#,
        )
        .file(
            "crates/concepts/src/lib.rs",
            "pub fn foo() { val_json::foo() }",
        )
        .file(
            "crates/utils/Cargo.toml",
            r#"
            [package]
            name = "obeli-sk-utils"
            version = "0.16.2"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
            concepts = { package = "obeli-sk-concepts", path = "../concepts", version = "0.16.2" }
            val-json = { package = "obeli-sk-val-json", path = "../val-json", version = "0.16.2" }
        "#,
        )
        .file(
            "crates/utils/src/lib.rs",
            "pub fn foo() { val_json::foo(); concepts::foo(); }",
        )
        .build();

    p.cargo("package")
        .replace_crates_io(reg.index_url())
        .with_stderr_data(
            str![[r#"
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] obeli-sk-val-json v0.16.2 ([ROOT]/foo/crates/val-json)
[PACKAGING] obeli-sk-concepts v0.16.2 ([ROOT]/foo/crates/concepts)
[PACKAGING] obeli-sk-utils v0.16.2 ([ROOT]/foo/crates/utils)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] obeli-sk-val-json v0.16.2 ([ROOT]/foo/crates/val-json)
[COMPILING] obeli-sk-val-json v0.16.2 ([ROOT]/foo/target/package/obeli-sk-val-json-0.16.2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] obeli-sk-concepts v0.16.2 ([ROOT]/foo/crates/concepts)
[UNPACKING] obeli-sk-val-json v0.16.2 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] obeli-sk-val-json v0.16.2
[COMPILING] obeli-sk-concepts v0.16.2 ([ROOT]/foo/target/package/obeli-sk-concepts-0.16.2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] obeli-sk-utils v0.16.2 ([ROOT]/foo/crates/utils)
[UNPACKING] obeli-sk-concepts v0.16.2 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] obeli-sk-concepts v0.16.2
[COMPILING] obeli-sk-utils v0.16.2 ([ROOT]/foo/target/package/obeli-sk-utils-0.16.2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn workspace_with_dot_rs_dir() {
    let reg = registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["crates/*"]
            "#,
        )
        .file(
            "crates/foo.rs/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.16.2"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
        "#,
        )
        .file("crates/foo.rs/src/lib.rs", "pub fn foo() {}")
        .file(
            "crates/bar.rs/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.16.2"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
            foo = { path = "../foo.rs", version = "0.16.2" }
        "#,
        )
        .file("crates/bar.rs/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("package")
        .replace_crates_io(reg.index_url())
        .with_stderr_data(
            str![[r#"
[PACKAGING] foo v0.16.2 ([ROOT]/foo/crates/foo.rs)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] bar v0.16.2 ([ROOT]/foo/crates/bar.rs)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.16.2 ([ROOT]/foo/crates/foo.rs)
[COMPILING] foo v0.16.2 ([ROOT]/foo/target/package/foo-0.16.2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] bar v0.16.2 ([ROOT]/foo/crates/bar.rs)
[UNPACKING] foo v0.16.2 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] foo v0.16.2
[COMPILING] bar v0.16.2 ([ROOT]/foo/target/package/bar-0.16.2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn registry_not_in_publish_list() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                publish = [
                    "test"
                ]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("package --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `foo` cannot be packaged.
The registry `alternative` is not listed in the `package.publish` value in Cargo.toml.

"#]])
        .run();
}

#[cargo_test]
fn registry_inferred_from_unique_option() {
    let _registry = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"
            publish = ["alternative"]

            [dependencies]
            dep = { path = "../dep", version = "0.1.0", registry = "alternative" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
            publish = ["alternative"]
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[UNPACKING] dep v0.1.0 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] dep v0.1.0 (registry `alternative`)
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn registry_not_inferred_because_of_conflict() {
    let alt_reg = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"
            publish = ["alternative"]

            [dependencies]
            dep = { path = "../dep", version = "0.1.0", registry = "alternative" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
            publish = ["alternative2"]
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] conflicts between `package.publish` fields in the selected packages

"#]])
        .run();

    p.cargo("package --exclude-lockfile")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] conflicts between `package.publish` fields in the selected packages

"#]])
        .run();

    p.cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] conflicts between `package.publish` fields in the selected packages

"#]])
        .run();

    p.cargo("package --exclude-lockfile --no-verify")
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    p.cargo("package --registry=alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `dep` cannot be packaged.
The registry `alternative` is not listed in the `package.publish` value in Cargo.toml.

"#]])
        .run();

    p.cargo(&format!("package --index {}", alt_reg.index_url()))
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[UNPACKING] dep v0.1.0 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] dep v0.1.0 (registry `alternative`)
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn registry_inference_ignores_unpublishable() {
    let _alt_reg = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"
            publish = false

            [dependencies]
            dep = { path = "../dep", version = "0.1.0", registry = "alternative" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
            publish = ["alternative"]
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[UNPACKING] dep v0.1.0 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] dep v0.1.0 (registry `alternative`)
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("package --registry=alternative")
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn registry_not_inferred_because_of_multiple_options() {
    let _alt_reg = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"
            publish = ["alternative", "alternative2"]

            [dependencies]
            dep = { path = "../dep", version = "0.1.0", registry = "alternative" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
            publish = ["alternative", "alternative2"]
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] --registry is required to disambiguate between "alternative" or "alternative2" registries

"#]])
        .run();

    p.cargo("package --exclude-lockfile")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] --registry is required to disambiguate between "alternative" or "alternative2" registries

"#]])
        .run();

    p.cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] --registry is required to disambiguate between "alternative" or "alternative2" registries

"#]])
        .run();

    p.cargo("package --exclude-lockfile --no-verify")
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    p.cargo("package --registry=alternative")
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[UNPACKING] dep v0.1.0 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] dep v0.1.0 (registry `alternative`)
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn registry_not_inferred_because_of_mismatch() {
    let _alt_reg = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"
            publish = ["alternative"]

            [dependencies]
            dep = { path = "../dep", version = "0.1.0", registry = "alternative" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        // No `publish` field means "any registry", but the presence of this package
        // will stop us from inferring a registry.
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] --registry is required because not all `package.publish` settings agree

"#]])
        .run();

    p.cargo("package --exclude-lockfile")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] --registry is required because not all `package.publish` settings agree

"#]])
        .run();

    p.cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] --registry is required because not all `package.publish` settings agree

"#]])
        .run();

    p.cargo("package --exclude-lockfile --no-verify")
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    p.cargo("package --registry=alternative")
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[UNPACKING] dep v0.1.0 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] dep v0.1.0 (registry `alternative`)
[COMPILING] main v0.0.1 ([ROOT]/foo/target/package/main-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unpublishable_dependency() {
    let _alt_reg = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"

            [dependencies]
            dep = { path = "../dep", version = "0.1.0", registry = "alternative" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
            publish = false
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr_data(str![[r#"
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] `alternative` index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `dep` found
  location searched: `alternative` index
  required by package `main v0.0.1 ([ROOT]/foo/main)`

"#]])
        .run();
}

#[cargo_test]
fn in_package_workspace_with_members_with_features_old() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [workspace]
                members = ["li"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "li/Cargo.toml",
            r#"
                [package]
                name = "li"
                version = "0.0.1"
                edition = "2015"
                rust-version = "1.69"
                description = "li"
                license = "MIT"
            "#,
        )
        .file("li/src/main.rs", "fn main() {}")
        .build();

    p.cargo("package -p li --no-verify")
        .with_stderr_data(str![[r#"
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] li v0.0.1 ([ROOT]/foo/li)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();
}

#[cargo_test]
#[cfg(unix)]
fn simple_with_fifo() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    std::process::Command::new("mkfifo")
        .current_dir(p.root())
        .arg(p.root().join("blocks-when-read"))
        .status()
        .expect("a FIFO can be created");

    // Avoid actual blocking even in case of failure, assuming that what it lists here
    // would also be read eventually.
    p.cargo("package -l")
        .with_stdout_data(str![[r#"
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
}

#[cargo_test]
fn git_core_symlinks_false() {
    if !symlink_supported() {
        return;
    }

    let git_project = git::new("bar", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                description = "bar"
                license = "MIT"
                edition = "2021"
                documentation = "foo"
            "#,
        )
        .file("src/lib.rs", "//! This is a module")
        .symlink("src/lib.rs", "symlink-lib.rs")
        .symlink_dir("src", "symlink-dir")
    });

    let url = git_project.root().to_url().to_string();

    let p = project().build();
    let root = p.root();
    // Remove the default project layout,
    // so we can git-fetch from git_project under the same directory
    fs::remove_dir_all(&root).unwrap();
    fs::create_dir_all(&root).unwrap();
    let repo = git::init(&root);

    let mut cfg = repo.config().unwrap();
    cfg.set_bool("core.symlinks", false).unwrap();

    // let's fetch from git_project so it respects our core.symlinks=false config.
    repo.remote_anonymous(&url)
        .unwrap()
        .fetch(&["HEAD"], None, None)
        .unwrap();
    let rev = repo
        .find_reference("FETCH_HEAD")
        .unwrap()
        .peel_to_commit()
        .unwrap();
    repo.reset(rev.as_object(), git2::ResetType::Hard, None)
        .unwrap();

    p.cargo("package --allow-dirty")
        .with_stderr_data(str![[r#"
[WARNING] found symbolic links that may be checked out as regular files for git repo at `[ROOT]/foo/`
  |
  = [NOTE] this might cause the `.crate` file to include incorrect or incomplete files
  = [HELP] to avoid this, set the Git config `core.symlinks` to `true`
...
[PACKAGING] bar v0.0.0 ([ROOT]/foo)
[PACKAGED] 7 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] bar v0.0.0 ([ROOT]/foo)
[COMPILING] bar v0.0.0 ([ROOT]/foo/target/package/bar-0.0.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/bar-0.0.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "bar-0.0.0.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/lib.rs",
            // We're missing symlink-dir/lib.rs in the `.crate` file.
            "symlink-dir",
            "symlink-lib.rs",
            ".cargo_vcs_info.json",
        ],
        [
            // And their contents are incorrect.
            ("symlink-dir", str!["[ROOT]/bar/src"]),
            ("symlink-lib.rs", str!["[ROOT]/bar/src/lib.rs"]),
        ],
    );
}

#[cargo_test]
fn exclude_lockfile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --list --exclude-lockfile")
        .with_stdout_data(str![[r#"
Cargo.toml
Cargo.toml.orig
src/lib.rs

"#]])
        .with_stderr_data("")
        .run();

    p.cargo("package --exclude-lockfile")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        (),
    );
}

// A failing case from <https://github.com/rust-lang/cargo/issues/15059>
#[cargo_test]
fn unpublished_cyclic_dev_dependencies() {
    registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"

                [dev-dependencies]
                foo = { path = ".", version = "0.0.1" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify --exclude-lockfile")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        // no Cargo.lock
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        (),
    );
}

// A failing case from <https://github.com/rust-lang/cargo/issues/15059>
#[cargo_test]
fn unpublished_dependency() {
    registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"

                [dependencies]
                dep = { path = "./dep", version = "0.0.1" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "dep/Cargo.toml",
            r#"
                [package]
                name = "dep"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("package --no-verify -p foo --exclude-lockfile")
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 3 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.0.1.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.0.1.crate",
        // no Cargo.lock
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        (),
    );
}

// This is a companion to `publish::checksum_changed`, but because this one
// is packaging without dry-run, it should fail.
#[cargo_test]
fn checksum_changed() {
    let registry = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .build();

    Package::new("dep", "1.0.0").publish();
    Package::new("transitive", "1.0.0")
        .dep("dep", "1.0.0")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["dep"]

                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"

                [dependencies]
                dep = { path = "./dep", version = "1.0.0" }
                transitive = "1.0.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "dep/Cargo.toml",
            r#"
                [package]
                name = "dep"
                version = "1.0.0"
                edition = "2015"
            "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("check").run();

    p.cargo("package --workspace")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] dep v1.0.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[ERROR] failed to prepare local package for uploading

Caused by:
  checksum for `dep v1.0.0` changed between lock files

  this could be indicative of a few possible errors:

      * the lock file is corrupt
      * a replacement source in use (e.g., a mirror) returned a different checksum
      * the source itself may be corrupt in one way or another

  unable to verify that `dep v1.0.0` is the same as when the lockfile was generated

"#]])
        .run();
}
