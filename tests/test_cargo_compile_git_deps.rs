use std::io::fs;
use std::io::{timer, File};
use std::time::Duration;
use git2;

use support::{ProjectBuilder, project, execs, main_file};
use support::{cargo_dir, path2url};
use support::{COMPILING, UPDATING, RUNNING};
use support::paths::PathExt;
use hamcrest::{assert_that,existing_file};
use cargo;
use cargo::util::{ProcessError, process};


fn setup() {
}

fn git_repo(name: &str, callback: |ProjectBuilder| -> ProjectBuilder)
    -> Result<ProjectBuilder, ProcessError>
{
    let mut git_project = project(name);
    git_project = callback(git_project);
    git_project.build();

    let repo = git2::Repository::init(&git_project.root()).unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.email", "foo@bar.com").unwrap();
    cfg.set_str("user.name", "Foo Bar").unwrap();
    drop(cfg);
    add(&repo);
    commit(&repo);
    Ok(git_project)
}

fn add(repo: &git2::Repository) {
    // FIXME(libgit2/libgit2#2514): apparently add_all will add all submodules
    // as well, and then fail b/c they're a directory. As a stopgap, we just
    // ignore all submodules.
    let mut s = repo.submodules().unwrap();
    for submodule in s.iter_mut() {
        submodule.add_to_index(false).unwrap();
    }
    let mut index = repo.index().unwrap();
    index.add_all(&["*"], git2::ADD_DEFAULT, Some(|a: &[u8], _b: &[u8]| {
        if s.iter().any(|s| a.starts_with(s.path().as_vec())) {1} else {0}
    })).unwrap();
    index.write().unwrap();
}

fn add_submodule<'a>(repo: &'a git2::Repository, url: &str,
                     path: &Path) -> git2::Submodule<'a> {
    let mut s = repo.submodule(url, path, false).unwrap();
    let subrepo = s.open().unwrap();
    let mut origin = subrepo.find_remote("origin").unwrap();
    origin.add_fetch("refs/heads/*:refs/heads/*").unwrap();
    origin.fetch(&[], None, None).unwrap();
    origin.save().unwrap();
    subrepo.checkout_head(None).unwrap();
    s.add_finalize().unwrap();
    return s;
}

fn commit(repo: &git2::Repository) -> git2::Oid {
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let sig = repo.signature().unwrap();
    let mut parents = Vec::new();
    match repo.head().ok().map(|h| h.target().unwrap()) {
        Some(parent) => parents.push(repo.find_commit(parent).unwrap()),
        None => {}
    }
    let parents = parents.iter().collect::<Vec<_>>();
    repo.commit(Some("HEAD"), &sig, &sig, "test",
                &repo.find_tree(tree_id).unwrap(),
                parents.as_slice()).unwrap()
}

