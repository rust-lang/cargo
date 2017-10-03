extern crate cargo;
extern crate cargotest;
extern crate git2;
extern crate hamcrest;

use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;

use cargo::util::process;
use cargotest::sleep_ms;
use cargotest::support::paths::{self, CargoPathExt};
use cargotest::support::{git, project, execs, main_file, path2url};
use hamcrest::{assert_that,existing_file};

#[test]
fn cargo_compile_simple_git_dep() {
    let project = project("foo");
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]

                name = "dep1"
            "#)
            .file("src/dep1.rs", r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#)
    }).unwrap();

    let project = project
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'
        "#, git_project.url()))
        .file("src/main.rs", &main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("build"),
        execs()
        .with_stderr(&format!("[UPDATING] git repository `{}`\n\
                              [COMPILING] dep1 v0.5.0 ({}#[..])\n\
                              [COMPILING] foo v0.5.0 ({})\n\
                              [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
                             path2url(git_root.clone()),
                             path2url(git_root),
                             path2url(root))));

    assert_that(&project.bin("foo"), existing_file());

    assert_that(
      process(&project.bin("foo")),
      execs().with_stdout("hello world\n"));
}

#[test]
fn cargo_compile_git_dep_branch() {
    let project = project("foo");
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]

                name = "dep1"
            "#)
            .file("src/dep1.rs", r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#)
    }).unwrap();

    // Make a new branch based on the current HEAD commit
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    let head = repo.find_commit(head).unwrap();
    repo.branch("branchy", &head, true).unwrap();

    let project = project
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'
            branch = "branchy"

        "#, git_project.url()))
        .file("src/main.rs", &main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("build"),
        execs()
        .with_stderr(&format!("[UPDATING] git repository `{}`\n\
                              [COMPILING] dep1 v0.5.0 ({}?branch=branchy#[..])\n\
                              [COMPILING] foo v0.5.0 ({})\n\
                              [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
                             path2url(git_root.clone()),
                             path2url(git_root),
                             path2url(root))));

    assert_that(&project.bin("foo"), existing_file());

    assert_that(
      process(&project.bin("foo")),
      execs().with_stdout("hello world\n"));
}

#[test]
fn cargo_compile_git_dep_tag() {
    let project = project("foo");
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]

                name = "dep1"
            "#)
            .file("src/dep1.rs", r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#)
    }).unwrap();

    // Make a tag corresponding to the current HEAD
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    repo.tag("v0.1.0",
             &repo.find_object(head, None).unwrap(),
             &repo.signature().unwrap(),
             "make a new tag",
             false).unwrap();

    let project = project
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'
            tag = "v0.1.0"
        "#, git_project.url()))
        .file("src/main.rs", &main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("build"),
        execs()
        .with_stderr(&format!("[UPDATING] git repository `{}`\n\
                              [COMPILING] dep1 v0.5.0 ({}?tag=v0.1.0#[..])\n\
                              [COMPILING] foo v0.5.0 ({})\n\
                              [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
                             path2url(git_root.clone()),
                             path2url(git_root),
                             path2url(root))));

    assert_that(&project.bin("foo"), existing_file());

    assert_that(process(&project.bin("foo")),
                execs().with_stdout("hello world\n"));

    assert_that(project.cargo("build"),
                execs().with_status(0));
}

#[test]
fn cargo_compile_with_nested_paths() {
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [dependencies.dep2]

                version = "0.5.0"
                path = "vendor/dep2"

                [lib]

                name = "dep1"
            "#)
            .file("src/dep1.rs", r#"
                extern crate dep2;

                pub fn hello() -> &'static str {
                    dep2::hello()
                }
            "#)
            .file("vendor/dep2/Cargo.toml", r#"
                [project]

                name = "dep2"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]

                name = "dep2"
            "#)
            .file("vendor/dep2/src/dep2.rs", r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#)
    }).unwrap();

    let p = project("parent")
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "parent"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            version = "0.5.0"
            git = '{}'

            [[bin]]

            name = "parent"
        "#, git_project.url()))
        .file("src/parent.rs",
              &main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("parent"), existing_file());

    assert_that(process(&p.bin("parent")),
                execs().with_stdout("hello world\n"));
}

#[test]
fn cargo_compile_with_malformed_nested_paths() {
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]

                name = "dep1"
            "#)
            .file("src/dep1.rs", r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#)
            .file("vendor/dep2/Cargo.toml", r#"
                !INVALID!
            "#)
    }).unwrap();

    let p = project("parent")
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "parent"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            version = "0.5.0"
            git = '{}'

            [[bin]]

            name = "parent"
        "#, git_project.url()))
        .file("src/parent.rs",
              &main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("parent"), existing_file());

    assert_that(process(&p.bin("parent")),
                execs().with_stdout("hello world\n"));
}

