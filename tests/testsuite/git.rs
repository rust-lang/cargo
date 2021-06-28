//! Tests for git support.

use std::env;
use std::fs;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::str;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::{
    basic_lib_manifest, basic_manifest, git, main_file, path2url, project, rustc_host,
};
use cargo_test_support::{sleep_ms, t, Project};

fn disable_git_cli() -> bool {
    // mingw git on Windows does not support Windows-style file URIs.
    // Appveyor in the rust repo has that git up front in the PATH instead
    // of Git-for-Windows, which causes this to fail.
    env::var("CARGO_TEST_DISABLE_GIT_CLI") == Ok("1".to_string())
}

#[cargo_test]
fn cargo_compile_simple_git_dep() {
    let project = project();
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file(
                "src/dep1.rs",
                r#"
                    pub fn hello() -> &'static str {
                        "hello world"
                    }
                "#,
            )
    });

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/main.rs",
            &main_file(r#""{}", dep1::hello()"#, &["dep1"]),
        )
        .build();

    let git_root = git_project.root();

    project
        .cargo("build")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [COMPILING] dep1 v0.5.0 ({}#[..])\n\
             [COMPILING] foo v0.5.0 ([CWD])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
            path2url(&git_root),
            path2url(&git_root),
        ))
        .run();

    assert!(project.bin("foo").is_file());

    project
        .process(&project.bin("foo"))
        .with_stdout("hello world\n")
        .run();
}

#[cargo_test]
fn cargo_compile_git_dep_branch() {
    let project = project();
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file(
                "src/dep1.rs",
                r#"
                    pub fn hello() -> &'static str {
                        "hello world"
                    }
                "#,
            )
    });

    // Make a new branch based on the current HEAD commit
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    let head = repo.find_commit(head).unwrap();
    repo.branch("branchy", &head, true).unwrap();

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    git = '{}'
                    branch = "branchy"

                "#,
                git_project.url()
            ),
        )
        .file(
            "src/main.rs",
            &main_file(r#""{}", dep1::hello()"#, &["dep1"]),
        )
        .build();

    let git_root = git_project.root();

    project
        .cargo("build")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [COMPILING] dep1 v0.5.0 ({}?branch=branchy#[..])\n\
             [COMPILING] foo v0.5.0 ([CWD])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
            path2url(&git_root),
            path2url(&git_root),
        ))
        .run();

    assert!(project.bin("foo").is_file());

    project
        .process(&project.bin("foo"))
        .with_stdout("hello world\n")
        .run();
}

#[cargo_test]
fn cargo_compile_git_dep_tag() {
    let project = project();
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file(
                "src/dep1.rs",
                r#"
                    pub fn hello() -> &'static str {
                        "hello world"
                    }
                "#,
            )
    });

    // Make a tag corresponding to the current HEAD
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    repo.tag(
        "v0.1.0",
        &repo.find_object(head, None).unwrap(),
        &repo.signature().unwrap(),
        "make a new tag",
        false,
    )
    .unwrap();

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    git = '{}'
                    tag = "v0.1.0"
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/main.rs",
            &main_file(r#""{}", dep1::hello()"#, &["dep1"]),
        )
        .build();

    let git_root = git_project.root();

    project
        .cargo("build")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [COMPILING] dep1 v0.5.0 ({}?tag=v0.1.0#[..])\n\
             [COMPILING] foo v0.5.0 ([CWD])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
            path2url(&git_root),
            path2url(&git_root),
        ))
        .run();

    assert!(project.bin("foo").is_file());

    project
        .process(&project.bin("foo"))
        .with_stdout("hello world\n")
        .run();

    project.cargo("build").run();
}

#[cargo_test]
fn cargo_compile_with_nested_paths() {
    let git_project = git::new("dep1", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [project]

                    name = "dep1"
                    version = "0.5.0"
                    authors = ["carlhuda@example.com"]

                    [dependencies.dep2]

                    version = "0.5.0"
                    path = "vendor/dep2"

                    [lib]

                    name = "dep1"
                "#,
            )
            .file(
                "src/dep1.rs",
                r#"
                    extern crate dep2;

                    pub fn hello() -> &'static str {
                        dep2::hello()
                    }
                "#,
            )
            .file("vendor/dep2/Cargo.toml", &basic_lib_manifest("dep2"))
            .file(
                "vendor/dep2/src/dep2.rs",
                r#"
                    pub fn hello() -> &'static str {
                        "hello world"
                    }
                "#,
            )
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    version = "0.5.0"
                    git = '{}'

                    [[bin]]

                    name = "foo"
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/foo.rs",
            &main_file(r#""{}", dep1::hello()"#, &["dep1"]),
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("hello world\n").run();
}

#[cargo_test]
fn cargo_compile_with_malformed_nested_paths() {
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file(
                "src/dep1.rs",
                r#"
                    pub fn hello() -> &'static str {
                        "hello world"
                    }
                "#,
            )
            .file("vendor/dep2/Cargo.toml", "!INVALID!")
            .file(
                "vendor/dep3/Cargo.toml",
                r#"
                [project]
                name = "dep3"
                version = "0.5.0"
                [dependencies]
                subdep1 = { path = "../require-extra-build-step" }
                "#,
            )
            .file("vendor/dep3/src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    version = "0.5.0"
                    git = '{}'

                    [[bin]]

                    name = "foo"
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/foo.rs",
            &main_file(r#""{}", dep1::hello()"#, &["dep1"]),
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("hello world\n").run();
}

#[cargo_test]
fn cargo_compile_with_meta_package() {
    let git_project = git::new("meta-dep", |project| {
        project
            .file("dep1/Cargo.toml", &basic_lib_manifest("dep1"))
            .file(
                "dep1/src/dep1.rs",
                r#"
                    pub fn hello() -> &'static str {
                        "this is dep1"
                    }
                "#,
            )
            .file("dep2/Cargo.toml", &basic_lib_manifest("dep2"))
            .file(
                "dep2/src/dep2.rs",
                r#"
                    pub fn hello() -> &'static str {
                        "this is dep2"
                    }
                "#,
            )
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    version = "0.5.0"
                    git = '{}'

                    [dependencies.dep2]

                    version = "0.5.0"
                    git = '{}'

                    [[bin]]

                    name = "foo"
                "#,
                git_project.url(),
                git_project.url()
            ),
        )
        .file(
            "src/foo.rs",
            &main_file(
                r#""{} {}", dep1::hello(), dep2::hello()"#,
                &["dep1", "dep2"],
            ),
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo"))
        .with_stdout("this is dep1 this is dep2\n")
        .run();
}