test!(cargo_compile_simple_git_dep {
    let project = project("foo");
    let git_project = git_repo("dep1", |project| {
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
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'

            [[bin]]

            name = "foo"
        "#, git_project.url()))
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("build"),
        execs()
        .with_stdout(format!("{} git repository `{}`\n\
                              {} dep1 v0.5.0 ({}#[..])\n\
                              {} foo v0.5.0 ({})\n",
                             UPDATING, path2url(git_root.clone()),
                             COMPILING, path2url(git_root),
                             COMPILING, path2url(root)))
        .with_stderr(""));

    assert_that(&project.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(project.bin("foo")).unwrap(),
      execs().with_stdout("hello world\n"));
});

test!(cargo_compile_git_dep_branch {
    let project = project("foo");
    let git_project = git_repo("dep1", |project| {
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
    repo.branch("branchy", &head, true, None, None).unwrap();

    let project = project
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'
            branch = "branchy"

            [[bin]]

            name = "foo"
        "#, git_project.url()))
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("build"),
        execs()
        .with_stdout(format!("{} git repository `{}`\n\
                              {} dep1 v0.5.0 ({}?branch=branchy#[..])\n\
                              {} foo v0.5.0 ({})\n",
                             UPDATING, path2url(git_root.clone()),
                             COMPILING, path2url(git_root),
                             COMPILING, path2url(root)))
        .with_stderr(""));

    assert_that(&project.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(project.bin("foo")).unwrap(),
      execs().with_stdout("hello world\n"));
});

test!(cargo_compile_git_dep_tag {
    let project = project("foo");
    let git_project = git_repo("dep1", |project| {
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

    // Make a tag correponding to the current HEAD
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    repo.tag("v0.1.0",
             &repo.find_object(head, None).unwrap(),
             &repo.signature().unwrap(),
             "make a new tag",
             false).unwrap();

    let project = project
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep1]

            git = '{}'
            tag = "v0.1.0"

            [[bin]]

            name = "foo"
        "#, git_project.url()))
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("build"),
        execs()
        .with_stdout(format!("{} git repository `{}`\n\
                              {} dep1 v0.5.0 ({}?tag=v0.1.0#[..])\n\
                              {} foo v0.5.0 ({})\n",
                             UPDATING, path2url(git_root.clone()),
                             COMPILING, path2url(git_root),
                             COMPILING, path2url(root))));

    assert_that(&project.bin("foo"), existing_file());

    assert_that(cargo::util::process(project.bin("foo")).unwrap(),
                execs().with_stdout("hello world\n"));

    assert_that(project.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));
});

test!(cargo_compile_with_nested_paths {
    let git_project = git_repo("dep1", |project| {
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
        .file("Cargo.toml", format!(r#"
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
              main_file(r#""{}", dep1::hello()"#, &["dep1"]).as_slice());

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("parent"), existing_file());

    assert_that(
      cargo::util::process(p.bin("parent")).unwrap(),
      execs().with_stdout("hello world\n"));
});

test!(cargo_compile_with_meta_package {
    let git_project = git_repo("meta-dep", |project| {
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
        .file("Cargo.toml", format!(r#"
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
              main_file(r#""{} {}", dep1::hello(), dep2::hello()"#, &["dep1", "dep2"]).as_slice());

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("parent"), existing_file());

    assert_that(
      cargo::util::process(p.bin("parent")).unwrap(),
      execs().with_stdout("this is dep1 this is dep2\n"));
});

test!(cargo_compile_with_short_ssh_git {
    let url = "git@github.com:a/dep";

    let project = project("project")
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.dep]

            git = "{}"

            [[bin]]

            name = "foo"
        "#, url))
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, &["dep1"]));

    assert_that(project.cargo_process("build"),
        execs()
        .with_stdout("")
        .with_stderr(format!("Cargo.toml is not a valid manifest\n\n\
                              invalid url `{}`: relative URL without a base\n", url)));
});

test!(two_revs_same_deps {
    let bar = git_repo("meta-dep", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn bar() -> int { 1 }")
    }).unwrap();

    let repo = git2::Repository::open(&bar.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Commit the changes and make sure we trigger a recompile
    File::create(&bar.root().join("src/lib.rs")).write_str(r#"
        pub fn bar() -> int { 2 }
    "#).unwrap();
    add(&repo);
    let rev2 = commit(&repo);

    let foo = project("foo")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies.bar]
            git = '{}'
            rev = "{}"

            [dependencies.baz]
            path = "../baz"
        "#, bar.url(), rev1).as_slice())
        .file("src/main.rs", r#"
            extern crate bar;
            extern crate baz;

            fn main() {
                assert_eq!(bar::bar(), 1);
                assert_eq!(baz::baz(), 2);
            }
        "#);

    let baz = project("baz")
        .file("Cargo.toml", format!(r#"
            [package]
            name = "baz"
            version = "0.0.0"
            authors = []

            [dependencies.bar]
            git = '{}'
            rev = "{}"
        "#, bar.url(), rev2).as_slice())
        .file("src/lib.rs", r#"
            extern crate bar;
            pub fn baz() -> int { bar::bar() }
        "#);

    baz.build();

    assert_that(foo.cargo_process("build"),
                execs().with_status(0));
    assert_that(&foo.bin("foo"), existing_file());
    assert_that(foo.process(foo.bin("foo")), execs().with_status(0));
});

test!(recompilation {
    let git_project = git_repo("bar", |project| {
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
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]

            version = "0.5.0"
            git = '{}'

            [[bin]]

            name = "foo"
        "#, git_project.url()))
        .file("src/foo.rs",
              main_file(r#""{}", bar::bar()"#, &["bar"]).as_slice());

    // First time around we should compile both foo and bar
    assert_that(p.cargo_process("build"),
                execs().with_stdout(format!("{} git repository `{}`\n\
                                             {} bar v0.5.0 ({}#[..])\n\
                                             {} foo v0.5.0 ({})\n",
                                            UPDATING, git_project.url(),
                                            COMPILING, git_project.url(),
                                            COMPILING, p.url())));

    // Don't recompile the second time
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(""));

    // Modify a file manually, shouldn't trigger a recompile
    File::create(&git_project.root().join("src/bar.rs")).write_str(r#"
        pub fn bar() { println!("hello!"); }
    "#).unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(""));

    assert_that(p.process(cargo_dir().join("cargo")).arg("update"),
                execs().with_stdout(format!("{} git repository `{}`",
                                            UPDATING,
                                            git_project.url())));

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(""));

    // Commit the changes and make sure we don't trigger a recompile because the
    // lockfile says not to change
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    add(&repo);
    commit(&repo);

    println!("compile after commit");
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(""));
    p.root().move_into_the_past().unwrap();

    // Update the dependency and carry on!
    assert_that(p.process(cargo_dir().join("cargo")).arg("update"),
                execs().with_stdout(format!("{} git repository `{}`",
                                            UPDATING,
                                            git_project.url())));
    println!("going for the last compile");
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(format!("{} bar v0.5.0 ({}#[..])\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, git_project.url(),
                                            COMPILING, p.url())));

    // Make sure clean only cleans one dep
    assert_that(p.process(cargo_dir().join("cargo")).arg("clean")
                 .arg("-p").arg("foo"),
                execs().with_stdout(""));
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(format!("{} foo v0.5.0 ({})\n",
                                            COMPILING, p.url())));
});

test!(update_with_shared_deps {
    let git_project = git_repo("bar", |project| {
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
            extern crate dep1;
            extern crate dep2;
            fn main() {}
        "#)
        .file("dep1/Cargo.toml", format!(r#"
            [package]
            name = "dep1"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            version = "0.5.0"
            git = '{}'
        "#, git_project.url()))
        .file("dep1/src/lib.rs", "")
        .file("dep2/Cargo.toml", format!(r#"
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
                execs().with_stdout(format!("\
{updating} git repository `{git}`
{compiling} bar v0.5.0 ({git}#[..])
{compiling} [..] v0.5.0 ({dir})
{compiling} [..] v0.5.0 ({dir})
{compiling} foo v0.5.0 ({dir})\n",
                    updating = UPDATING, git = git_project.url(),
                    compiling = COMPILING, dir = p.url())));

    // Modify a file manually, and commit it
    File::create(&git_project.root().join("src/bar.rs")).write_str(r#"
        pub fn bar() { println!("hello!"); }
    "#).unwrap();
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let old_head = repo.head().unwrap().target().unwrap();
    add(&repo);
    commit(&repo);

    timer::sleep(Duration::milliseconds(1000));

    // By default, not transitive updates
    println!("dep1 update");
    assert_that(p.process(cargo_dir().join("cargo")).arg("update").arg("dep1"),
                execs().with_stdout(""));

    // Specifying a precise rev to the old rev shouldn't actually update
    // anything because we already have the rev in the db.
    println!("bar precise update");
    assert_that(p.process(cargo_dir().join("cargo")).arg("update").arg("bar")
                 .arg("--precise").arg(old_head.to_string()),
                execs().with_stdout(""));

    // Updating aggressively should, however, update the repo.
    println!("dep1 aggressive update");
    assert_that(p.process(cargo_dir().join("cargo")).arg("update").arg("dep1")
                 .arg("--aggressive"),
                execs().with_stdout(format!("{} git repository `{}`",
                                            UPDATING,
                                            git_project.url())));

    // Make sure we still only compile one version of the git repo
    println!("build");
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(format!("\
{compiling} bar v0.5.0 ({git}#[..])
{compiling} [..] v0.5.0 ({dir})
{compiling} [..] v0.5.0 ({dir})
{compiling} foo v0.5.0 ({dir})\n",
                    git = git_project.url(),
                    compiling = COMPILING, dir = p.url())));

    // We should be able to update transitive deps
    assert_that(p.process(cargo_dir().join("cargo")).arg("update").arg("bar"),
                execs().with_stdout(format!("{} git repository `{}`",
                                            UPDATING,
                                            git_project.url())));
});

test!(dep_with_submodule {
    let project = project("foo");
    let git_project = git_repo("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
    }).unwrap();
    let git_project2 = git_repo("dep2", |project| {
        project
            .file("lib.rs", "pub fn dep() {}")
    }).unwrap();

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let url = path2url(git_project2.root()).to_string();
    add_submodule(&repo, url.as_slice(), &Path::new("src"));
    commit(&repo);

    let project = project
        .file("Cargo.toml", format!(r#"
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
        execs().with_stderr("").with_status(0));
});

test!(two_deps_only_update_one {
    let project = project("foo");
    let git1 = git_repo("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
            .file("src/lib.rs", "")
    }).unwrap();
    let git2 = git_repo("dep2", |project| {
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
        .file("Cargo.toml", format!(r#"
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
        .with_stdout(format!("{} git repository `[..]`\n\
                              {} git repository `[..]`\n\
                              {} [..] v0.5.0 ([..])\n\
                              {} [..] v0.5.0 ([..])\n\
                              {} foo v0.5.0 ({})\n",
                             UPDATING,
                             UPDATING,
                             COMPILING,
                             COMPILING,
                             COMPILING, project.url()))
        .with_stderr(""));

    File::create(&git1.root().join("src/lib.rs")).write_str(r#"
        pub fn foo() {}
    "#).unwrap();
    let repo = git2::Repository::open(&git1.root()).unwrap();
    add(&repo);
    commit(&repo);

    assert_that(project.process(cargo_dir().join("cargo")).arg("update")
                       .arg("-p").arg("dep1"),
        execs()
        .with_stdout(format!("{} git repository `{}`\n",
                             UPDATING, git1.url()))
        .with_stderr(""));
});

test!(stale_cached_version {
    let bar = git_repo("meta-dep", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn bar() -> int { 1 }")
    }).unwrap();

    // Update the git database in the cache with the current state of the git
    // repo
    let foo = project("foo")
        .file("Cargo.toml", format!(r#"
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
    assert_that(foo.process(foo.bin("foo")), execs().with_status(0));

    // Update the repo, and simulate someone else updating the lockfile and then
    // us pulling it down.
    File::create(&bar.root().join("src/lib.rs")).write_str(r#"
        pub fn bar() -> int { 1 + 0 }
    "#).unwrap();
    let repo = git2::Repository::open(&bar.root()).unwrap();
    add(&repo);
    commit(&repo);

    timer::sleep(Duration::milliseconds(1000));

    let rev = repo.revparse_single("HEAD").unwrap().id();

    File::create(&foo.root().join("Cargo.lock")).write_str(format!(r#"
        [root]
        name = "foo"
        version = "0.0.0"
        dependencies = [
         'bar 0.0.0 (git+{url}#{hash})'
        ]

        [[package]]
        name = "bar"
        version = "0.0.0"
        source = 'git+{url}#{hash}'
    "#, url = bar.url(), hash = rev).as_slice()).unwrap();

    // Now build!
    assert_that(foo.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{updating} git repository `{bar}`
{compiling} bar v0.0.0 ({bar}#[..])
{compiling} foo v0.0.0 ({foo})
", updating = UPDATING, compiling = COMPILING, bar = bar.url(), foo = foo.url())));
    assert_that(foo.process(foo.bin("foo")), execs().with_status(0));
});

test!(dep_with_changed_submodule {
    let project = project("foo");
    let git_project = git_repo("dep1", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep1"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
    }).unwrap();

    let git_project2 = git_repo("dep2", |project| {
        project
            .file("lib.rs", "pub fn dep() -> &'static str { \"project2\" }")
    }).unwrap();

    let git_project3 = git_repo("dep3", |project| {
        project
            .file("lib.rs", "pub fn dep() -> &'static str { \"project3\" }")
    }).unwrap();

    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let mut sub = add_submodule(&repo, git_project2.url().to_string().as_slice(),
                                &Path::new("src"));
    commit(&repo);

    let project = project
        .file("Cargo.toml", format!(r#"
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
                .with_stdout(format!("{} git repository `[..]`\n\
                                      {} dep1 v0.5.0 ([..])\n\
                                      {} foo v0.5.0 ([..])\n\
                                      {} `target[..]foo`\n\
                                      project2\
                                      ",
                                      UPDATING,
                                      COMPILING,
                                      COMPILING,
                                      RUNNING))
                .with_stderr("")
                .with_status(0));

    let mut file = File::create(&git_project.root().join(".gitmodules"));
    file.write_str(format!("[submodule \"src\"]\n\tpath = src\n\turl={}",
                           git_project3.url()).as_slice()).unwrap();

    // Sync the submodule and reset it to the new remote.
    sub.sync().unwrap();
    {
        let subrepo = sub.open().unwrap();
        let mut origin = subrepo.find_remote("origin").unwrap();
        origin.set_url(git_project3.url().to_string().as_slice()).unwrap();
        origin.add_fetch("refs/heads/*:refs/heads/*").unwrap();;
        origin.fetch(&[], None, None).unwrap();
        origin.save().unwrap();
        let id = subrepo.refname_to_id("refs/remotes/origin/master").unwrap();
        let obj = subrepo.find_object(id, None).unwrap();
        subrepo.reset(&obj, git2::ResetType::Hard, None, None).unwrap();
    }
    sub.add_to_index(true).unwrap();
    add(&repo);
    commit(&repo);

    timer::sleep(Duration::milliseconds(1000));
    // Update the dependency and carry on!
    println!("update");
    assert_that(project.process(cargo_dir().join("cargo")).arg("update").arg("-v"),
                execs()
                .with_stderr("")
                .with_stdout(format!("{} git repository `{}`",
                                     UPDATING,
                                     git_project.url())));

    println!("last run");
    assert_that(project.process(cargo_dir().join("cargo")).arg("run"), execs()
                .with_stdout(format!("{compiling} dep1 v0.5.0 ([..])\n\
                                      {compiling} foo v0.5.0 ([..])\n\
                                      {running} `target[..]foo`\n\
                                      project3\
                                      ",
                                      compiling = COMPILING, running = RUNNING))
                .with_stderr("")
                .with_status(0));
});

test!(dev_deps_with_testing {
    let p2 = git_repo("bar", |project| {
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
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dev-dependencies.bar]
            version = "0.5.0"
            git = '{}'
        "#, p2.url()).as_slice())
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
        execs().with_stdout(format!("\
{updating} git repository `{bar}`
{compiling} foo v0.5.0 ({url})
", updating = UPDATING, compiling = COMPILING, url = p.url(), bar = p2.url())));

    // Make sure we use the previous resolution of `bar` instead of updating it
    // a second time.
    assert_that(p.process(cargo_dir().join("cargo")).arg("test"),
        execs().with_stdout(format!("\
{compiling} [..] v0.5.0 ([..])
{compiling} [..] v0.5.0 ([..]
{running} target[..]foo-[..]

running 1 test
test tests::foo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

", compiling = COMPILING, running = RUNNING)));
});

test!(git_build_cmd_freshness {
    let foo = git_repo("foo", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "true"
        "#)
        .file("src/lib.rs", "pub fn bar() -> int { 1 }")
        .file(".gitignore", "
            src/bar.rs
        ")
    }).unwrap();
    foo.root().move_into_the_past().unwrap();

    timer::sleep(Duration::milliseconds(1000));

    assert_that(foo.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
", compiling = COMPILING, url = foo.url())));

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    assert_that(foo.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(""));

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    File::create(&foo.root().join("src/bar.rs")).unwrap();
    assert_that(foo.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(""));
});

test!(git_name_not_always_needed {
    let p2 = git_repo("bar", |project| {
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
        .file("Cargo.toml", format!(r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dev-dependencies.bar]
            git = '{}'
        "#, p2.url()).as_slice())
        .file("src/main.rs", "fn main() {}");

    // Generate a lockfile which did not use `bar` to compile, but had to update
    // `bar` to generate the lockfile
    assert_that(p.cargo_process("build"),
        execs().with_stdout(format!("\
{updating} git repository `{bar}`
{compiling} foo v0.5.0 ({url})
", updating = UPDATING, compiling = COMPILING, url = p.url(), bar = p2.url())));
});

test!(git_repo_changing_no_rebuild {
    let bar = git_repo("bar", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "pub fn bar() -> int { 1 }")
    }).unwrap();

    // Lock p1 to the first rev in the git repo
    let p1 = project("p1")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "p1"
            version = "0.5.0"
            authors = []
            build = 'true'
            [dependencies.bar]
            git = '{}'
        "#, bar.url()).as_slice())
        .file("src/main.rs", "fn main() {}");
    p1.build();
    p1.root().move_into_the_past().unwrap();
    assert_that(p1.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(format!("\
{updating} git repository `{bar}`
{compiling} [..]
{compiling} [..]
", updating = UPDATING, compiling = COMPILING, bar = bar.url())));

    // Make a commit to lock p2 to a different rev
    File::create(&bar.root().join("src/lib.rs")).write_str(r#"
        pub fn bar() -> int { 2 }
    "#).unwrap();
    let repo = git2::Repository::open(&bar.root()).unwrap();
    add(&repo);
    commit(&repo);

    // Lock p2 to the second rev
    let p2 = project("p2")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "p2"
            version = "0.5.0"
            authors = []
            [dependencies.bar]
            git = '{}'
        "#, bar.url()).as_slice())
        .file("src/main.rs", "fn main() {}");
    assert_that(p2.cargo_process("build"),
                execs().with_stdout(format!("\
{updating} git repository `{bar}`
{compiling} [..]
{compiling} [..]
", updating = UPDATING, compiling = COMPILING, bar = bar.url())));

    // And now for the real test! Make sure that p1 doesn't get rebuilt
    // even though the git repo has changed.
    assert_that(p1.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_stdout(""));
});

test!(git_dep_build_cmd {
    let p = git_repo("foo", |project| {
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
              main_file(r#""{}", bar::gimme()"#, &["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "cp src/bar.rs.in src/bar.rs"

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs.in", r#"
            pub fn gimme() -> int { 0 }
        "#)
    }).unwrap();

    p.root().join("bar").move_into_the_past().unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
        execs().with_status(0));

    assert_that(
      cargo::util::process(p.bin("foo")).unwrap(),
      execs().with_stdout("0\n"));

    // Touching bar.rs.in should cause the `build` command to run again.
    let mut file = fs::File::create(&p.root().join("bar/src/bar.rs.in")).unwrap();
    file.write_str(r#"pub fn gimme() -> int { 1 }"#).unwrap();
    drop(file);

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
        execs().with_status(0));

    assert_that(
      cargo::util::process(p.bin("foo")).unwrap(),
      execs().with_stdout("1\n"));
});

test!(fetch_downloads {
    let bar = git_repo("bar", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "pub fn bar() -> int { 1 }")
    }).unwrap();

    let p = project("p1")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "p1"
            version = "0.5.0"
            authors = []
            [dependencies.bar]
            git = '{}'
        "#, bar.url()).as_slice())
        .file("src/main.rs", "fn main() {}");
    assert_that(p.cargo_process("fetch"),
                execs().with_status(0).with_stdout(format!("\
{updating} git repository `{url}`
", updating = UPDATING, url = bar.url())));

    assert_that(p.process(cargo_dir().join("cargo")).arg("fetch"),
                execs().with_status(0).with_stdout(""));
});

test!(warnings_in_git_dep {
    let bar = git_repo("bar", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "fn unused() {}")
    }).unwrap();

    let p = project("foo")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []
            [dependencies.bar]
            git = '{}'
        "#, bar.url()).as_slice())
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
        execs()
        .with_stdout(format!("{} git repository `{}`\n\
                              {} bar v0.5.0 ({}#[..])\n\
                              {} foo v0.5.0 ({})\n",
                             UPDATING, bar.url(),
                             COMPILING, bar.url(),
                             COMPILING, p.url()))
        .with_stderr(""));
});

test!(update_ambiguous {
    let foo1 = git_repo("foo1", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "")
    }).unwrap();
    let foo2 = git_repo("foo2", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.6.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "")
    }).unwrap();
    let bar = git_repo("bar", |project| {
        project.file("Cargo.toml", format!(r#"
            [package]
            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.foo]
            git = '{}'
        "#, foo2.url()).as_slice())
        .file("src/lib.rs", "")
    }).unwrap();

    let p = project("project")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.foo]
            git = '{}'
            [dependencies.bar]
            git = '{}'
        "#, foo1.url(), bar.url()).as_slice())
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("generate-lockfile"), execs().with_status(0));
    assert_that(p.process(cargo_dir().join("cargo")).arg("update")
                 .arg("-p").arg("foo"),
                execs().with_status(101)
                       .with_stderr("\
There are multiple `foo` packages in your project, and the specification `foo` \
is ambiguous.
Please re-run this command with `-p <spec>` where `<spec>` is one of the \
following:
  foo:0.[..].0
  foo:0.[..].0
"));
});

test!(update_one_dep_in_repo_with_many_deps {
    let foo = git_repo("foo", |project| {
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
        .file("Cargo.toml", format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.foo]
            git = '{}'
            [dependencies.a]
            git = '{}'
        "#, foo.url(), foo.url()).as_slice())
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("generate-lockfile"), execs().with_status(0));
    assert_that(p.process(cargo_dir().join("cargo")).arg("update")
                 .arg("-p").arg("foo"),
                execs().with_status(0)
                       .with_stdout(format!("\
Updating git repository `{}`
", foo.url())));
});

test!(switch_deps_does_not_update_transitive {
    let transitive = git_repo("transitive", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "transitive"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/lib.rs", "")
    }).unwrap();
    let dep1 = git_repo("dep1", |project| {
        project.file("Cargo.toml", format!(r#"
            [package]
            name = "dep"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.transitive]
            git = '{}'
        "#, transitive.url()).as_slice())
        .file("src/lib.rs", "")
    }).unwrap();
    let dep2 = git_repo("dep2", |project| {
        project.file("Cargo.toml", format!(r#"
            [package]
            name = "dep"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.transitive]
            git = '{}'
        "#, transitive.url()).as_slice())
        .file("src/lib.rs", "")
    }).unwrap();

    let p = project("project")
        .file("Cargo.toml", format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.dep]
            git = '{}'
        "#, dep1.url()).as_slice())
        .file("src/main.rs", "fn main() {}");

    p.build();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
Updating git repository `{}`
Updating git repository `{}`
{compiling} transitive [..]
{compiling} dep [..]
{compiling} project [..]
", dep1.url(), transitive.url(), compiling = COMPILING)));

    // Update the dependency to point to the second repository, but this
    // shouldn't update the transitive dependency which is the same.
    File::create(&p.root().join("Cargo.toml")).write_str(format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.dep]
            git = '{}'
    "#, dep2.url()).as_slice()).unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
Updating git repository `{}`
{compiling} dep [..]
{compiling} project [..]
", dep2.url(), compiling = COMPILING)));
});

test!(update_one_source_updates_all_packages_in_that_git_source {
    let dep = git_repo("dep", |project| {
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
        .file("Cargo.toml", format!(r#"
            [project]
            name = "project"
            version = "0.5.0"
            authors = []
            [dependencies.dep]
            git = '{}'
        "#, dep.url()).as_slice())
        .file("src/main.rs", "fn main() {}");

    p.build();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));

    let repo = git2::Repository::open(&dep.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Just be sure to change a file
    File::create(&dep.root().join("src/lib.rs")).write_str(r#"
        pub fn bar() -> int { 2 }
    "#).unwrap();
    add(&repo);
    commit(&repo);

    assert_that(p.process(cargo_dir().join("cargo")).arg("update")
                 .arg("-p").arg("dep"),
                execs().with_status(0));
    let lockfile = File::open(&p.root().join("Cargo.lock")).read_to_string()
                                                           .unwrap();
    assert!(!lockfile.as_slice().contains(rev1.to_string().as_slice()),
            "{} in {}", rev1, lockfile);
});

test!(switch_sources {
    let a1 = git_repo("a1", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.5.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
    }).unwrap();
    let a2 = git_repo("a2", |project| {
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
        .file("b/Cargo.toml", format!(r#"
            [project]
            name = "b"
            version = "0.5.0"
            authors = []
            [dependencies.a]
            git = '{}'
        "#, a1.url()).as_slice())
        .file("b/src/lib.rs", "fn main() {}");

    p.build();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{updating} git repository `file://[..]a1`
{compiling} a v0.5.0 ([..]a1#[..]
{compiling} b v0.5.0 ([..])
{compiling} project v0.5.0 ([..])
", updating = UPDATING, compiling = COMPILING).as_slice()));

    File::create(&p.root().join("b/Cargo.toml")).write_str(format!(r#"
        [project]
        name = "b"
        version = "0.5.0"
        authors = []
        [dependencies.a]
        git = '{}'
    "#, a2.url()).as_slice()).unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{updating} git repository `file://[..]a2`
{compiling} a v0.5.1 ([..]a2#[..]
{compiling} b v0.5.0 ([..])
{compiling} project v0.5.0 ([..])
", updating = UPDATING, compiling = COMPILING).as_slice()));
});
