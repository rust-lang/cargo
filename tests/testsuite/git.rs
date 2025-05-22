//! Tests for git support.

use std::fs;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::str;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use cargo_test_support::git::{add_submodule, cargo_uses_gitoxide};
use cargo_test_support::paths;
use cargo_test_support::prelude::IntoData;
use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, git, main_file, project};
use cargo_test_support::{sleep_ms, str, t, Project};

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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    project
        .cargo("build")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[COMPILING] dep1 v0.5.0 ([ROOTURL]/dep1#[..])
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(project.bin("foo").is_file());

    project
        .process(&project.bin("foo"))
        .with_stdout_data(str![[r#"
hello world

"#]])
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    project
        .cargo("build")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[COMPILING] dep1 v0.5.0 ([ROOTURL]/dep1?branch=branchy#[..])
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(project.bin("foo").is_file());

    project
        .process(&project.bin("foo"))
        .with_stdout_data(str![[r#"
hello world

"#]])
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    project
        .cargo("build")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[COMPILING] dep1 v0.5.0 ([ROOTURL]/dep1?tag=v0.1.0#[..])
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(project.bin("foo").is_file());

    project
        .process(&project.bin("foo"))
        .with_stdout_data(str![[r#"
hello world

"#]])
        .run();

    project.cargo("build").run();
}

#[cargo_test]
fn cargo_compile_git_dep_pull_request() {
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

    // Make a reference in GitHub's pull request ref naming convention.
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let oid = repo.refname_to_id("HEAD").unwrap();
    let force = false;
    let log_message = "open pull request";
    repo.reference("refs/pull/330/head", oid, force, log_message)
        .unwrap();

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.0"
                    edition = "2015"

                    [dependencies]
                    dep1 = {{ git = "{}", rev = "refs/pull/330/head" }}
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/main.rs",
            &main_file(r#""{}", dep1::hello()"#, &["dep1"]),
        )
        .build();

    project
        .cargo("build")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[COMPILING] dep1 v0.5.0 ([ROOTURL]/dep1?rev=refs%2Fpull%2F330%2Fhead#[..])
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(project.bin("foo").is_file());
}

#[cargo_test]
fn cargo_compile_with_nested_paths() {
    let git_project = git::new("dep1", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [package]

                    name = "dep1"
                    version = "0.5.0"
                    edition = "2015"
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
hello world

"#]])
        .run();
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
                [package]
                name = "dep3"
                version = "0.5.0"
                edition = "2015"
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
hello world

"#]])
        .run();
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
        .with_stdout_data(str![[r#"
this is dep1 this is dep2

"#]])
        .run();
}

#[cargo_test]
fn cargo_compile_with_short_ssh_git() {
    let url = "git@github.com:a/dep";
    let well_formed_url = "ssh://git@github.com/a/dep";

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(&format!(
            "\
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  invalid url `{}`: relative URL without a base; try using `{}` instead
",
            url, well_formed_url
        ))
        .run();
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.5.0 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Don't recompile the second time
    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Modify a file manually, shouldn't trigger a recompile
    git_project.change_file("src/bar.rs", r#"pub fn bar() { println!("hello!"); }"#);

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("update")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 0 packages to latest compatible versions

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Commit the changes and make sure we don't trigger a recompile because the
    // lock file says not to change
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);

    println!("compile after commit");
    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.root().move_into_the_past();

    // Update the dependency and carry on!
    p.cargo("update")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.5.0 ([ROOTURL]/bar#[..]) -> #[..]

"#]])
        .run();
    println!("going for the last compile");
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.5.0 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Make sure clean only cleans one dep
    p.cargo("clean -p foo")
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
                    edition = "2015"
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
                    edition = "2015"
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
    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 3 packages to latest compatible versions
[CHECKING] bar v0.5.0 ([ROOTURL]/bar#[..])
[CHECKING] dep1 v0.5.0 ([ROOT]/foo/dep1)
[CHECKING] dep2 v0.5.0 ([ROOT]/foo/dep2)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
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
    p.cargo("update dep1")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.5.0 ([ROOTURL]/bar#[..]) -> #[..]

"#]])
        .run();

    // Don't do anything bad on a weird --precise argument
    println!("bar bad precise update");
    p.cargo("update bar --precise 0.1.2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[ERROR] Unable to update [ROOTURL]/bar#0.1.2

Caused by:
  revspec '0.1.2' not found; class=Reference (4); code=NotFound (-3)

"#]])
        .run();

    // Specifying a precise rev to the old rev shouldn't actually update
    // anything because we already have the rev in the db.
    println!("bar precise update");
    p.cargo("update bar --precise")
        .arg(&old_head.to_string())
        .with_stderr_data(str![[r#"
[UPDATING] bar v0.5.0 ([ROOTURL]/bar#[..]) -> #[..]

"#]])
        .run();

    // Updating recursively should, however, update the repo.
    println!("dep1 recursive update");
    p.cargo("update dep1 --recursive")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.5.0 ([ROOTURL]/bar#[..]) -> #[..]

"#]])
        .run();

    // Make sure we still only compile one version of the git repo
    println!("build");
    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[CHECKING] bar v0.5.0 ([ROOTURL]/bar#[..])
[CHECKING] dep1 v0.5.0 ([ROOT]/foo/dep1)
[CHECKING] dep2 v0.5.0 ([ROOT]/foo/dep2)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    // We should be able to update transitive deps
    p.cargo("update bar")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 0 packages to latest compatible versions

"#]])
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
    let url = git_project2.root().to_url().to_string();
    git::add_submodule(&repo, &url, Path::new("src"));
    git::commit(&repo);

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
        .cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git submodule `[ROOTURL]/dep2`
[LOCKING] 1 package to latest compatible version
[CHECKING] dep1 v0.5.0 ([ROOTURL]/dep1#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn dep_with_relative_submodule() {
    let foo = project();
    let base = git::new("base", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
            [package]
            name = "base"
            version = "0.5.0"
            edition = "2015"

            [dependencies]
            deployment.path = "deployment"
        "#,
            )
            .file(
                "src/lib.rs",
                r#"
            pub fn dep() {
                deployment::deployment_func();
            }
            "#,
            )
    });
    let _deployment = git::new("deployment", |project| {
        project
            .file("src/lib.rs", "pub fn deployment_func() {}")
            .file("Cargo.toml", &basic_lib_manifest("deployment"))
    });

    let base_repo = git2::Repository::open(&base.root()).unwrap();
    git::add_submodule(&base_repo, "../deployment", Path::new("deployment"));
    git::commit(&base_repo);

    let project = foo
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"

                    [dependencies.base]
                    git = '{}'
                "#,
                base.url()
            ),
        )
        .file("src/lib.rs", "pub fn foo() {  }")
        .build();

    project
        .cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/base`
[UPDATING] git submodule `[ROOTURL]/deployment`
[LOCKING] 2 packages to latest compatible versions
[CHECKING] deployment v0.5.0 ([ROOTURL]/base#[..])
[CHECKING] base v0.5.0 ([ROOTURL]/base#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
    let url = git_project2.root().to_url().to_string();
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    let expected = str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git submodule `[ROOTURL]/dep2`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.5.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  Unable to update [ROOTURL]/dep1

Caused by:
  failed to update submodule `src`

Caused by:
  object not found - no match for id ([..]); class=Odb (9); code=NotFound (-3)

"#]];

    p.cargo("check")
        .with_stderr_data(expected)
        .with_status(101)
        .run();
}

#[cargo_test]
fn dep_with_skipped_submodule() {
    // Ensure we skip dependency submodules if their update strategy is `none`.
    let qux = git::new("qux", |project| {
        project.no_manifest().file("README", "skip me")
    });

    let bar = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.0.0"))
            .file("src/lib.rs", "")
    });

    // `qux` is a submodule of `bar`, but we don't want to update it.
    let repo = git2::Repository::open(&bar.root()).unwrap();
    git::add_submodule(&repo, qux.url().as_str(), Path::new("qux"));

    let mut conf = git2::Config::open(&bar.root().join(".gitmodules")).unwrap();
    conf.set_str("submodule.qux.update", "none").unwrap();

    git::add(&repo);
    git::commit(&repo);

    let foo = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.0"
                    edition = "2015"
                    authors = []

                    [dependencies.bar]
                    git = "{}"
                "#,
                bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    foo.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[SKIPPING] git submodule `[ROOTURL]/qux` due to update strategy in .gitmodules
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.0.0 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn ambiguous_published_deps() {
    let project = project();
    let git_project = git::new("dep", |project| {
        project
            .file(
                "duplicate1/Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "duplicate"
                    version = "0.5.0"
                    edition = "2015"
                    publish = true
                "#
                ),
            )
            .file("duplicate1/src/lib.rs", "")
            .file(
                "duplicate2/Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "duplicate"
                    version = "0.5.0"
                    edition = "2015"
                    publish = true
                "#
                ),
            )
            .file("duplicate2/src/lib.rs", "")
    });

    let p = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = ["wycats@example.com"]

                    [dependencies.duplicate]
                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file("src/main.rs", "fn main() {  }")
        .build();

    p.cargo("build").run();
    p.cargo("run")
        .with_stderr_data(str![[r#"
[WARNING] skipping duplicate package `duplicate v0.5.0 ([ROOTURL]/dep#[..])`:
  [ROOT]/home/.cargo/git/checkouts/dep-[HASH]/[..]/duplicate2/Cargo.toml
in favor of [ROOT]/home/.cargo/git/checkouts/dep-[HASH]/[..]/duplicate1/Cargo.toml

[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn unused_ambiguous_published_deps() {
    let project = project();
    let git_project = git::new("dep", |project| {
        project
            .file(
                "unique/Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "unique"
                    version = "0.5.0"
                    edition = "2015"
                    publish = true
                "#
                ),
            )
            .file("unique/src/lib.rs", "")
            .file(
                "duplicate1/Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "duplicate"
                    version = "0.5.0"
                    edition = "2015"
                    publish = true
                "#
                ),
            )
            .file("duplicate1/src/lib.rs", "")
            .file(
                "duplicate2/Cargo.toml",
                &format!(
                    r#"
                    [package]
                    name = "duplicate"
                    version = "0.5.0"
                    edition = "2015"
                    publish = true
                "#
                ),
            )
            .file("duplicate2/src/lib.rs", "")
            .file(
                "invalid/Cargo.toml",
                &format!(
                    r#"
                    [package
                    name = "bar"
                    version = "0.5.0"
                    edition = "2015"
                    publish = true
                "#
                ),
            )
            .file("invalid/src/lib.rs", "")
    });

    let p = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = ["wycats@example.com"]

                    [dependencies.unique]
                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file("src/main.rs", "fn main() {  }")
        .build();

    p.cargo("build").run();
    p.cargo("run")
        .with_stderr_data(str![[r#"
[ERROR] invalid table header
expected `.`, `]`
 --> ../home/.cargo/git/checkouts/dep-[HASH]/[..]/invalid/Cargo.toml:2:29
  |
2 |                     [package
  |                             ^
  |
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git repository `[ROOTURL]/dep2`
[LOCKING] 2 packages to latest compatible versions
[CHECKING] dep1 v0.5.0 ([ROOTURL]/dep1#[..])
[CHECKING] dep2 v0.5.0 ([ROOTURL]/dep2#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    git1.change_file("src/lib.rs", "pub fn foo() {}");
    let repo = git2::Repository::open(&git1.root()).unwrap();
    git::add(&repo);
    let oid = git::commit(&repo);
    println!("dep1 head sha: {}", oid_to_short_sha(oid));

    p.cargo("update dep1")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[UPDATING] dep1 v0.5.0 ([ROOTURL]/dep1#[..]) -> #[..]

"#]])
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
                    [package]
                    name = "foo"
                    version = "0.0.0"
                    edition = "2015"
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
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/meta-dep`
[COMPILING] bar v0.0.0 ([ROOTURL]/meta-dep#[..])
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git submodule `[ROOTURL]/dep2`
[LOCKING] 1 package to latest compatible version
[COMPILING] dep1 v0.5.0 ([ROOTURL]/dep1#[..])
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
project2

"#]])
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
    p.cargo("update")
        .with_stdout_data(str![])
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git submodule `[ROOTURL]/dep3`
[LOCKING] 1 package to latest compatible version
[UPDATING] dep1 v0.5.0 ([ROOTURL]/dep1#[..]) -> #[..]

"#]])
        .run();

    println!("last run");
    p.cargo("run")
        .with_stderr_data(str![[r#"
[COMPILING] dep1 v0.5.0 ([ROOTURL]/dep1#[..])
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
project3

"#]])
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Make sure we use the previous resolution of `bar` instead of updating it
    // a second time.
    p.cargo("test")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.5.0 ([ROOTURL]/bar#[..])
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .with_stdout_data(str![[r#"
...
test tests::foo ... ok
...
"#]])
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
                    edition = "2015"
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

    foo.cargo("check")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    foo.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    foo.change_file("src/bar.rs", "");
    foo.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    [package]
                    name = "p1"
                    version = "0.5.0"
                    edition = "2015"
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
    p1.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[COMPILING] p1 v0.5.0 ([ROOT]/p1)
[CHECKING] bar v0.5.0 ([ROOTURL]/bar#[..])
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    [package]
                    name = "p2"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []
                    [dependencies.bar]
                    git = '{}'
                "#,
                bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p2.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.5.0 ([ROOTURL]/bar#[..])
[CHECKING] p2 v0.5.0 ([ROOT]/p2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // And now for the real test! Make sure that p1 doesn't get rebuilt
    // even though the git repo has changed.
    p1.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn git_dep_build_cmd() {
    let p = git::new("foo", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
                    [package]

                    name = "bar"
                    version = "0.5.0"
                    edition = "2015"
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

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
0

"#]])
        .run();

    // Touching bar.rs.in should cause the `build` command to run again.
    p.change_file("bar/src/bar.rs.in", "pub fn gimme() -> i32 { 1 }");

    p.cargo("build").run();

    p.process(&p.bin("foo"))
        .with_stdout_data(str![[r#"
1

"#]])
        .run();
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version

"#]])
        .run();

    p.cargo("fetch").with_stderr_data(str![]).run();
}

#[cargo_test]
fn fetch_downloads_with_git2_first_then_with_gitoxide_and_vice_versa() {
    let bar = git::new("bar", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.5.0"))
            .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    });
    let feature_configuration = if cargo_uses_gitoxide() {
        // When we are always using `gitoxide` by default, create the registry with git2 as well as the download…
        "-Zgitoxide=internal-use-git2"
    } else {
        // …otherwise create the registry and the git download with `gitoxide`.
        "-Zgitoxide=fetch"
    };

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []
                    [dependencies.bar]
                    git = '{url}'
                "#,
                url = bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("fetch")
        .arg(feature_configuration)
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version

"#]])
        .run();

    Package::new("bar", "1.0.0").publish(); // trigger a crates-index change.
    p.cargo("fetch").with_stderr_data(str![]).run();
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []
                    [dependencies.bar]
                    git = '{}'
                "#,
                bar.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.5.0 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                        edition = "2015"
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
    p.cargo("update bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] There are multiple `bar` packages in your project, and the specification `bar` is ambiguous.
Please re-run this command with one of the following specifications:
  bar@0.5.0
  bar@0.6.0

"#]])
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
    p.cargo("update bar")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 0 packages to latest compatible versions

"#]])
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
                        edition = "2015"
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
                        edition = "2015"
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []
                    [dependencies.dep]
                    git = '{}'
                "#,
                dep1.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git repository `[ROOTURL]/transitive`
[LOCKING] 2 packages to latest compatible versions
[CHECKING] transitive v0.5.0 ([ROOTURL]/transitive#[..])
[CHECKING] dep v0.5.0 ([ROOTURL]/dep1#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Update the dependency to point to the second repository, but this
    // shouldn't update the transitive dependency which is the same.
    p.change_file(
        "Cargo.toml",
        &format!(
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []
                [dependencies.dep]
                git = '{}'
            "#,
            dep2.url()
        ),
    );

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep2`
[LOCKING] 1 package to latest compatible version
[ADDING] dep v0.5.0 ([ROOTURL]/dep2#[..])
[CHECKING] dep v0.5.0 ([ROOTURL]/dep2#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    edition = "2015"
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []
                    [dependencies.dep]
                    git = '{}'
                "#,
                dep.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check").run();

    let repo = git2::Repository::open(&dep.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Just be sure to change a file
    dep.change_file("src/lib.rs", "pub fn bar() -> i32 { 2 }");
    git::add(&repo);
    git::commit(&repo);

    p.cargo("update dep").run();
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
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
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
                    [package]
                    name = "b"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []
                    [dependencies.a]
                    git = '{}'
                "#,
                a1.url()
            ),
        )
        .file("b/src/lib.rs", "pub fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/a1`
[LOCKING] 2 packages to latest compatible versions
[CHECKING] a v0.5.0 ([ROOTURL]/a1#[..])
[CHECKING] b v0.5.0 ([ROOT]/foo/b)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.change_file(
        "b/Cargo.toml",
        &format!(
            r#"
                [package]
                name = "b"
                version = "0.5.0"
                edition = "2015"
                authors = []
                [dependencies.a]
                git = '{}'
            "#,
            a2.url()
        ),
    );

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/a2`
[LOCKING] 1 package to latest compatible version
[ADDING] a v0.5.1 ([ROOTURL]/a2#[..])
[CHECKING] a v0.5.1 ([ROOTURL]/a2#[..])
[CHECKING] b v0.5.0 ([ROOT]/foo/b)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn dont_require_submodules_are_checked_out() {
    let p = project().build();
    let git1 = git::new("dep1", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
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
    let url = git2.root().to_url().to_string();
    git::add_submodule(&repo, &url, Path::new("a/submodule"));
    git::commit(&repo);

    git2::Repository::init(&p.root()).unwrap();
    let url = git1.root().to_url().to_string();
    let dst = paths::home().join("foo");
    git2::Repository::clone(&url, &dst).unwrap();

    git1.cargo("check -v").cwd(&dst).run();
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
                    [package]
                    name = "a"
                    version = "0.5.0"
                    edition = "2015"
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
                    edition = "2015"
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
                    edition = "2015"
                    authors = []

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                a.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/a`
[LOCKING] 1 package to latest compatible version
[CHECKING] a v0.5.0 ([ROOTURL]/a#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    edition = "2015"
                    authors = []

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                a.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/a`
[LOCKING] 1 package to latest compatible version
[CHECKING] a v0.5.0 ([ROOTURL]/a#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    edition = "2015"
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

    p.cargo("check").run();

    assert!(paths::home().join(".cargo/git/CACHEDIR.TAG").is_file());

    p.change_file(
        "a/Cargo.toml",
        &format!(
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                git = {{ git = '{}' }}
            "#,
            git.url()
        ),
    );

    p.cargo("check").run();
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
                    edition = "2015"
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
    p.cargo("check -v").run();
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
                edition = "2015"
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

    p.cargo("check").run();
    p.change_file("ignored.txt", "Trigger rebuild.");
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.5.0 ([ROOT]/foo): the precalculated components changed
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("package --list --allow-dirty")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
ignored.txt
src/lib.rs

"#]])
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
                    [package]

                    name = "dep1"
                    version = "0.5.0"
                    edition = "2015"
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
                    [package]

                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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

    project
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] duplicate key `categories` in table `package`
 --> ../home/.cargo/git/checkouts/dep1-[HASH]/[..]/Cargo.toml:9:21
  |
9 |                     categories = ["algorithms"]
  |                     ^
  |
[ERROR] failed to get `dep1` as a dependency of package `foo v0.5.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  Unable to update [ROOTURL]/dep1

"#]])
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
    let url = git_project2.root().to_url().to_string();
    git::add_submodule(&repo, &url, Path::new("src"));
    git::commit(&repo);
    drop(repo);

    let project = project
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
  failed to update submodule `src`
...
  failed to update submodule `bar`
...
"#]])
        .run();
    project
        .cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
  failed to update submodule `src`
...
  failed to update submodule `bar`
...
"#]])
        .run();

    done.store(true, Ordering::SeqCst);
    drop(TcpStream::connect(&addr));
    t.join().unwrap();
}

#[cargo_test(requires = "git")]
fn use_the_cli() {
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            "
                [net]
                git-fetch-with-cli = true
            ",
        )
        .build();

    let stderr = str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[RUNNING] `git fetch --no-tags --verbose --force --update-head-ok [..][ROOTURL]/dep1[..] [..]+HEAD:refs/remotes/origin/HEAD[..]`
From [ROOTURL]/dep1
 * [new ref] [..] -> origin/HEAD[..]
[LOCKING] 1 package to latest compatible version
[CHECKING] dep1 v0.5.0 ([ROOTURL]/dep1#[..])
[RUNNING] `rustc --crate-name dep1 [..]`
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]];

    project
        .cargo("check -v")
        .env("LC_ALL", "C")
        .with_stderr_data(stderr)
        .run();
    assert!(paths::home().join(".cargo/git/CACHEDIR.TAG").is_file());
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
                    [package]
                    name = "fo"
                    version = "0.5.0"
                    edition = "2015"
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

    p.cargo("check").run();
}

#[cargo_test(requires = "git")]
fn git_with_cli_force() {
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
                [package]
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
            ".cargo/config.toml",
            "
            [net]
            git-fetch-with-cli = true
            ",
        )
        .build();
    p.cargo("build").run();
    p.rename_run("foo", "foo1")
        .with_stdout_data(str![[r#"
one

"#]])
        .run();

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
    p.rename_run("foo", "foo2")
        .with_stdout_data(str![[r#"
two

"#]])
        .run();
}

#[cargo_test(requires = "git")]
fn git_fetch_cli_env_clean() {
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
                    edition = "2015"
                    [dependencies]
                    dep1 = {{ git = '{}' }}
                    "#,
                    git_dep.url()
                ),
            )
            .file("src/lib.rs", "pub extern crate dep1;")
            .file(
                ".cargo/config.toml",
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

    let url = git_project2.root().to_url().to_string();
    git::add_submodule(&repo, &url, Path::new("src"));

    // Submodule added, but not committed.
    git_project
        .cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

.gitmodules

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
        .run();

    git::commit(&repo);
    git_project.cargo("package --no-verify").run();

    // Modify file, check for warning.
    git_project.change_file("src/lib.rs", "");
    git_project
        .cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

src/lib.rs

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
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
    let url = git_project3.root().to_url().to_string();
    git::add_submodule(&sub_repo, &url, Path::new("bar"));
    git_project
        .cargo("package --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

src/.gitmodules

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
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
        .with_stderr_data(str![[r#"
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

src/bar/new_file.rs

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "pub fn foo() { dep1::bar() }")
        .build();

    project
        .cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[CHECKING] dep1 v0.5.0 ([ROOTURL]/dep1#[..])
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"

                    [dependencies]
                    dep1 = {{ git = '{}', branch = 'master' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    project.cargo("check").run();
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
        .cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ADDING] dep1 v0.5.0 ([ROOTURL]/dep1?branch=master#[..])
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"

                    [dependencies]
                    dep1 = {{ git = '{}', branch = 'master' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    let output = project.cargo("vendor").run();
    project.change_file(
        ".cargo/config.toml",
        str::from_utf8(&output.stdout).unwrap(),
    );
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
    project.cargo("check").run();
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
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
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
                    [package]
                    name = "a"
                    version = "0.5.0"
                    edition = "2015"
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
        .cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 3 packages to latest compatible versions
[CHECKING] dep1 v0.5.0 ([ROOTURL]/dep1#[..])
[CHECKING] dep1 v0.5.0 ([ROOTURL]/dep1?branch=master#[..])
[CHECKING] a v0.5.0 ([ROOT]/foo/a)
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"

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
                  "id": "__BAR_SOURCE__#1.0.0",
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
                  "rust_version": null,
                  "homepage": null,
                  "documentation": null,
                  "edition": "2015",
                  "links": null
                },
                {
                  "name": "foo",
                  "version": "0.1.0",
                  "id": "[..]foo#0.1.0",
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
                  "rust_version": null,
                  "homepage": null,
                  "documentation": null,
                  "edition": "2015",
                  "links": null
                }
              ],
              "workspace_members": [
                "[..]foo#0.1.0"
              ],
              "workspace_default_members": [
                "[..]foo#0.1.0"
              ],
              "resolve": {
                "nodes": [
                  {
                    "id": "__BAR_SOURCE__#1.0.0",
                    "dependencies": [],
                    "deps": [],
                    "features": []
                  },
                  {
                    "id": "[..]foo#0.1.0",
                    "dependencies": [
                      "__BAR_SOURCE__#1.0.0"
                    ],
                    "deps": [
                      {
                        "name": "bar",
                        "pkg": "__BAR_SOURCE__#1.0.0",
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
                "root": "[..]foo#0.1.0"
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

    let bar_source = "git+[ROOTURL]/bar?branch=master";
    p.cargo("metadata")
        .with_stdout_data(&metadata(&bar_source).is_json())
        .run();

    // Conversely, remove branch="master" from Cargo.toml, but use a new Cargo.lock that has ?branch=master.
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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
    let bar_source = "git+[ROOTURL]/bar";
    p.cargo("metadata")
        .with_stdout_data(&metadata(&bar_source).is_json())
        .run();
}

#[cargo_test]
fn git_with_force_push() {
    // Checks that cargo can handle force-pushes to git repos.
    // This works by having a git dependency that is updated with an amend
    // commit, and tries with various forms (default branch, branch, rev,
    // tag).
    let main = |text| format!(r#"pub fn f() {{ println!("{}"); }}"#, text);
    let (git_project, repo) = git::new_repo("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", &main("one"))
    });
    let manifest = |extra| {
        format!(
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2018"

                [dependencies]
                dep1 = {{ git = "{}"{} }}
            "#,
            git_project.url(),
            extra
        )
    };
    let p = project()
        .file("Cargo.toml", &manifest(""))
        .file("src/main.rs", "fn main() { dep1::f(); }")
        .build();
    // Download the original and make sure it is OK.
    p.cargo("build").run();
    p.rename_run("foo", "foo1")
        .with_stdout_data(str![[r#"
one

"#]])
        .run();

    let find_head = || t!(t!(repo.head()).peel_to_commit());

    let amend_commit = |text| {
        // commit --amend a change that will require a force fetch.
        git_project.change_file("src/lib.rs", &main(text));
        git::add(&repo);
        let commit = find_head();
        let tree_id = t!(t!(repo.index()).write_tree());
        t!(commit.amend(
            Some("HEAD"),
            None,
            None,
            None,
            None,
            Some(&t!(repo.find_tree(tree_id)))
        ));
    };

    let mut rename_annoyance = 1;

    let mut verify = |text| {
        // Perform the fetch.
        p.cargo("update").run();
        p.cargo("build").run();
        rename_annoyance += 1;
        p.rename_run("foo", &format!("foo{}", rename_annoyance))
            .with_stdout_data(text)
            .run();
    };

    amend_commit("two");
    verify(str![[r#"
two

"#]]);

    // Try with a rev.
    let head1 = find_head().id().to_string();
    let extra = format!(", rev = \"{}\"", head1);
    p.change_file("Cargo.toml", &manifest(&extra));
    verify(str![[r#"
two

"#]]);
    amend_commit("three");
    let head2 = find_head().id().to_string();
    assert_ne!(&head1, &head2);
    let extra = format!(", rev = \"{}\"", head2);
    p.change_file("Cargo.toml", &manifest(&extra));
    verify(str![[r#"
three

"#]]);

    // Try with a tag.
    git::tag(&repo, "my-tag");
    p.change_file("Cargo.toml", &manifest(", tag = \"my-tag\""));
    verify(str![[r#"
three

"#]]);
    amend_commit("tag-three");
    let head = t!(t!(repo.head()).peel(git2::ObjectType::Commit));
    t!(repo.tag("my-tag", &head, &t!(repo.signature()), "move tag", true));
    verify(str![[r#"
tag-three

"#]]);

    // Try with a branch.
    let br = t!(repo.branch("awesome-stuff", &find_head(), false));
    t!(repo.checkout_tree(&t!(br.get().peel(git2::ObjectType::Tree)), None));
    t!(repo.set_head("refs/heads/awesome-stuff"));
    git_project.change_file("src/lib.rs", &main("awesome-three"));
    git::add(&repo);
    git::commit(&repo);
    p.change_file("Cargo.toml", &manifest(", branch = \"awesome-stuff\""));
    verify(str![[r#"
awesome-three

"#]]);
    amend_commit("awesome-four");
    verify(str![[r#"
awesome-four

"#]]);
}

#[cargo_test]
fn corrupted_checkout() {
    // Test what happens if the checkout is corrupted somehow.
    _corrupted_checkout(false);
}

#[cargo_test]
fn corrupted_checkout_with_cli() {
    // Test what happens if the checkout is corrupted somehow with git cli.
    _corrupted_checkout(true);
}

fn _corrupted_checkout(with_cli: bool) {
    let (git_project, repository) = git::new_repo("dep1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep1", "0.5.0"))
            .file("src/lib.rs", "")
    });

    let project2 = git::new("dep2", |project| {
        project.no_manifest().file("README.md", "")
    });
    let url = project2.root().to_url().to_string();
    add_submodule(&repository, &url, Path::new("dep2"));
    git::commit(&repository);
    drop(repository);

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"
                    edition = "2015"

                    [dependencies]
                    dep1 = {{ git = "{}" }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch").run();

    let mut dep1_co_paths = t!(glob::glob(
        paths::home()
            .join(".cargo/git/checkouts/dep1-*/*")
            .to_str()
            .unwrap()
    ));
    let dep1_co_path = dep1_co_paths.next().unwrap().unwrap();
    let dep1_ok = dep1_co_path.join(".cargo-ok");
    let dep1_manifest = dep1_co_path.join("Cargo.toml");
    let dep2_readme = dep1_co_path.join("dep2/README.md");

    // Deleting this file simulates an interrupted checkout.
    t!(fs::remove_file(&dep1_ok));
    t!(fs::remove_file(&dep1_manifest));
    t!(fs::remove_file(&dep2_readme));

    // This should refresh the checkout.
    let mut e = p.cargo("fetch");
    if with_cli {
        e.env("CARGO_NET_GIT_FETCH_WITH_CLI", "true");
    }
    e.run();
    assert!(dep1_ok.exists());
    assert!(dep1_manifest.exists());
    assert!(dep2_readme.exists());
}

#[cargo_test]
fn cleans_temp_pack_files() {
    // Checks that cargo removes temp files left by libgit2 when it is
    // interrupted (see clean_repo_temp_files).
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch").run();
    // Simulate what happens when libgit2 is interrupted while indexing a pack file.
    let tmp_path = super::git_gc::find_index().join(".git/objects/pack/pack_git2_91ab40da04fdc2e7");
    fs::write(&tmp_path, "test").unwrap();
    let mut perms = fs::metadata(&tmp_path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&tmp_path, perms).unwrap();

    // Trigger an index update.
    p.cargo("generate-lockfile").run();
    assert!(!tmp_path.exists());
}

#[cargo_test]
fn different_user_relative_submodules() {
    let user1_git_project = git::new("user1/dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", "")
    });

    let user2_git_project = git::new("user2/dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", "")
    });
    let _user2_git_project2 = git::new("user2/dep2", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", "")
    });

    let user2_repo = git2::Repository::open(&user2_git_project.root()).unwrap();
    let url = "../dep2";
    git::add_submodule(&user2_repo, url, Path::new("dep2"));
    git::commit(&user2_repo);

    let user1_repo = git2::Repository::open(&user1_git_project.root()).unwrap();
    let url = user2_git_project.url();
    git::add_submodule(&user1_repo, url.as_str(), Path::new("user2/dep1"));
    git::commit(&user1_repo);

    let project = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo" 
                    version = "0.5.0"
                    edition = "2015"

                    [dependencies.dep1]
                    git = '{}'
                "#,
                user1_git_project.url()
            ),
        )
        .file("src/main.rs", &main_file(r#""hello""#, &[]))
        .build();

    project
        .cargo("build")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/user1/dep1`
[UPDATING] git submodule `[ROOTURL]/user2/dep1`
[UPDATING] git submodule `[ROOTURL]/user2/dep2`
[LOCKING] 1 package to latest compatible version
[COMPILING] dep1 v0.5.0 ([ROOTURL]/user1/dep1#[..])
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert!(project.bin("foo").is_file());
}

#[cargo_test]
fn git_worktree_with_original_repo_renamed() {
    let project = project().build();
    let git_project = git::new("foo", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []
                    license = "MIR OR Apache-2.0"
                    description = "A test!"
                    homepage = "https://example.org"
                    documentation = ""
                    repository = "https://example.org"
                    readme = "./README.md"
                "#,
            )
            .file("src/lib.rs", "")
            .file("README.md", "")
    });

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let repo_root = repo.workdir().unwrap().parent().unwrap();
    let opts = git2::WorktreeAddOptions::new();
    let _ = repo
        .worktree("bar", &repo_root.join("bar"), Some(&opts))
        .unwrap();

    // Rename the original repository
    let new = repo_root.join("foo2");
    fs::rename(&git_project.root(), &new).unwrap();

    project
        .cargo("package --list")
        .cwd(&new)
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
README.md
src/lib.rs

"#]])
        .run();

    project
        .cargo("check")
        .cwd(&new)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/foo2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(public_network_test, requires = "git")]
fn github_fastpath_error_message() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bitflags = { git = "https://github.com/rust-lang/bitflags.git", rev="11111b376b93484341c68fbca3ca110ae5cd2790" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .env("CARGO_NET_GIT_FETCH_WITH_CLI", "true")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `https://github.com/rust-lang/bitflags.git`
fatal: remote [ERROR] upload-pack: not our ref 11111b376b93484341c68fbca3ca110ae5cd2790
[ERROR] failed to get `bitflags` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bitflags`

Caused by:
  Unable to update https://github.com/rust-lang/bitflags.git?rev=11111b376b93484341c68fbca3ca110ae5cd2790

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bitflags-[HASH]

Caused by:
  revision 11111b376b93484341c68fbca3ca110ae5cd2790 not found

Caused by:
  process didn't exit successfully: `git fetch --no-tags --force --update-head-ok [..]

"#]])
        .run();
}

#[cargo_test(public_network_test)]
fn git_fetch_libgit2_error_message() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bitflags = { git = "https://github.com/rust-lang/bitflags.git", rev="11111b376b93484341c68fbca3ca110ae5cd2790" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `https://github.com/rust-lang/bitflags.git`
...
[ERROR] failed to get `bitflags` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bitflags`

Caused by:
  Unable to update https://github.com/rust-lang/bitflags.git?rev=11111b376b93484341c68fbca3ca110ae5cd2790

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/bitflags-[HASH]

Caused by:
  revision 11111b376b93484341c68fbca3ca110ae5cd2790 not found
...
"#]])
        .run();
}

#[cargo_test]
fn git_worktree_with_bare_original_repo() {
    let project = project().build();
    let git_project = git::new("foo", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                    [package]
                    name = "foo"
                    version = "0.5.0"
                    edition = "2015"
                    authors = []
                    license = "MIR OR Apache-2.0"
                    description = "A test!"
                    homepage = "https://example.org"
                    documentation = ""
                    repository = "https://example.org"
                    readme = "./README.md"
                "#,
            )
            .file("src/lib.rs", "")
            .file("README.md", "")
    });

    // Create a "bare" Git repository.
    // Keep the `.git` folder and delete the others.
    let repo = {
        let mut repo_builder = git2::build::RepoBuilder::new();
        repo_builder
            .bare(true)
            .clone_local(git2::build::CloneLocal::Local)
            .clone(
                git_project.root().to_url().as_str(),
                &paths::root().join("foo-bare"),
            )
            .unwrap()
    };
    assert!(repo.is_bare());
    let opts = git2::WorktreeAddOptions::new();
    let wt = repo
        .worktree("bar", &paths::root().join("bar"), Some(&opts))
        .unwrap();

    project
        .cargo("package --list")
        .cwd(wt.path())
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
README.md
src/lib.rs

"#]])
        .run();

    project
        .cargo("check")
        .cwd(wt.path())
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.5.0 ([ROOT]/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
#[cfg(unix)]
fn simple_with_fifo() {
    let git_project = git::new("foo", |project| {
        project
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
    });

    std::process::Command::new("mkfifo")
        .current_dir(git_project.root())
        .arg(git_project.root().join("blocks-when-read"))
        .status()
        .expect("a FIFO can be created");

    // Avoid actual blocking even in case of failure, assuming that what it lists here
    // would also be read eventually.
    git_project
        .cargo("package -l")
        .with_stdout_data(str![[r#"
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
src/main.rs

"#]])
        .run();
}