#[cargo_test]
fn cargo_compile_with_short_ssh_git() {
    let url = "git@github.com:a/dep";

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep]

                    git = "{}"

                    [[bin]]

                    name = "foo"
                "#,
                url
            ),
        )
        .file(
            "src/foo.rs",
            &main_file(r#""{}", dep1::hello()"#, &["dep1"]),
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stdout("")
        .with_stderr(&format!(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  invalid url `{}`: relative URL without a base
",
            url
        ))
        .run();
}

#[cargo_test]
fn two_revs_same_deps() {
    let bar = git::new("meta-dep", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.0.0"))
            .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    });

    let repo = git2::Repository::open(&bar.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Commit the changes and make sure we trigger a recompile
    bar.change_file("src/lib.rs", "pub fn bar() -> i32 { 2 }");
    git::add(&repo);
    let rev2 = git::commit(&repo);

    let foo = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.0.0"
                    authors = []

                    [dependencies.bar]
                    git = '{}'
                    rev = "{}"

                    [dependencies.baz]
                    path = "../baz"
                "#,
                bar.url(),
                rev1
            ),
        )
        .file(
            "src/main.rs",
            r#"
                extern crate bar;
                extern crate baz;

                fn main() {
                    assert_eq!(bar::bar(), 1);
                    assert_eq!(baz::baz(), 2);
                }
            "#,
        )
        .build();

    let _baz = project()
        .at("baz")
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "baz"
                    version = "0.0.0"
                    authors = []

                    [dependencies.bar]
                    git = '{}'
                    rev = "{}"
                "#,
                bar.url(),
                rev2
            ),
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;
                pub fn baz() -> i32 { bar::bar() }
            "#,
        )
        .build();

    foo.cargo("build -v").run();
    assert!(foo.bin("foo").is_file());
    foo.process(&foo.bin("foo")).run();
}

#[cargo_test]
fn recompilation() {
    let git_project = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("bar"))
            .file("src/bar.rs", "pub fn bar() {}")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.bar]

                    version = "0.5.0"
                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file("src/main.rs", &main_file(r#""{:?}", bar::bar()"#, &["bar"]))
        .build();

    // First time around we should compile both foo and bar
    p.cargo("build")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [COMPILING] bar v0.5.0 ({}#[..])\n\
             [COMPILING] foo v0.5.0 ([CWD])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) \
             in [..]\n",
            git_project.url(),
            git_project.url(),
        ))
        .run();

    // Don't recompile the second time
    p.cargo("build").with_stdout("").run();

    // Modify a file manually, shouldn't trigger a recompile
    git_project.change_file("src/bar.rs", r#"pub fn bar() { println!("hello!"); }"#);

    p.cargo("build").with_stdout("").run();

    p.cargo("update")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`",
            git_project.url()
        ))
        .run();

    p.cargo("build").with_stdout("").run();

    // Commit the changes and make sure we don't trigger a recompile because the
    // lock file says not to change
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);

    println!("compile after commit");
    p.cargo("build").with_stdout("").run();
    p.root().move_into_the_past();

    // Update the dependency and carry on!
    p.cargo("update")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [UPDATING] bar v0.5.0 ([..]) -> #[..]\n\
             ",
            git_project.url()
        ))
        .run();
    println!("going for the last compile");
    p.cargo("build")
        .with_stderr(&format!(
            "[COMPILING] bar v0.5.0 ({}#[..])\n\
             [COMPILING] foo v0.5.0 ([CWD])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) \
             in [..]\n",
            git_project.url(),
        ))
        .run();

    // Make sure clean only cleans one dep
    p.cargo("clean -p foo").with_stdout("").run();
    p.cargo("build")
        .with_stderr(
            "[COMPILING] foo v0.5.0 ([CWD])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) \
             in [..]",
        )
        .run();
}

#[cargo_test]
fn update_with_shared_deps() {
    let git_project = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("bar"))
            .file("src/bar.rs", "pub fn bar() {}")
    });

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.dep1]
                path = "dep1"
                [dependencies.dep2]
                path = "dep2"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate dep1;
                #[allow(unused_extern_crates)]
                extern crate dep2;
                fn main() {}
            "#,
        )
        .file(
            "dep1/Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "dep1"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.bar]
                    version = "0.5.0"
                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file("dep1/src/lib.rs", "")
        .file(
            "dep2/Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "dep2"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.bar]
                    version = "0.5.0"
                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file("dep2/src/lib.rs", "")
        .build();

    // First time around we should compile both foo and bar
    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{git}`
[COMPILING] bar v0.5.0 ({git}#[..])
[COMPILING] [..] v0.5.0 ([..])
[COMPILING] [..] v0.5.0 ([..])
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
            git = git_project.url(),
        ))
        .run();

    // Modify a file manually, and commit it
    git_project.change_file("src/bar.rs", r#"pub fn bar() { println!("hello!"); }"#);
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let old_head = repo.head().unwrap().target().unwrap();
    git::add(&repo);
    git::commit(&repo);

    sleep_ms(1000);

    // By default, not transitive updates
    println!("dep1 update");
    p.cargo("update -p dep1").with_stdout("").run();

    // Don't do anything bad on a weird --precise argument
    println!("bar bad precise update");
    p.cargo("update -p bar --precise 0.1.2")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] Unable to update [..]

Caused by:
  precise value for git is not a git revision: 0.1.2

Caused by:
  unable to parse OID - contains invalid characters; class=Invalid (3)
",
        )
        .run();

    // Specifying a precise rev to the old rev shouldn't actually update
    // anything because we already have the rev in the db.
    println!("bar precise update");
    p.cargo("update -p bar --precise")
        .arg(&old_head.to_string())
        .with_stdout("")
        .run();

    // Updating aggressively should, however, update the repo.
    println!("dep1 aggressive update");
    p.cargo("update -p dep1 --aggressive")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [UPDATING] bar v0.5.0 ([..]) -> #[..]\n\
             ",
            git_project.url()
        ))
        .run();

    // Make sure we still only compile one version of the git repo
    println!("build");
    p.cargo("build")
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.5.0 ({git}#[..])
[COMPILING] [..] v0.5.0 ([CWD][..]dep[..])
[COMPILING] [..] v0.5.0 ([CWD][..]dep[..])
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
            git = git_project.url(),
        ))
        .run();

    // We should be able to update transitive deps
    p.cargo("update -p bar")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`",
            git_project.url()
        ))
        .run();
}

#[cargo_test]
fn dep_with_submodule() {
    let project = project();
    let git_project = git::new("dep1", |project| {
        project.file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
    });
    let git_project2 = git::new("dep2", |project| project.file("lib.rs", "pub fn dep() {}"));

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let url = path2url(git_project2.root()).to_string();
    git::add_submodule(&repo, &url, Path::new("src"));
    git::commit(&repo);

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate dep1; pub fn foo() { dep1::dep() }",
        )
        .build();

    project
        .cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository [..]
[UPDATING] git submodule `file://[..]/dep2`
[COMPILING] dep1 [..]
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();
}