#[test]
fn cargo_compile_with_meta_package() {
    let git_project = git::new("meta-dep", |project| {
        project
            .file("dep1/Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]

                name = "dep1"
            "#)
            .file("dep1/src/dep1.rs", r#"
                pub fn hello() -> &'static str {
                    "this is dep1"
                }
            "#)
            .file("dep2/Cargo.toml", r#"
                [project]

                name = "dep2"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]

                name = "dep2"
            "#)
            .file("dep2/src/dep2.rs", r#"
                pub fn hello() -> &'static str {
                    "this is dep2"
                }
            "#)
    }).unwrap();

    let p = project("parent")
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "parent"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            version = "0.5.0"
            git = '{}'

            [dependencies.dep2]

            version = "0.5.0"
            git = '{}'

            [[bin]]

            name = "parent"
        "#, git_project.url(), git_project.url()))
        .file("src/parent.rs",
              &main_file(r#""{} {}", dep1::hello(), dep2::hello()"#, &["dep1", "dep2"]));

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("parent"), existing_file());

    assert_that(process(&p.bin("parent")),
                execs().with_stdout("this is dep1 this is dep2\n"));
}

#[test]
fn cargo_compile_with_short_ssh_git() {
    let url = "git@github.com:a/dep";

    let project = project("project")
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep]

            git = "{}"

            [[bin]]

            name = "foo"
        "#, url))
        .file("src/foo.rs", &main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    assert_that(project.cargo_process("build"),
        execs()
        .with_stdout("")
        .with_stderr(&format!("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  invalid url `{}`: relative URL without a base
", url)));
}

#[test]
fn two_revs_same_deps() {
    let bar = git::new("meta-dep", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    }).unwrap();

    let repo = git2::Repository::open(&bar.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Commit the changes and make sure we trigger a recompile
    File::create(&bar.root().join("src/lib.rs")).unwrap().write_all(br#"
        pub fn bar() -> i32 { 2 }
    "#).unwrap();
    git::add(&repo);
    let rev2 = git::commit(&repo);

    let foo = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies.bar]
            git = '{}'
            rev = "{}"

            [dependencies.baz]
            path = "../baz"
        "#, bar.url(), rev1))
        .file("src/main.rs", r#"
            extern crate bar;
            extern crate baz;

            fn main() {
                assert_eq!(bar::bar(), 1);
                assert_eq!(baz::baz(), 2);
            }
        "#);

    let baz = project("baz")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "baz"
            version = "0.0.0"
            authors = []

            [dependencies.bar]
            git = '{}'
            rev = "{}"
        "#, bar.url(), rev2))
        .file("src/lib.rs", r#"
            extern crate bar;
            pub fn baz() -> i32 { bar::bar() }
        "#);

    baz.build();

    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(0));
    assert_that(&foo.bin("foo"), existing_file());
    assert_that(foo.process(&foo.bin("foo")), execs().with_status(0));
}

#[test]
fn recompilation() {
    let git_project = git::new("bar", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]
                name = "bar"
            "#)
            .file("src/bar.rs", r#"
                pub fn bar() {}
            "#)
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            git = '{}'
        "#, git_project.url()))
        .file("src/main.rs",
              &main_file(r#""{:?}", bar::bar()"#, &["bar"]));

    // First time around we should compile both foo and bar
    assert_that(p.cargo_process("build"),
                execs().with_stderr(&format!("[UPDATING] git repository `{}`\n\
                                             [COMPILING] bar v0.5.0 ({}#[..])\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            git_project.url(),
                                            git_project.url(),
                                            p.url())));

    // Don't recompile the second time
    assert_that(p.cargo("build"),
                execs().with_stdout(""));

    // Modify a file manually, shouldn't trigger a recompile
    File::create(&git_project.root().join("src/bar.rs")).unwrap().write_all(br#"
        pub fn bar() { println!("hello!"); }
    "#).unwrap();

    assert_that(p.cargo("build"),
                execs().with_stdout(""));

    assert_that(p.cargo("update"),
                execs().with_stderr(&format!("[UPDATING] git repository `{}`",
                                            git_project.url())));

    assert_that(p.cargo("build"),
                execs().with_stdout(""));

    // Commit the changes and make sure we don't trigger a recompile because the
    // lockfile says not to change
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);

    println!("compile after commit");
    assert_that(p.cargo("build"),
                execs().with_stdout(""));
    p.root().move_into_the_past();

    // Update the dependency and carry on!
    assert_that(p.cargo("update"),
                execs().with_stderr(&format!("[UPDATING] git repository `{}`\n\
                                              [UPDATING] bar v0.5.0 ([..]) -> #[..]\n\
                                             ",
                                            git_project.url())));
    println!("going for the last compile");
    assert_that(p.cargo("build"),
                execs().with_stderr(&format!("[COMPILING] bar v0.5.0 ({}#[..])\n\
                                             [COMPILING] foo v0.5.0 ({})\n\
                                             [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                             in [..]\n",
                                            git_project.url(),
                                            p.url())));

    // Make sure clean only cleans one dep
    assert_that(p.cargo("clean")
                 .arg("-p").arg("foo"),
                execs().with_stdout(""));
    assert_that(p.cargo("build"),
                execs().with_stderr(&format!("[COMPILING] foo v0.5.0 ({})\n\
                                              [FINISHED] dev [unoptimized + debuginfo] target(s) \
                                              in [..]\n",
                                            p.url())));
}

#[test]
fn update_with_shared_deps() {
    let git_project = git::new("bar", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]

                [lib]
                name = "bar"
            "#)
            .file("src/bar.rs", r#"
                pub fn bar() {}
            "#)
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]
            path = "dep1"
            [dependencies.dep2]
            path = "dep2"
        "#)
        .file("src/main.rs", r#"
            #[allow(unused_extern_crates)]
            extern crate dep1;
            #[allow(unused_extern_crates)]
            extern crate dep2;
            fn main() {}
        "#)
        .file("dep1/Cargo.toml", &format!(r#"
            [package]
            name = "dep1"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            version = "0.5.0"
            git = '{}'
        "#, git_project.url()))
        .file("dep1/src/lib.rs", "")
        .file("dep2/Cargo.toml", &format!(r#"
            [package]
            name = "dep2"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            version = "0.5.0"
            git = '{}'
        "#, git_project.url()))
        .file("dep2/src/lib.rs", "");

    // First time around we should compile both foo and bar
    assert_that(p.cargo_process("build"),
                execs().with_stderr(&format!("\
[UPDATING] git repository `{git}`
[COMPILING] bar v0.5.0 ({git}#[..])
[COMPILING] [..] v0.5.0 ([..])
[COMPILING] [..] v0.5.0 ([..])
[COMPILING] foo v0.5.0 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
git = git_project.url(), dir = p.url())));

    // Modify a file manually, and commit it
    File::create(&git_project.root().join("src/bar.rs")).unwrap().write_all(br#"
        pub fn bar() { println!("hello!"); }
    "#).unwrap();
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let old_head = repo.head().unwrap().target().unwrap();
    git::add(&repo);
    git::commit(&repo);

    sleep_ms(1000);

    // By default, not transitive updates
    println!("dep1 update");
    assert_that(p.cargo("update")
                 .arg("-p").arg("dep1"),
                execs().with_stdout(""));

    // Don't do anything bad on a weird --precise argument
    println!("bar bad precise update");
    assert_that(p.cargo("update")
                 .arg("-p").arg("bar")
                 .arg("--precise").arg("0.1.2"),
                execs().with_status(101).with_stderr("\
[UPDATING] git repository [..]
[ERROR] Unable to update [..]

To learn more, run the command again with --verbose.
"));

    // Specifying a precise rev to the old rev shouldn't actually update
    // anything because we already have the rev in the db.
    println!("bar precise update");
    assert_that(p.cargo("update")
                 .arg("-p").arg("bar")
                 .arg("--precise").arg(&old_head.to_string()),
                execs().with_stdout(""));

    // Updating aggressively should, however, update the repo.
    println!("dep1 aggressive update");
    assert_that(p.cargo("update")
                 .arg("-p").arg("dep1")
                 .arg("--aggressive"),
                execs().with_stderr(&format!("[UPDATING] git repository `{}`\n\
                                              [UPDATING] bar v0.5.0 ([..]) -> #[..]\n\
                                             ", git_project.url())));

    // Make sure we still only compile one version of the git repo
    println!("build");
    assert_that(p.cargo("build"),
                execs().with_stderr(&format!("\
[COMPILING] bar v0.5.0 ({git}#[..])
[COMPILING] [..] v0.5.0 ({dir}[..]dep[..])
[COMPILING] [..] v0.5.0 ({dir}[..]dep[..])
[COMPILING] foo v0.5.0 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
                    git = git_project.url(), dir = p.url())));

    // We should be able to update transitive deps
    assert_that(p.cargo("update").arg("-p").arg("bar"),
                execs().with_stderr(&format!("[UPDATING] git repository `{}`",
                                            git_project.url())));
}

#[test]
fn dep_with_submodule() {
    let project = project("foo");
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
    }).unwrap();
    let git_project2 = git::new("dep2", |project| {
        project.file("lib.rs", "pub fn dep() {}")
    }).unwrap();

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let url = path2url(git_project2.root()).to_string();
    git::add_submodule(&repo, &url, Path::new("src"));
    git::commit(&repo);

    let project = project
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'
        "#, git_project.url()))
        .file("src/lib.rs", "
            extern crate dep1;
            pub fn foo() { dep1::dep() }
        ");

    assert_that(project.cargo_process("build"),
                execs().with_stderr("\
[UPDATING] git repository [..]
[COMPILING] dep1 [..]
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n").with_status(0));
}

#[test]
fn dep_with_bad_submodule() {
    let project = project("foo");
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
    }).unwrap();
    let git_project2 = git::new("dep2", |project| {
        project.file("lib.rs", "pub fn dep() {}")
    }).unwrap();

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let url = path2url(git_project2.root()).to_string();
    git::add_submodule(&repo, &url, Path::new("src"));
    git::commit(&repo);

    // now amend the first commit on git_project2 to make submodule ref point to not-found
    // commit
    let repo = git2::Repository::open(&git_project2.root()).unwrap();
    let original_submodule_ref = repo.refname_to_id("refs/heads/master").unwrap();
    let commit = repo.find_commit(original_submodule_ref).unwrap();
    commit.amend(
        Some("refs/heads/master"),
        None,
        None,
        None,
        Some("something something"),
        None).unwrap();

    let project = project
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'
        "#, git_project.url()))
        .file("src/lib.rs", "
            extern crate dep1;
            pub fn foo() { dep1::dep() }
        ");

    let expected = format!("\
[UPDATING] git repository [..]
[ERROR] failed to load source for a dependency on `dep1`

Caused by:
  Unable to update {}

Caused by:
  failed to update submodule `src`

To learn more, run the command again with --verbose.\n", path2url(git_project.root()));

    assert_that(project.cargo_process("build"),
                execs().with_stderr(expected).with_status(101));
}

#[test]
fn two_deps_only_update_one() {
    let project = project("foo");
    let git1 = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
            .file("src/lib.rs", "")
    }).unwrap();
    let git2 = git::new("dep2", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep2"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
            .file("src/lib.rs", "")
    }).unwrap();

    let project = project
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]
            git = '{}'
            [dependencies.dep2]
            git = '{}'
        "#, git1.url(), git2.url()))
        .file("src/main.rs", "fn main() {}");

    assert_that(project.cargo_process("build"),
        execs()
        .with_stderr(&format!("[UPDATING] git repository `[..]`\n\
                              [UPDATING] git repository `[..]`\n\
                              [COMPILING] [..] v0.5.0 ([..])\n\
                              [COMPILING] [..] v0.5.0 ([..])\n\
                              [COMPILING] foo v0.5.0 ({})\n\
                              [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
                             project.url())));

    File::create(&git1.root().join("src/lib.rs")).unwrap().write_all(br#"
        pub fn foo() {}
    "#).unwrap();
    let repo = git2::Repository::open(&git1.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);

    assert_that(project.cargo("update")
                       .arg("-p").arg("dep1"),
        execs()
        .with_stderr(&format!("[UPDATING] git repository `{}`\n\
                               [UPDATING] dep1 v0.5.0 ([..]) -> #[..]\n\
                              ", git1.url())));
}

#[test]
fn stale_cached_version() {
    let bar = git::new("meta-dep", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    }).unwrap();

    // Update the git database in the cache with the current state of the git
    // repo
    let foo = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies.bar]
            git = '{}'
        "#, bar.url()))
        .file("src/main.rs", r#"
            extern crate bar;

            fn main() { assert_eq!(bar::bar(), 1) }
        "#);

    assert_that(foo.cargo_process("build"), execs().with_status(0));
    assert_that(foo.process(&foo.bin("foo")), execs().with_status(0));

    // Update the repo, and simulate someone else updating the lockfile and then
    // us pulling it down.
    File::create(&bar.root().join("src/lib.rs")).unwrap().write_all(br#"
        pub fn bar() -> i32 { 1 + 0 }
    "#).unwrap();
    let repo = git2::Repository::open(&bar.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);

    sleep_ms(1000);

    let rev = repo.revparse_single("HEAD").unwrap().id();

    File::create(&foo.root().join("Cargo.lock")).unwrap().write_all(format!(r#"
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
    "#, url = bar.url(), hash = rev).as_bytes()).unwrap();

    // Now build!
    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[UPDATING] git repository `{bar}`
[COMPILING] bar v0.0.0 ({bar}#[..])
[COMPILING] foo v0.0.0 ({foo})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", bar = bar.url(), foo = foo.url())));
    assert_that(foo.process(&foo.bin("foo")), execs().with_status(0));
}

#[test]
fn dep_with_changed_submodule() {
    let project = project("foo");
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
    }).unwrap();

    let git_project2 = git::new("dep2", |project| {
        project
            .file("lib.rs", "pub fn dep() -> &'static str { \"project2\" }")
    }).unwrap();

    let git_project3 = git::new("dep3", |project| {
        project
            .file("lib.rs", "pub fn dep() -> &'static str { \"project3\" }")
    }).unwrap();

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let mut sub = git::add_submodule(&repo, &git_project2.url().to_string(),
                                     Path::new("src"));
    git::commit(&repo);

    let project = project
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            [dependencies.dep1]
            git = '{}'
        "#, git_project.url()))
        .file("src/main.rs", "
            extern crate dep1;
            pub fn main() { println!(\"{}\", dep1::dep()) }
        ");

    println!("first run");
    assert_that(project.cargo_process("run"), execs()
                .with_stderr("[UPDATING] git repository `[..]`\n\
                                      [COMPILING] dep1 v0.5.0 ([..])\n\
                                      [COMPILING] foo v0.5.0 ([..])\n\
                                      [FINISHED] dev [unoptimized + debuginfo] target(s) in \
                                      [..]\n\
                                      [RUNNING] `target[/]debug[/]foo[EXE]`\n")
                .with_stdout("project2\n")
                .with_status(0));

    File::create(&git_project.root().join(".gitmodules")).unwrap()
         .write_all(format!("[submodule \"src\"]\n\tpath = src\n\turl={}",
                            git_project3.url()).as_bytes()).unwrap();

    // Sync the submodule and reset it to the new remote.
    sub.sync().unwrap();
    {
        let subrepo = sub.open().unwrap();
        subrepo.remote_add_fetch("origin",
                                 "refs/heads/*:refs/heads/*").unwrap();
        subrepo.remote_set_url("origin",
                               &git_project3.url().to_string()).unwrap();
        let mut origin = subrepo.find_remote("origin").unwrap();
        origin.fetch(&[], None, None).unwrap();
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
    assert_that(project.cargo("update").arg("-v"),
                execs()
                .with_stderr("")
                .with_stderr(&format!("[UPDATING] git repository `{}`\n\
                                       [UPDATING] dep1 v0.5.0 ([..]) -> #[..]\n\
                                      ", git_project.url())));

    println!("last run");
    assert_that(project.cargo("run"), execs()
                .with_stderr("[COMPILING] dep1 v0.5.0 ([..])\n\
                                      [COMPILING] foo v0.5.0 ([..])\n\
                                      [FINISHED] dev [unoptimized + debuginfo] target(s) in \
                                      [..]\n\
                                      [RUNNING] `target[/]debug[/]foo[EXE]`\n")
                .with_stdout("project3\n")
                .with_status(0));
}

#[test]
fn dev_deps_with_testing() {
    let p2 = git::new("bar", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", r#"
            pub fn gimme() -> &'static str { "zoidberg" }
        "#)
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dev-dependencies.bar]
            version = "0.5.0"
            git = '{}'
        "#, p2.url()))
        .file("src/main.rs", r#"
            fn main() {}

            #[cfg(test)]
            mod tests {
                extern crate bar;
                #[test] fn foo() { bar::gimme(); }
            }
        "#);

    // Generate a lockfile which did not use `bar` to compile, but had to update
    // `bar` to generate the lockfile
    assert_that(p.cargo_process("build"),
        execs().with_stderr(&format!("\
[UPDATING] git repository `{bar}`
[COMPILING] foo v0.5.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", url = p.url(), bar = p2.url())));

    // Make sure we use the previous resolution of `bar` instead of updating it
    // a second time.
    assert_that(p.cargo("test"),
                execs().with_stderr("\
[COMPILING] [..] v0.5.0 ([..])
[COMPILING] [..] v0.5.0 ([..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] target[/]debug[/]deps[/]foo-[..][EXE]")
                       .with_stdout_contains("test tests::foo ... ok"));
}

#[test]
fn git_build_cmd_freshness() {
    let foo = git::new("foo", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"
        "#)
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .file(".gitignore", "
            src/bar.rs
        ")
    }).unwrap();
    foo.root().move_into_the_past();

    sleep_ms(1000);

    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", url = foo.url())));

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stdout(""));

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    File::create(&foo.root().join("src/bar.rs")).unwrap();
    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stdout(""));
}

#[test]
fn git_name_not_always_needed() {
    let p2 = git::new("bar", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", r#"
            pub fn gimme() -> &'static str { "zoidberg" }
        "#)
    }).unwrap();

    let repo = git2::Repository::open(&p2.root()).unwrap();
    let mut cfg = repo.config().unwrap();
    let _ = cfg.remove("user.name");
    let _ = cfg.remove("user.email");

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dev-dependencies.bar]
            git = '{}'
        "#, p2.url()))
        .file("src/main.rs", "fn main() {}");

    // Generate a lockfile which did not use `bar` to compile, but had to update
    // `bar` to generate the lockfile
    assert_that(p.cargo_process("build"),
        execs().with_stderr(&format!("\
[UPDATING] git repository `{bar}`
[COMPILING] foo v0.5.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", url = p.url(), bar = p2.url())));
}

#[test]
fn git_repo_changing_no_rebuild() {
    let bar = git::new("bar", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    }).unwrap();

    // Lock p1 to the first rev in the git repo
    let p1 = project("p1")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "p1"
            version = "0.5.0"
            authors = []
            build = 'build.rs'
            [dependencies.bar]
            git = '{}'
        "#, bar.url()))
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", "fn main() {}");
    p1.build();
    p1.root().move_into_the_past();
    assert_that(p1.cargo("build"),
                execs().with_stderr(&format!("\
[UPDATING] git repository `{bar}`
[COMPILING] [..]
[COMPILING] [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", bar = bar.url())));

    // Make a commit to lock p2 to a different rev
    File::create(&bar.root().join("src/lib.rs")).unwrap().write_all(br#"
        pub fn bar() -> i32 { 2 }
    "#).unwrap();
    let repo = git2::Repository::open(&bar.root()).unwrap();
    git::add(&repo);
    git::commit(&repo);

    // Lock p2 to the second rev
    let p2 = project("p2")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "p2"
            version = "0.5.0"
            authors = []
            [dependencies.bar]
            git = '{}'
        "#, bar.url()))
        .file("src/main.rs", "fn main() {}");
    assert_that(p2.cargo_process("build"),
                execs().with_stderr(&format!("\
[UPDATING] git repository `{bar}`
[COMPILING] [..]
[COMPILING] [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", bar = bar.url())));

    // And now for the real test! Make sure that p1 doesn't get rebuilt
    // even though the git repo has changed.
    assert_that(p1.cargo("build"),
                execs().with_stdout(""));
}

#[test]
fn git_dep_build_cmd() {
    let p = git::new("foo", |project| {
        project.file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            path = "bar"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"

            [lib]
            name = "bar"
            path = "src/bar.rs"
        "#)
        .file("bar/src/bar.rs.in", r#"
            pub fn gimme() -> i32 { 0 }
        "#)
        .file("bar/build.rs", r#"
            use std::fs;
            fn main() {
                fs::copy("src/bar.rs.in", "src/bar.rs").unwrap();
            }
        "#)
    }).unwrap();

    p.root().join("bar").move_into_the_past();

    assert_that(p.cargo("build"),
                execs().with_status(0));

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("0\n"));

    // Touching bar.rs.in should cause the `build` command to run again.
    fs::File::create(&p.root().join("bar/src/bar.rs.in")).unwrap()
             .write_all(b"pub fn gimme() -> i32 { 1 }").unwrap();

    assert_that(p.cargo("build"),
                execs().with_status(0));

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("1\n"));
}

#[test]
fn fetch_downloads() {
    let bar = git::new("bar", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    }).unwrap();

    let p = project("p1")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "p1"
            version = "0.5.0"
            authors = []
            [dependencies.bar]
            git = '{}'
        "#, bar.url()))
        .file("src/main.rs", "fn main() {}");
    assert_that(p.cargo_process("fetch"),
                execs().with_status(0).with_stderr(&format!("\
[UPDATING] git repository `{url}`
", url = bar.url())));

    assert_that(p.cargo("fetch"),
                execs().with_status(0).with_stdout(""));
}

#[test]
fn warnings_in_git_dep() {
    let bar = git::new("bar", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "fn unused() {}")
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            [dependencies.bar]
            git = '{}'
        "#, bar.url()))
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
        execs()
        .with_stderr(&format!("[UPDATING] git repository `{}`\n\
                              [COMPILING] bar v0.5.0 ({}#[..])\n\
                              [COMPILING] foo v0.5.0 ({})\n\
                              [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
                             bar.url(),
                             bar.url(),
                             p.url())));
}

#[test]
fn update_ambiguous() {
    let foo1 = git::new("foo1", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "")
    }).unwrap();
    let foo2 = git::new("foo2", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.6.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "")
    }).unwrap();
    let bar = git::new("bar", |project| {
        project.file("Cargo.toml", &format!(r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.foo]
            git = '{}'
        "#, foo2.url()))
        .file("src/lib.rs", "")
    }).unwrap();

    let p = project("project")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.foo]
            git = '{}'
            [dependencies.bar]
            git = '{}'
        "#, foo1.url(), bar.url()))
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("generate-lockfile"), execs().with_status(0));
    assert_that(p.cargo("update")
                 .arg("-p").arg("foo"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] There are multiple `foo` packages in your project, and the specification `foo` \
is ambiguous.
Please re-run this command with `-p <spec>` where `<spec>` is one of the \
following:
  foo:0.[..].0
  foo:0.[..].0
"));
}

#[test]
fn update_one_dep_in_repo_with_many_deps() {
    let foo = git::new("foo", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("a/src/lib.rs", "")
    }).unwrap();

    let p = project("project")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.foo]
            git = '{}'
            [dependencies.a]
            git = '{}'
        "#, foo.url(), foo.url()))
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("generate-lockfile"), execs().with_status(0));
    assert_that(p.cargo("update")
                 .arg("-p").arg("foo"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[UPDATING] git repository `{}`
", foo.url())));
}

#[test]
fn switch_deps_does_not_update_transitive() {
    let transitive = git::new("transitive", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "transitive"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "")
    }).unwrap();
    let dep1 = git::new("dep1", |project| {
        project.file("Cargo.toml", &format!(r#"
            [package]
            name = "dep"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.transitive]
            git = '{}'
        "#, transitive.url()))
        .file("src/lib.rs", "")
    }).unwrap();
    let dep2 = git::new("dep2", |project| {
        project.file("Cargo.toml", &format!(r#"
            [package]
            name = "dep"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.transitive]
            git = '{}'
        "#, transitive.url()))
        .file("src/lib.rs", "")
    }).unwrap();

    let p = project("project")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.dep]
            git = '{}'
        "#, dep1.url()))
        .file("src/main.rs", "fn main() {}");

    p.build();
    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[UPDATING] git repository `{}`
[UPDATING] git repository `{}`
[COMPILING] transitive [..]
[COMPILING] dep [..]
[COMPILING] project [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dep1.url(), transitive.url())));

    // Update the dependency to point to the second repository, but this
    // shouldn't update the transitive dependency which is the same.
    File::create(&p.root().join("Cargo.toml")).unwrap().write_all(format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.dep]
            git = '{}'
    "#, dep2.url()).as_bytes()).unwrap();

    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[UPDATING] git repository `{}`
[COMPILING] dep [..]
[COMPILING] project [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dep2.url())));
}

#[test]
fn update_one_source_updates_all_packages_in_that_git_source() {
    let dep = git::new("dep", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "dep"
            version = "0.5.0"
            authors = []

            [dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "")
    }).unwrap();

    let p = project("project")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.dep]
            git = '{}'
        "#, dep.url()))
        .file("src/main.rs", "fn main() {}");

    p.build();
    assert_that(p.cargo("build"),
                execs().with_status(0));

    let repo = git2::Repository::open(&dep.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Just be sure to change a file
    File::create(&dep.root().join("src/lib.rs")).unwrap().write_all(br#"
        pub fn bar() -> i32 { 2 }
    "#).unwrap();
    git::add(&repo);
    git::commit(&repo);

    assert_that(p.cargo("update").arg("-p").arg("dep"),
                execs().with_status(0));
    let mut lockfile = String::new();
    File::open(&p.root().join("Cargo.lock")).unwrap()
         .read_to_string(&mut lockfile).unwrap();
    assert!(!lockfile.contains(&rev1.to_string()),
            "{} in {}", rev1, lockfile);
}

#[test]
fn switch_sources() {
    let a1 = git::new("a1", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
    }).unwrap();
    let a2 = git::new("a2", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.5.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
    }).unwrap();

    let p = project("project")
        .file("Cargo.toml", r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.b]
            path = "b"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("b/Cargo.toml", &format!(r#"
            [project]
            name = "b"
            version = "0.5.0"
            authors = []
            [dependencies.a]
            git = '{}'
        "#, a1.url()))
        .file("b/src/lib.rs", "pub fn main() {}");

    p.build();
    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] git repository `file://[..]a1`
[COMPILING] a v0.5.0 ([..]a1#[..]
[COMPILING] b v0.5.0 ([..])
[COMPILING] project v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    File::create(&p.root().join("b/Cargo.toml")).unwrap().write_all(format!(r#"
        [project]
        name = "b"
        version = "0.5.0"
        authors = []
        [dependencies.a]
        git = '{}'
    "#, a2.url()).as_bytes()).unwrap();

    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] git repository `file://[..]a2`
[COMPILING] a v0.5.1 ([..]a2#[..]
[COMPILING] b v0.5.0 ([..])
[COMPILING] project v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn dont_require_submodules_are_checked_out() {
    let project = project("foo");
    let git1 = git::new("dep1", |p| {
        p.file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            build = "build.rs"
        "#)
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .file("a/foo", "")
    }).unwrap();
    let git2 = git::new("dep2", |p| p).unwrap();

    let repo = git2::Repository::open(&git1.root()).unwrap();
    let url = path2url(git2.root()).to_string();
    git::add_submodule(&repo, &url, Path::new("a/submodule"));
    git::commit(&repo);

    git2::Repository::init(&project.root()).unwrap();
    let url = path2url(git1.root()).to_string();
    let dst = paths::home().join("foo");
    git2::Repository::clone(&url, &dst).unwrap();

    assert_that(git1.cargo("build").arg("-v").cwd(&dst),
                execs().with_status(0));
}

#[test]
fn doctest_same_name() {
    let a2 = git::new("a2", |p| {
        p.file("Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn a2() {}")
    }).unwrap();

    let a1 = git::new("a1", |p| {
        p.file("Cargo.toml", &format!(r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
            [dependencies]
            a = {{ git = '{}' }}
        "#, a2.url()))
        .file("src/lib.rs", "extern crate a; pub fn a1() {}")
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = {{ git = '{}' }}
        "#, a1.url()))
        .file("src/lib.rs", r#"
            #[macro_use]
            extern crate a;
        "#);

    assert_that(p.cargo_process("test").arg("-v"),
                execs().with_status(0));
}

#[test]
fn lints_are_suppressed() {
    let a = git::new("a", |p| {
        p.file("Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "
            use std::option;
        ")
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = {{ git = '{}' }}
        "#, a.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] git repository `[..]`
[COMPILING] a v0.5.0 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn denied_lints_are_allowed() {
    let a = git::new("a", |p| {
        p.file("Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "
            #![deny(warnings)]
            use std::option;
        ")
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = {{ git = '{}' }}
        "#, a.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] git repository `[..]`
[COMPILING] a v0.5.0 ([..])
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn add_a_git_dep() {
    let git = git::new("git", |p| {
        p.file("Cargo.toml", r#"
            [project]
            name = "git"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = {{ path = 'a' }}
            git = {{ git = '{}' }}
        "#, git.url()))
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build"), execs().with_status(0));

    File::create(p.root().join("a/Cargo.toml")).unwrap().write_all(format!(r#"
        [package]
        name = "a"
        version = "0.0.1"
        authors = []

        [dependencies]
        git = {{ git = '{}' }}
    "#, git.url()).as_bytes()).unwrap();

    assert_that(p.cargo("build"), execs().with_status(0));
}

#[test]
fn two_at_rev_instead_of_tag() {
    let git = git::new("git", |p| {
        p.file("Cargo.toml", r#"
            [project]
            name = "git1"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [project]
            name = "git2"
            version = "0.5.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "")
    }).unwrap();

    // Make a tag corresponding to the current HEAD
    let repo = git2::Repository::open(&git.root()).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    repo.tag("v0.1.0",
             &repo.find_object(head, None).unwrap(),
             &repo.signature().unwrap(),
             "make a new tag",
             false).unwrap();

    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            git1 = {{ git = '{0}', rev = 'v0.1.0' }}
            git2 = {{ git = '{0}', rev = 'v0.1.0' }}
        "#, git.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("generate-lockfile"), execs().with_status(0));
    assert_that(p.cargo("build").arg("-v"), execs().with_status(0));
}

#[test]
#[ignore] // accesses crates.io
fn include_overrides_gitignore() {
    let p = git::new("reduction", |repo| {
        repo.file("Cargo.toml", r#"
            [package]
            name = "reduction"
            version = "0.5.0"
            authors = ["pnkfelix"]
            build = "tango-build.rs"
            include = ["src/lib.rs", "src/incl.rs", "src/mod.md", "tango-build.rs", "Cargo.toml"]

            [build-dependencies]
            filetime = "0.1"
        "#)
        .file(".gitignore", r#"
            target
            Cargo.lock
            # Below files represent generated code, thus not managed by `git`
            src/incl.rs
            src/not_incl.rs
        "#)
        .file("tango-build.rs", r#"
            extern crate filetime;
            use filetime::FileTime;
            use std::fs::{self, File};

            fn main() {
                // generate files, or bring their timestamps into sync.
                let source = "src/mod.md";

                let metadata = fs::metadata(source).unwrap();
                let mtime = FileTime::from_last_modification_time(&metadata);
                let atime = FileTime::from_last_access_time(&metadata);

                // sync time stamps for generated files with time stamp of source file.

                let files = ["src/not_incl.rs", "src/incl.rs"];
                for file in files.iter() {
                    File::create(file).unwrap();
                    filetime::set_file_times(file, atime, mtime).unwrap();
                }
            }
        "#)
        .file("src/lib.rs", r#"
            mod not_incl;
            mod incl;
        "#)
        .file("src/mod.md", r#"
            (The content of this file does not matter since we are not doing real codegen.)
        "#)
    }).unwrap();

    println!("build 1: all is new");
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] filetime [..]
[DOWNLOADING] libc [..]
[COMPILING] libc [..]
[RUNNING] `rustc --crate-name libc [..]`
[COMPILING] filetime [..]
[RUNNING] `rustc --crate-name filetime [..]`
[COMPILING] reduction [..]
[RUNNING] `rustc --crate-name build_script_tango_build tango-build.rs --crate-type bin [..]`
[RUNNING] `[..][/]build-script-tango-build`
[RUNNING] `rustc --crate-name reduction src[/]lib.rs --crate-type lib [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    println!("build 2: nothing changed; file timestamps reset by build script");
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[FRESH] libc [..]
[FRESH] filetime [..]
[FRESH] reduction [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    println!("build 3: touch `src/not_incl.rs`; expect build script *not* re-run");
    sleep_ms(1000);
    File::create(p.root().join("src").join("not_incl.rs")).unwrap();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[FRESH] libc [..]
[FRESH] filetime [..]
[COMPILING] reduction [..]
[RUNNING] `rustc --crate-name reduction src[/]lib.rs --crate-type lib [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    // This final case models the bug from rust-lang/cargo#4135: an
    // explicitly included file should cause a build-script re-run,
    // even if that same file is matched by `.gitignore`.
    println!("build 4: touch `src/incl.rs`; expect build script re-run");
    sleep_ms(1000);
    File::create(p.root().join("src").join("incl.rs")).unwrap();

    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[FRESH] libc [..]
[FRESH] filetime [..]
[COMPILING] reduction [..]
[RUNNING] `[..][/]build-script-tango-build`
[RUNNING] `rustc --crate-name reduction src[/]lib.rs --crate-type lib [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn invalid_git_dependency_manifest() {
    let project = project("foo");
    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [project]

                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
                categories = ["algorithms"]
                categories = ["algorithms"]

                [lib]

                name = "dep1"
            "#)
            .file("src/dep1.rs", r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#)
    }).unwrap();

    let project = project
        .file("Cargo.toml", &format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'
        "#, git_project.url()))
        .file("src/main.rs", &main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    let git_root = git_project.root();

    assert_that(project.cargo_process("build"),
        execs()
        .with_stderr(&format!("[UPDATING] git repository `{}`\n\
                              error: failed to load source for a dependency on `dep1`\n\
                              \n\
                              Caused by:\n  \
                              Unable to update {}\n\
                              \n\
                              Caused by:\n  \
                              failed to parse manifest at `[..]`\n\
                              \n\
                              Caused by:\n  \
                              could not parse input as TOML\n\
                              \n\
                              Caused by:\n  \
                              duplicate key: `categories` for key `project`",
                             path2url(git_root.clone()),
                             path2url(git_root),
                             )));
}