#[cargo_test]
fn dep_with_bad_submodule() {
    let project = project();
    let git_project = git::new("dep1", |project| {
        project.file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
    });
    let git_project2 = git::new("dep2", |project| project.file("lib.rs", "pub fn dep() {}"));

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let url = path2url(git_project2.root()).to_string();
    git::add_submodule(&repo, &url, Path::new("src"));
    git::commit(&repo);

    // now amend the first commit on git_project2 to make submodule ref point to not-found
    // commit
    let repo = git2::Repository::open(&git_project2.root()).unwrap();
    let original_submodule_ref = repo.refname_to_id("refs/heads/master").unwrap();
    let commit = repo.find_commit(original_submodule_ref).unwrap();
    commit
        .amend(
            Some("refs/heads/master"),
            None,
            None,
            None,
            Some("something something"),
            None,
        )
        .unwrap();

    let p = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/lib.rs",
            "extern crate dep1; pub fn foo() { dep1::dep() }",
        )
        .build();

    let expected = format!(
        "\
[UPDATING] git repository [..]
[UPDATING] git submodule `file://[..]/dep2`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.5.0 [..]`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  Unable to update {}

Caused by:
  failed to update submodule `src`

Caused by:
  object not found - no match for id [..]
",
        path2url(git_project.root())
    );

    p.cargo("build")
        .with_stderr(expected)
        .with_status(101)
        .run();
}

#[cargo_test]
fn two_deps_only_update_one() {
    let project = project();
    let git1 = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
            .file("src/lib.rs", "")
    });
    let git2 = git::new("dep2", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep2", "0.5.0"))
            .file("src/lib.rs", "")
    });

    let p = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]
                    git = '{}'
                    [dependencies.dep2]
                    git = '{}'
                "#,
                git1.url(),
                git2.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    fn oid_to_short_sha(oid: git2::Oid) -> String {
        oid.to_string()[..8].to_string()
    }
    fn git_repo_head_sha(p: &Project) -> String {
        let repo = git2::Repository::open(p.root()).unwrap();
        let head = repo.head().unwrap().target().unwrap();
        oid_to_short_sha(head)
    }

    println!("dep1 head sha: {}", git_repo_head_sha(&git1));
    println!("dep2 head sha: {}", git_repo_head_sha(&git2));

    p.cargo("build")
        .with_stderr(
            "[UPDATING] git repository `[..]`\n\
             [UPDATING] git repository `[..]`\n\
             [COMPILING] [..] v0.5.0 ([..])\n\
             [COMPILING] [..] v0.5.0 ([..])\n\
             [COMPILING] foo v0.5.0 ([CWD])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();

    git1.change_file("src/lib.rs", "pub fn foo() {}");
    let repo = git2::Repository::open(&git1.root()).unwrap();
    git::add(&repo);
    let oid = git::commit(&repo);
    println!("dep1 head sha: {}", oid_to_short_sha(oid));

    p.cargo("update -p dep1")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [UPDATING] dep1 v0.5.0 ([..]) -> #[..]\n\
             ",
            git1.url()
        ))
        .run();
}

#[cargo_test]
fn stale_cached_version() {
    let bar = git::new("meta-dep", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.0.0"))
            .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    });

    // Update the git database in the cache with the current state of the git
    // repo
    let foo = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.0.0"
                    authors = []

                    [dependencies.bar]
                    git = '{}'
                "#,
                bar.url()
            ),
        )
        .file(
            "src/main.rs",
            r#"
                extern crate bar;

                fn main() { assert_eq!(bar::bar(), 1) }
            "#,
        )
        .build();

    foo.cargo("build").run();
    foo.process(&foo.bin("foo")).run();

    // Update the repo, and simulate someone else updating the lock file and then
    // us pulling it down.
    bar.change_file("src/lib.rs", "pub fn bar() -> i32 { 1 + 0 }");
    let repo = git2::Repository::open(&bar.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);

    sleep_ms(1000);

    let rev = repo.revparse_single("HEAD").unwrap().id();

    foo.change_file(
        "Cargo.lock",
        &format!(
            r#"
                [[package]]
                name = "foo"
                version = "0.0.0"
                dependencies = [
                 'bar 0.0.0 (git+{url}#{hash})'
                ]

                [[package]]
                name = "bar"
                version = "0.0.0"
                source = 'git+{url}#{hash}'
            "#,
            url = bar.url(),
            hash = rev
        ),
    );

    // Now build!
    foo.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{bar}`
[COMPILING] bar v0.0.0 ({bar}#[..])
[COMPILING] foo v0.0.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            bar = bar.url(),
        ))
        .run();
    foo.process(&foo.bin("foo")).run();
}

#[cargo_test]
fn dep_with_changed_submodule() {
    let project = project();
    let git_project = git::new("dep1", |project| {
        project.file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
    });

    let git_project2 = git::new("dep2", |project| {
        project.file("lib.rs", "pub fn dep() -> &'static str { \"project2\" }")
    });

    let git_project3 = git::new("dep3", |project| {
        project.file("lib.rs", "pub fn dep() -> &'static str { \"project3\" }")
    });

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let mut sub = git::add_submodule(&repo, &git_project2.url().to_string(), Path::new("src"));
    git::commit(&repo);

    let p = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]
                    [dependencies.dep1]
                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/main.rs",
            "
            extern crate dep1;
            pub fn main() { println!(\"{}\", dep1::dep()) }
        ",
        )
        .build();

    println!("first run");
    p.cargo("run")
        .with_stderr(&format!(
            "[UPDATING] git repository `[..]`\n\
             [UPDATING] git submodule `file://[..]/dep2`\n\
             [COMPILING] dep1 v0.5.0 ([..])\n\
             [COMPILING] foo v0.5.0 ([..])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in \
             [..]\n\
             [RUNNING] `target/{}/debug/foo[EXE]`\n",
            rustc_host()
        ))
        .with_stdout("project2\n")
        .run();

    git_project.change_file(
        ".gitmodules",
        &format!(
            "[submodule \"src\"]\n\tpath = src\n\turl={}",
            git_project3.url()
        ),
    );

    // Sync the submodule and reset it to the new remote.
    sub.sync().unwrap();
    {
        let subrepo = sub.open().unwrap();
        subrepo
            .remote_add_fetch("origin", "refs/heads/*:refs/heads/*")
            .unwrap();
        subrepo
            .remote_set_url("origin", &git_project3.url().to_string())
            .unwrap();
        let mut origin = subrepo.find_remote("origin").unwrap();
        origin.fetch(&Vec::<String>::new(), None, None).unwrap();
        let id = subrepo.refname_to_id("refs/remotes/origin/master").unwrap();
        let obj = subrepo.find_object(id, None).unwrap();
        subrepo.reset(&obj, git2::ResetType::Hard, None).unwrap();
    }
    sub.add_to_index(true).unwrap();
    git::add(&repo);
    git::commit(&repo);

    sleep_ms(1000);
    // Update the dependency and carry on!
    println!("update");
    p.cargo("update -v")
        .with_stderr("")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [UPDATING] git submodule `file://[..]/dep3`\n\
             [UPDATING] dep1 v0.5.0 ([..]) -> #[..]\n\
             ",
            git_project.url()
        ))
        .run();

    println!("last run");
    p.cargo("run")
        .with_stderr(&format!(
            "[COMPILING] dep1 v0.5.0 ([..])\n\
             [COMPILING] foo v0.5.0 ([..])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in \
             [..]\n\
             [RUNNING] `target/{}/debug/foo[EXE]`\n",
            rustc_host()
        ))
        .with_stdout("project3\n")
        .run();
}

#[cargo_test]
fn dev_deps_with_testing() {
    let p2 = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
            .file(
                "src/lib.rs",
                r#"
                    pub fn gimme() -> &'static str { "zoidberg" }
                "#,
            )
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dev-dependencies.bar]
                    version = "0.5.0"
                    git = '{}'
                "#,
                p2.url()
            ),
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {}

                #[cfg(test)]
                mod tests {
                    extern crate bar;
                    #[test] fn foo() { bar::gimme(); }
                }
            "#,
        )
        .build();

    // Generate a lock file which did not use `bar` to compile, but had to update
    // `bar` to generate the lock file
    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{bar}`
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            bar = p2.url()
        ))
        .run();

    // Make sure we use the previous resolution of `bar` instead of updating it
    // a second time.
    p.cargo("test")
        .with_stderr(format!(
            "\
[COMPILING] [..] v0.5.0 ([..])
[COMPILING] [..] v0.5.0 ([..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{}/debug/deps/foo-[..][EXE])",
            rustc_host()
        ))
        .with_stdout_contains("test tests::foo ... ok")
        .run();
}

#[cargo_test]
fn git_build_cmd_freshness() {
    let foo = git::new("foo", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.0"
                    authors = []
                    build = "build.rs"
                "#,
            )
            .file("build.rs", "fn main() {}")
            .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
            .file(".gitignore", "src/bar.rs")
    });
    foo.root().move_into_the_past();

    sleep_ms(1000);

    foo.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    foo.cargo("build").with_stdout("").run();

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    foo.change_file("src/bar.rs", "");
    foo.cargo("build").with_stdout("").run();
}

#[cargo_test]
fn git_name_not_always_needed() {
    let p2 = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
            .file(
                "src/lib.rs",
                r#"
                    pub fn gimme() -> &'static str { "zoidberg" }
                "#,
            )
    });

    let repo = git2::Repository::open(&p2.root()).unwrap();
    let mut cfg = repo.config().unwrap();
    let _ = cfg.remove("user.name");
    let _ = cfg.remove("user.email");

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []

                    [dev-dependencies.bar]
                    git = '{}'
                "#,
                p2.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Generate a lock file which did not use `bar` to compile, but had to update
    // `bar` to generate the lock file
    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{bar}`
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            bar = p2.url()
        ))
        .run();
}

#[cargo_test]
fn git_repo_changing_no_rebuild() {
    let bar = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
            .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    });

    // Lock p1 to the first rev in the git repo
    let p1 = project()
        .at("p1")
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "p1"
                    version = "0.5.0"
                    authors = []
                    build = 'build.rs'
                    [dependencies.bar]
                    git = '{}'
                "#,
                bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "fn main() {}")
        .build();
    p1.root().move_into_the_past();
    p1.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{bar}`
[COMPILING] [..]
[COMPILING] [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            bar = bar.url()
        ))
        .run();

    // Make a commit to lock p2 to a different rev
    bar.change_file("src/lib.rs", "pub fn bar() -> i32 { 2 }");
    let repo = git2::Repository::open(&bar.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);

    // Lock p2 to the second rev
    let p2 = project()
        .at("p2")
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "p2"
                    version = "0.5.0"
                    authors = []
                    [dependencies.bar]
                    git = '{}'
                "#,
                bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p2.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{bar}`
[COMPILING] [..]
[COMPILING] [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            bar = bar.url()
        ))
        .run();

    // And now for the real test! Make sure that p1 doesn't get rebuilt
    // even though the git repo has changed.
    p1.cargo("build").with_stdout("").run();
}

#[cargo_test]
fn git_dep_build_cmd() {
    let p = git::new("foo", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.bar]

                    version = "0.5.0"
                    path = "bar"

                    [[bin]]

                    name = "foo"
                "#,
            )
            .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
            .file(
                "bar/Cargo.toml",
                r#"
                    [project]

                    name = "bar"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]
                    build = "build.rs"

                    [lib]
                    name = "bar"
                    path = "src/bar.rs"
                "#,
            )
            .file(
                "bar/src/bar.rs.in",
                r#"
                    pub fn gimme() -> i32 { 0 }
                "#,
            )
            .file(
                "bar/build.rs",
                r#"
                    use std::fs;
                    fn main() {
                        fs::copy("src/bar.rs.in", "src/bar.rs").unwrap();
                    }
                "#,
            )
    });

    p.root().join("bar").move_into_the_past();

    p.cargo("build").run();

    p.process(&p.bin("foo")).with_stdout("0\n").run();

    // Touching bar.rs.in should cause the `build` command to run again.
    p.change_file("bar/src/bar.rs.in", "pub fn gimme() -> i32 { 1 }");

    p.cargo("build").run();

    p.process(&p.bin("foo")).with_stdout("1\n").run();
}

#[cargo_test]
fn fetch_downloads() {
    let bar = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
            .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []
                    [dependencies.bar]
                    git = '{}'
                "#,
                bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("fetch")
        .with_stderr(&format!(
            "[UPDATING] git repository `{url}`",
            url = bar.url()
        ))
        .run();

    p.cargo("fetch").with_stdout("").run();
}

#[cargo_test]
fn warnings_in_git_dep() {
    let bar = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
            .file("src/lib.rs", "fn unused() {}")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []
                    [dependencies.bar]
                    git = '{}'
                "#,
                bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_stderr(&format!(
            "[UPDATING] git repository `{}`\n\
             [COMPILING] bar v0.5.0 ({}#[..])\n\
             [COMPILING] foo v0.5.0 ([CWD])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
            bar.url(),
            bar.url(),
        ))
        .run();
}

#[cargo_test]
fn update_ambiguous() {
    let bar1 = git::new("bar1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
            .file("src/lib.rs", "")
    });
    let bar2 = git::new("bar2", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.6.0"))
            .file("src/lib.rs", "")
    });
    let baz = git::new("baz", |project| {
        project
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                        [package]
                        name = "baz"
                        version = "0.5.0"
                        authors = ["wycats@example.com"]

                        [dependencies.bar]
                        git = '{}'
                    "#,
                    bar2.url()
                ),
            )
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []
                    [dependencies.bar]
                    git = '{}'
                    [dependencies.baz]
                    git = '{}'
                "#,
                bar1.url(),
                baz.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("update -p bar")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] There are multiple `bar` packages in your project, and the specification `bar` \
is ambiguous.
Please re-run this command with `-p <spec>` where `<spec>` is one of the \
following:
  bar:0.[..].0
  bar:0.[..].0
",
        )
        .run();
}

#[cargo_test]
fn update_one_dep_in_repo_with_many_deps() {
    let bar = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
            .file("src/lib.rs", "")
            .file("a/Cargo.toml", &basic_manifest("a", "0.5.0"))
            .file("a/src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []
                    [dependencies.bar]
                    git = '{}'
                    [dependencies.a]
                    git = '{}'
                "#,
                bar.url(),
                bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("update -p bar")
        .with_stderr(&format!("[UPDATING] git repository `{}`", bar.url()))
        .run();
}

#[cargo_test]
fn switch_deps_does_not_update_transitive() {
    let transitive = git::new("transitive", |project| {
        project
            .file("Cargo.toml", &basic_manifest("transitive", "0.5.0"))
            .file("src/lib.rs", "")
    });
    let dep1 = git::new("dep1", |project| {
        project
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                        [package]
                        name = "dep"
                        version = "0.5.0"
                        authors = ["wycats@example.com"]

                        [dependencies.transitive]
                        git = '{}'
                    "#,
                    transitive.url()
                ),
            )
            .file("src/lib.rs", "")
    });
    let dep2 = git::new("dep2", |project| {
        project
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                        [package]
                        name = "dep"
                        version = "0.5.0"
                        authors = ["wycats@example.com"]

                        [dependencies.transitive]
                        git = '{}'
                    "#,
                    transitive.url()
                ),
            )
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []
                    [dependencies.dep]
                    git = '{}'
                "#,
                dep1.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{}`
[UPDATING] git repository `{}`
[COMPILING] transitive [..]
[COMPILING] dep [..]
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dep1.url(),
            transitive.url()
        ))
        .run();

    // Update the dependency to point to the second repository, but this
    // shouldn't update the transitive dependency which is the same.
    p.change_file(
        "Cargo.toml",
        &format!(
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                [dependencies.dep]
                git = '{}'
            "#,
            dep2.url()
        ),
    );

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{}`
[COMPILING] dep [..]
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dep2.url()
        ))
        .run();
}

#[cargo_test]
fn update_one_source_updates_all_packages_in_that_git_source() {
    let dep = git::new("dep", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [package]
                    name = "dep"
                    version = "0.5.0"
                    authors = []

                    [dependencies.a]
                    path = "a"
                "#,
            )
            .file("src/lib.rs", "")
            .file("a/Cargo.toml", &basic_manifest("a", "0.5.0"))
            .file("a/src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []
                    [dependencies.dep]
                    git = '{}'
                "#,
                dep.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();

    let repo = git2::Repository::open(&dep.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Just be sure to change a file
    dep.change_file("src/lib.rs", "pub fn bar() -> i32 { 2 }");
    git::add(&repo);
    git::commit(&repo);

    p.cargo("update -p dep").run();
    let lockfile = p.read_lockfile();
    assert!(
        !lockfile.contains(&rev1.to_string()),
        "{} in {}",
        rev1,
        lockfile
    );
}

#[cargo_test]
fn switch_sources() {
    let a1 = git::new("a1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("a", "0.5.0"))
            .file("src/lib.rs", "")
    });
    let a2 = git::new("a2", |project| {
        project
            .file("Cargo.toml", &basic_manifest("a", "0.5.1"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                [dependencies.b]
                path = "b"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "b/Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "b"
                    version = "0.5.0"
                    authors = []
                    [dependencies.a]
                    git = '{}'
                "#,
                a1.url()
            ),
        )
        .file("b/src/lib.rs", "pub fn main() {}")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `file://[..]a1`
[COMPILING] a v0.5.0 ([..]a1#[..]
[COMPILING] b v0.5.0 ([..])
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.change_file(
        "b/Cargo.toml",
        &format!(
            r#"
                [project]
                name = "b"
                version = "0.5.0"
                authors = []
                [dependencies.a]
                git = '{}'
            "#,
            a2.url()
        ),
    );

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `file://[..]a2`
[COMPILING] a v0.5.1 ([..]a2#[..]
[COMPILING] b v0.5.0 ([..])
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn dont_require_submodules_are_checked_out() {
    let p = project().build();
    let git1 = git::new("dep1", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .file("a/foo", "")
    });
    let git2 = git::new("dep2", |p| p);

    let repo = git2::Repository::open(&git1.root()).unwrap();
    let url = path2url(git2.root()).to_string();
    git::add_submodule(&repo, &url, Path::new("a/submodule"));
    git::commit(&repo);

    git2::Repository::init(&p.root()).unwrap();
    let url = path2url(git1.root()).to_string();
    let dst = paths::home().join("foo");
    git2::Repository::clone(&url, &dst).unwrap();

    git1.cargo("build -v").cwd(&dst).run();
}

#[cargo_test]
fn doctest_same_name() {
    let a2 = git::new("a2", |p| {
        p.file("Cargo.toml", &basic_manifest("a", "0.5.0"))
            .file("src/lib.rs", "pub fn a2() {}")
    });

    let a1 = git::new("a1", |p| {
        p.file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "a"
                    version = "0.5.0"
                    authors = []
                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                a2.url()
            ),
        )
        .file("src/lib.rs", "extern crate a; pub fn a1() {}")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                a1.url()
            ),
        )
        .file(
            "src/lib.rs",
            r#"
                #[macro_use]
                extern crate a;
            "#,
        )
        .build();

    p.cargo("test -v").run();
}

#[cargo_test]
fn lints_are_suppressed() {
    let a = git::new("a", |p| {
        p.file("Cargo.toml", &basic_manifest("a", "0.5.0")).file(
            "src/lib.rs",
            "
            use std::option;
        ",
        )
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                a.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `[..]`
[COMPILING] a v0.5.0 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn denied_lints_are_allowed() {
    let a = git::new("a", |p| {
        p.file("Cargo.toml", &basic_manifest("a", "0.5.0")).file(
            "src/lib.rs",
            "
            #![deny(warnings)]
            use std::option;
        ",
        )
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                a.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `[..]`
[COMPILING] a v0.5.0 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn add_a_git_dep() {
    let git = git::new("git", |p| {
        p.file("Cargo.toml", &basic_manifest("git", "0.5.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    a = {{ path = 'a' }}
                    git = {{ git = '{}' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    p.change_file(
        "a/Cargo.toml",
        &format!(
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                authors = []

                [dependencies]
                git = {{ git = '{}' }}
            "#,
            git.url()
        ),
    );

    p.cargo("build").run();
}

#[cargo_test]
fn two_at_rev_instead_of_tag() {
    let git = git::new("git", |p| {
        p.file("Cargo.toml", &basic_manifest("git1", "0.5.0"))
            .file("src/lib.rs", "")
            .file("a/Cargo.toml", &basic_manifest("git2", "0.5.0"))
            .file("a/src/lib.rs", "")
    });

    // Make a tag corresponding to the current HEAD
    let repo = git2::Repository::open(&git.root()).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    repo.tag(
        "v0.1.0",
        &repo.find_object(head, None).unwrap(),
        &repo.signature().unwrap(),
        "make a new tag",
        false,
    )
    .unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    git1 = {{ git = '{0}', rev = 'v0.1.0' }}
                    git2 = {{ git = '{0}', rev = 'v0.1.0' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("build -v").run();
}

#[cargo_test]
fn include_overrides_gitignore() {
    // Make sure that `package.include` takes precedence over .gitignore.
    let p = git::new("foo", |repo| {
        repo.file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                include = ["src/lib.rs", "ignored.txt", "Cargo.toml"]
            "#,
        )
        .file(
            ".gitignore",
            r#"
                /target
                Cargo.lock
                ignored.txt
            "#,
        )
        .file("src/lib.rs", "")
        .file("ignored.txt", "")
        .file("build.rs", "fn main() {}")
    });

    p.cargo("build").run();
    p.change_file("ignored.txt", "Trigger rebuild.");
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..])
[RUNNING] `[..]build-script-build[..]`
[RUNNING] `rustc --crate-name foo src/lib.rs [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("package --list --allow-dirty")
        .with_stdout(
            "\
Cargo.toml
Cargo.toml.orig
ignored.txt
src/lib.rs
",
        )
        .run();
}

#[cargo_test]
fn invalid_git_dependency_manifest() {
    let project = project();
    let git_project = git::new("dep1", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [project]

                    name = "dep1"
                    version = "0.5.0"
                    authors = ["carlhuda@example.com"]
                    categories = ["algorithms"]
                    categories = ["algorithms"]

                    [lib]

                    name = "dep1"
                "#,
            )
            .file(
                "src/dep1.rs",
                r#"
                    pub fn hello() -> &'static str {
                        "hello world"
                    }
                "#,
            )
    });

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]

                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]

                    [dependencies.dep1]

                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/main.rs",
            &main_file(r#""{}", dep1::hello()"#, &["dep1"]),
        )
        .build();

    let git_root = git_project.root();

    project
        .cargo("build")
        .with_status(101)
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{}`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.5.0 ([..])`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  Unable to update {}

Caused by:
  failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  duplicate key: `categories` for key `project` at line 10 column 21",
            path2url(&git_root),
            path2url(&git_root),
        ))
        .run();
}

#[cargo_test]
fn failed_submodule_checkout() {
    let project = project();
    let git_project = git::new("dep1", |project| {
        project.file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
    });

    let git_project2 = git::new("dep2", |project| project.file("lib.rs", ""));

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();

    let t = thread::spawn(move || {
        while !done2.load(Ordering::SeqCst) {
            if let Ok((mut socket, _)) = listener.accept() {
                drop(socket.write_all(b"foo\r\n"));
            }
        }
    });

    let repo = git2::Repository::open(&git_project2.root()).unwrap();
    let url = format!("https://{}:{}/", addr.ip(), addr.port());
    {
        let mut s = repo.submodule(&url, Path::new("bar"), false).unwrap();
        let subrepo = s.open().unwrap();
        let mut cfg = subrepo.config().unwrap();
        cfg.set_str("user.email", "foo@bar.com").unwrap();
        cfg.set_str("user.name", "Foo Bar").unwrap();
        git::commit(&subrepo);
        s.add_finalize().unwrap();
    }
    git::commit(&repo);
    drop((repo, url));

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let url = path2url(git_project2.root()).to_string();
    git::add_submodule(&repo, &url, Path::new("src"));
    git::commit(&repo);
    drop(repo);

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    project
        .cargo("build")
        .with_status(101)
        .with_stderr_contains("  failed to update submodule `src`")
        .with_stderr_contains("  failed to update submodule `bar`")
        .run();
    project
        .cargo("build")
        .with_status(101)
        .with_stderr_contains("  failed to update submodule `src`")
        .with_stderr_contains("  failed to update submodule `bar`")
        .run();

    done.store(true, Ordering::SeqCst);
    drop(TcpStream::connect(&addr));
    t.join().unwrap();
}

#[cargo_test]
fn use_the_cli() {
    if disable_git_cli() {
        return;
    }
    let project = project();
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
            .file("src/lib.rs", "")
    });

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = []

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            "
                [net]
                git-fetch-with-cli = true
            ",
        )
        .build();

    let stderr = "\
[UPDATING] git repository `[..]`
[RUNNING] `git fetch [..]`
[COMPILING] dep1 [..]
[RUNNING] `rustc [..]`
[COMPILING] foo [..]
[RUNNING] `rustc [..]`
[FINISHED] [..]
";

    project.cargo("build -v").with_stderr(stderr).run();
}

#[cargo_test]
fn templatedir_doesnt_cause_problems() {
    let git_project2 = git::new("dep2", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep2", "0.5.0"))
            .file("src/lib.rs", "")
    });
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
            .file("src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "fo"
                    version = "0.5.0"
                    authors = []

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    fs::write(
        paths::home().join(".gitconfig"),
        format!(
            r#"
                [init]
                templatedir = {}
            "#,
            git_project2
                .url()
                .to_file_path()
                .unwrap()
                .to_str()
                .unwrap()
                .replace("\\", "/")
        ),
    )
    .unwrap();

    p.cargo("build").run();
}

#[cargo_test]
fn git_with_cli_force() {
    if disable_git_cli() {
        return;
    }
    // Supports a force-pushed repo.
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", r#"pub fn f() { println!("one"); }"#)
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [project]
                name = "foo"
                version = "0.0.1"
                edition = "2018"

                [dependencies]
                dep1 = {{ git = "{}" }}
                "#,
                git_project.url()
            ),
        )
        .file("src/main.rs", "fn main() { dep1::f(); }")
        .file(
            ".cargo/config",
            "
            [net]
            git-fetch-with-cli = true
            ",
        )
        .build();
    p.cargo("build").run();
    p.rename_run("foo", "foo1").with_stdout("one").run();

    // commit --amend a change that will require a force fetch.
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    git_project.change_file("src/lib.rs", r#"pub fn f() { println!("two"); }"#);
    git::add(&repo);
    let id = repo.refname_to_id("HEAD").unwrap();
    let commit = repo.find_commit(id).unwrap();
    let tree_id = t!(t!(repo.index()).write_tree());
    t!(commit.amend(
        Some("HEAD"),
        None,
        None,
        None,
        None,
        Some(&t!(repo.find_tree(tree_id)))
    ));
    // Perform the fetch.
    p.cargo("update").run();
    p.cargo("build").run();
    p.rename_run("foo", "foo2").with_stdout("two").run();
}

#[cargo_test]
fn git_fetch_cli_env_clean() {
    if disable_git_cli() {
        return;
    }
    // This tests that git-fetch-with-cli works when GIT_DIR environment
    // variable is set (for whatever reason).
    let git_dep = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
            .file("src/lib.rs", "")
    });

    let git_proj = git::new("foo", |project| {
        project
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"
                    [dependencies]
                    dep1 = {{ git = '{}' }}
                    "#,
                    git_dep.url()
                ),
            )
            .file("src/lib.rs", "pub extern crate dep1;")
            .file(
                ".cargo/config",
                "
                [net]
                git-fetch-with-cli = true
                ",
            )
    });

    // The directory set here isn't too important. Pointing to our own git
    // directory causes git to be confused and fail. Can also point to an
    // empty directory, or a nonexistent one.
    git_proj
        .cargo("fetch")
        .env("GIT_DIR", git_proj.root().join(".git"))
        .run();
}

#[cargo_test]
fn dirty_submodule() {
    // `cargo package` warns for dirty file in submodule.
    let (git_project, repo) = git::new_repo("foo", |project| {
        project
            .file("Cargo.toml", &basic_manifest("foo", "0.5.0"))
            // This is necessary because `git::add` is too eager.
            .file(".gitignore", "/target")
    });
    let git_project2 = git::new("src", |project| {
        project.no_manifest().file("lib.rs", "pub fn f() {}")
    });

    let url = path2url(git_project2.root()).to_string();
    git::add_submodule(&repo, &url, Path::new("src"));

    // Submodule added, but not committed.
    git_project
        .cargo("package --no-verify")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] manifest has no [..]
See [..]
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

.gitmodules

to proceed despite [..]
",
        )
        .run();

    git::commit(&repo);
    git_project.cargo("package --no-verify").run();

    // Modify file, check for warning.
    git_project.change_file("src/lib.rs", "");
    git_project
        .cargo("package --no-verify")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] manifest has no [..]
See [..]
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

src/lib.rs

to proceed despite [..]
",
        )
        .run();
    // Commit the change.
    let sub_repo = git2::Repository::open(git_project.root().join("src")).unwrap();
    git::add(&sub_repo);
    git::commit(&sub_repo);
    git::add(&repo);
    git::commit(&repo);
    git_project.cargo("package --no-verify").run();

    // Try with a nested submodule.
    let git_project3 = git::new("bar", |project| project.no_manifest().file("mod.rs", ""));
    let url = path2url(git_project3.root()).to_string();
    git::add_submodule(&sub_repo, &url, Path::new("bar"));
    git_project
        .cargo("package --no-verify")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] manifest has no [..]
See [..]
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

src/.gitmodules

to proceed despite [..]
",
        )
        .run();

    // Commit the submodule addition.
    git::commit(&sub_repo);
    git::add(&repo);
    git::commit(&repo);
    git_project.cargo("package --no-verify").run();
    // Modify within nested submodule.
    git_project.change_file("src/bar/new_file.rs", "//test");
    git_project
        .cargo("package --no-verify")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] manifest has no [..]
See [..]
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

src/bar/new_file.rs

to proceed despite [..]
",
        )
        .run();
    // And commit the change.
    let sub_sub_repo = git2::Repository::open(git_project.root().join("src/bar")).unwrap();
    git::add(&sub_sub_repo);
    git::commit(&sub_sub_repo);
    git::add(&sub_repo);
    git::commit(&sub_repo);
    git::add(&repo);
    git::commit(&repo);
    git_project.cargo("package --no-verify").run();
}

#[cargo_test]
fn default_not_master() {
    let project = project();

    // Create a repository with a `master` branch, but switch the head to a
    // branch called `main` at the same time.
    let (git_project, repo) = git::new_repo("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", "pub fn foo() {}")
    });
    let head_id = repo.head().unwrap().target().unwrap();
    let head = repo.find_commit(head_id).unwrap();
    repo.branch("main", &head, false).unwrap();
    repo.set_head("refs/heads/main").unwrap();

    // Then create a commit on the new `main` branch so `master` and `main`
    // differ.
    git_project.change_file("src/lib.rs", "pub fn bar() {}");
    git::add(&repo);
    git::commit(&repo);

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "pub fn foo() { dep1::bar() }")
        .build();

    project
        .cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `[..]`
[COMPILING] dep1 v0.5.0 ([..])
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
}

#[cargo_test]
fn historical_lockfile_works() {
    let project = project();

    let (git_project, repo) = git::new_repo("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", "")
    });
    let head_id = repo.head().unwrap().target().unwrap();

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"

                    [dependencies]
                    dep1 = {{ git = '{}', branch = 'master' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    project.cargo("build").run();
    project.change_file(
        "Cargo.lock",
        &format!(
            r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
[[package]]
name = "dep1"
version = "0.5.0"
source = "git+{}#{}"

[[package]]
name = "foo"
version = "0.5.0"
dependencies = [
 "dep1",
]
"#,
            git_project.url(),
            head_id
        ),
    );
    project
        .cargo("build")
        .with_stderr("[FINISHED] [..]\n")
        .run();
}

#[cargo_test]
fn historical_lockfile_works_with_vendor() {
    let project = project();

    let (git_project, repo) = git::new_repo("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", "")
    });
    let head_id = repo.head().unwrap().target().unwrap();

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"

                    [dependencies]
                    dep1 = {{ git = '{}', branch = 'master' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    let output = project.cargo("vendor").exec_with_output().unwrap();
    project.change_file(".cargo/config", str::from_utf8(&output.stdout).unwrap());
    project.change_file(
        "Cargo.lock",
        &format!(
            r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
[[package]]
name = "dep1"
version = "0.5.0"
source = "git+{}#{}"

[[package]]
name = "foo"
version = "0.5.0"
dependencies = [
 "dep1",
]
"#,
            git_project.url(),
            head_id
        ),
    );
    project.cargo("build").run();
}

#[cargo_test]
fn two_dep_forms() {
    let project = project();

    let (git_project, _repo) = git::new_repo("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", "")
    });

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    [dependencies]
                    dep1 = {{ git = '{}', branch = 'master' }}
                    a = {{ path = 'a' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "a"
                    version = "0.5.0"
                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("a/src/lib.rs", "")
        .build();

    // This'll download the git repository twice, one with HEAD and once with
    // the master branch. Then it'll compile 4 crates, the 2 git deps, then
    // the two local deps.
    project
        .cargo("build")
        .with_stderr(
            "\
[UPDATING] [..]
[UPDATING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn metadata_master_consistency() {
    // SourceId consistency in the `cargo metadata` output when `master` is
    // explicit or implicit, using new or old Cargo.lock.
    let (git_project, git_repo) = git::new_repo("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
            .file("src/lib.rs", "")
    });
    let bar_hash = git_repo.head().unwrap().target().unwrap().to_string();

    // Explicit branch="master" with a lock file created before 1.47 (does not contain ?branch=master).
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = {{ git = "{}", branch = "master" }}
            "#,
                git_project.url()
            ),
        )
        .file(
            "Cargo.lock",
            &format!(
                r#"
                    [[package]]
                    name = "bar"
                    version = "1.0.0"
                    source = "git+{}#{}"

                    [[package]]
                    name = "foo"
                    version = "0.1.0"
                    dependencies = [
                     "bar",
                    ]
                "#,
                git_project.url(),
                bar_hash,
            ),
        )
        .file("src/lib.rs", "")
        .build();

    let metadata = |bar_source| -> String {
        r#"
            {
              "packages": [
                {
                  "name": "bar",
                  "version": "1.0.0",
                  "id": "bar 1.0.0 (__BAR_SOURCE__#__BAR_HASH__)",
                  "license": null,
                  "license_file": null,
                  "description": null,
                  "source": "__BAR_SOURCE__#__BAR_HASH__",
                  "dependencies": [],
                  "targets": "{...}",
                  "features": {},
                  "manifest_path": "[..]",
                  "metadata": null,
                  "publish": null,
                  "authors": [],
                  "categories": [],
                  "default_run": null,
                  "keywords": [],
                  "readme": null,
                  "repository": null,
                  "homepage": null,
                  "documentation": null,
                  "edition": "2015",
                  "links": null
                },
                {
                  "name": "foo",
                  "version": "0.1.0",
                  "id": "foo 0.1.0 [..]",
                  "license": null,
                  "license_file": null,
                  "description": null,
                  "source": null,
                  "dependencies": [
                    {
                      "name": "bar",
                      "source": "__BAR_SOURCE__",
                      "req": "*",
                      "kind": null,
                      "rename": null,
                      "optional": false,
                      "uses_default_features": true,
                      "features": [],
                      "target": null,
                      "registry": null
                    }
                  ],
                  "targets": "{...}",
                  "features": {},
                  "manifest_path": "[..]",
                  "metadata": null,
                  "publish": null,
                  "authors": [],
                  "categories": [],
                  "default_run": null,
                  "keywords": [],
                  "readme": null,
                  "repository": null,
                  "homepage": null,
                  "documentation": null,
                  "edition": "2015",
                  "links": null
                }
              ],
              "workspace_members": [
                "foo 0.1.0 [..]"
              ],
              "resolve": {
                "nodes": [
                  {
                    "id": "bar 1.0.0 (__BAR_SOURCE__#__BAR_HASH__)",
                    "dependencies": [],
                    "deps": [],
                    "features": []
                  },
                  {
                    "id": "foo 0.1.0 [..]",
                    "dependencies": [
                      "bar 1.0.0 (__BAR_SOURCE__#__BAR_HASH__)"
                    ],
                    "deps": [
                      {
                        "name": "bar",
                        "pkg": "bar 1.0.0 (__BAR_SOURCE__#__BAR_HASH__)",
                        "dep_kinds": [
                          {
                            "kind": null,
                            "target": null
                          }
                        ]
                      }
                    ],
                    "features": []
                  }
                ],
                "root": "foo 0.1.0 [..]"
              },
              "target_directory": "[..]",
              "version": 1,
              "workspace_root": "[..]",
              "metadata": null
            }
        "#
        .replace("__BAR_SOURCE__", bar_source)
        .replace("__BAR_HASH__", &bar_hash)
    };

    let bar_source = format!("git+{}?branch=master", git_project.url());
    p.cargo("metadata").with_json(&metadata(&bar_source)).run();

    // Conversely, remove branch="master" from Cargo.toml, but use a new Cargo.lock that has ?branch=master.
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = {{ git = "{}" }}
            "#,
                git_project.url()
            ),
        )
        .file(
            "Cargo.lock",
            &format!(
                r#"
                    [[package]]
                    name = "bar"
                    version = "1.0.0"
                    source = "git+{}?branch=master#{}"

                    [[package]]
                    name = "foo"
                    version = "0.1.0"
                    dependencies = [
                     "bar",
                    ]
                "#,
                git_project.url(),
                bar_hash
            ),
        )
        .file("src/lib.rs", "")
        .build();

    // No ?branch=master!
    let bar_source = format!("git+{}", git_project.url());
    p.cargo("metadata").with_json(&metadata(&bar_source)).run();
}
