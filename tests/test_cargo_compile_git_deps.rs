use std::io::File;

use support::{ProjectBuilder, ResultTest, project, execs, main_file, paths};
use support::{cargo_dir, path2url};
use support::{COMPILING, FRESH, UPDATING};
use support::paths::PathExt;
use hamcrest::{assert_that,existing_file};
use cargo;
use cargo::util::{ProcessError, process};

fn setup() {
}

fn git_repo(name: &str, callback: |ProjectBuilder| -> ProjectBuilder)
    -> Result<ProjectBuilder, ProcessError>
{
    let gitconfig = paths::home().join(".gitconfig");

    if !gitconfig.exists() {
        File::create(&gitconfig).write(r"
            [user]

            email = foo@bar.com
            name = Foo Bar
        ".as_bytes()).assert()
    }

    let mut git_project = project(name);
    git_project = callback(git_project);
    git_project.build();

    log!(5, "git init");
    try!(git_project.process("git").args(["init", "--template="]).exec_with_output());
    log!(5, "building git project");
    log!(5, "git add .");
    try!(git_project.process("git").args(["add", "."]).exec_with_output());
    log!(5, "git commit");
    try!(git_project.process("git").args(["commit", "-m", "Initial commit"])
                    .exec_with_output());
    Ok(git_project)
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
    }).assert();

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
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, ["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("cargo-build"),
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
      cargo::util::process(project.bin("foo")),
      execs().with_stdout("hello world\n"));
})

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
    }).assert();

    git_project.process("git").args(["checkout", "-b", "branchy"]).exec_with_output().assert();
    git_project.process("git").args(["branch", "-d", "master"]).exec_with_output().assert();

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
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, ["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("cargo-build"),
        execs()
        .with_stdout(format!("{} git repository `{}`\n\
                              {} dep1 v0.5.0 ({}?ref=branchy#[..])\n\
                              {} foo v0.5.0 ({})\n",
                             UPDATING, path2url(git_root.clone()),
                             COMPILING, path2url(git_root),
                             COMPILING, path2url(root)))
        .with_stderr(""));

    assert_that(&project.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(project.bin("foo")),
      execs().with_stdout("hello world\n"));
})

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
    }).assert();

    git_project.process("git").args(["tag", "v0.1.0"]).exec_with_output().assert();
    git_project.process("git").args(["checkout", "-b", "tmp"]).exec_with_output().assert();
    git_project.process("git").args(["branch", "-d", "master"]).exec_with_output().assert();

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
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, ["dep1"]));

    let root = project.root();
    let git_root = git_project.root();

    assert_that(project.cargo_process("cargo-build"),
        execs()
        .with_stdout(format!("{} git repository `{}`\n\
                              {} dep1 v0.5.0 ({}?ref=v0.1.0#[..])\n\
                              {} foo v0.5.0 ({})\n",
                             UPDATING, path2url(git_root.clone()),
                             COMPILING, path2url(git_root),
                             COMPILING, path2url(root)))
        .with_stderr(""));

    assert_that(&project.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(project.bin("foo")),
      execs().with_stdout("hello world\n"));
})

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
    }).assert();

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
              main_file(r#""{}", dep1::hello()"#, ["dep1"]).as_slice());

    p.cargo_process("cargo-build")
        .exec_with_output()
        .assert();

    assert_that(&p.bin("parent"), existing_file());

    assert_that(
      cargo::util::process(p.bin("parent")),
      execs().with_stdout("hello world\n"));
})

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
    }).assert();

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
              main_file(r#""{} {}", dep1::hello(), dep2::hello()"#, ["dep1", "dep2"]).as_slice());

    p.cargo_process("cargo-build")
        .exec_with_output()
        .assert();

    assert_that(&p.bin("parent"), existing_file());

    assert_that(
      cargo::util::process(p.bin("parent")),
      execs().with_stdout("this is dep1 this is dep2\n"));
})

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
        .file("src/foo.rs", main_file(r#""{}", dep1::hello()"#, ["dep1"]));

    assert_that(project.cargo_process("cargo-build"),
        execs()
        .with_stdout("")
        .with_stderr(format!("Cargo.toml is not a valid manifest\n\n\
                              invalid url `{}`: Relative URL without a base\n", url)));
})

test!(two_revs_same_deps {
    let bar = git_repo("meta-dep", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn bar() -> int { 1 }")
    }).assert();

    // Commit the changes and make sure we trigger a recompile
    let rev1 = bar.process("git").args(["rev-parse", "HEAD"])
                  .exec_with_output().assert();
    File::create(&bar.root().join("src/lib.rs")).write_str(r#"
        pub fn bar() -> int { 2 }
    "#).assert();
    bar.process("git").args(["add", "."]).exec_with_output().assert();
    bar.process("git").args(["commit", "-m", "test"]).exec_with_output()
       .assert();
    let rev2 = bar.process("git").args(["rev-parse", "HEAD"])
                  .exec_with_output().assert();

    let rev1 = String::from_utf8(rev1.output).unwrap();
    let rev2 = String::from_utf8(rev2.output).unwrap();

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
        "#, bar.url(), rev1.as_slice().trim()).as_slice())
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
        "#, bar.url(), rev2.as_slice().trim()).as_slice())
        .file("src/lib.rs", r#"
            extern crate bar;
            pub fn baz() -> int { bar::bar() }
        "#);

    baz.build();

    // TODO: -j1 is a hack
    assert_that(foo.cargo_process("cargo-build").arg("-j").arg("1"),
                execs().with_status(0));
    assert_that(&foo.bin("foo"), existing_file());
    assert_that(foo.process(foo.bin("foo")), execs().with_status(0));
})

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
    }).assert();

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
              main_file(r#""{}", bar::bar()"#, ["bar"]).as_slice());

    // First time around we should compile both foo and bar
    assert_that(p.cargo_process("cargo-build"),
                execs().with_stdout(format!("{} git repository `{}`\n\
                                             {} bar v0.5.0 ({}#[..])\n\
                                             {} foo v0.5.0 ({})\n",
                                            UPDATING, git_project.url(),
                                            COMPILING, git_project.url(),
                                            COMPILING, p.url())));

    // Don't recompile the second time
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} bar v0.5.0 ({}#[..])\n\
                                             {} foo v0.5.0 ({})\n",
                                            FRESH, git_project.url(),
                                            FRESH, p.url())));

    // Modify a file manually, shouldn't trigger a recompile
    File::create(&git_project.root().join("src/bar.rs")).write_str(r#"
        pub fn bar() { println!("hello!"); }
    "#).assert();

    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} bar v0.5.0 ({}#[..])\n\
                                             {} foo v0.5.0 ({})\n",
                                            FRESH, git_project.url(),
                                            FRESH, p.url())));

    assert_that(p.process(cargo_dir().join("cargo-update")),
                execs().with_stdout(format!("{} git repository `{}`",
                                            UPDATING,
                                            git_project.url())));

    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} bar v0.5.0 ({}#[..])\n\
                                             {} foo v0.5.0 ({})\n",
                                            FRESH, git_project.url(),
                                            FRESH, p.url())));

    // Commit the changes and make sure we don't trigger a recompile because the
    // lockfile says not to change
    git_project.process("git").args(["add", "."]).exec_with_output().assert();
    git_project.process("git").args(["commit", "-m", "test"]).exec_with_output()
               .assert();

    println!("compile after commit");
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} bar v0.5.0 ({}#[..])\n\
                                             {} foo v0.5.0 ({})\n",
                                            FRESH, git_project.url(),
                                            FRESH, p.url())));
    p.root().move_into_the_past().assert();

    // Update the dependency and carry on!
    assert_that(p.process(cargo_dir().join("cargo-update")),
                execs().with_stdout(format!("{} git repository `{}`",
                                            UPDATING,
                                            git_project.url())));
    println!("going for the last compile");
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("{} bar v0.5.0 ({}#[..])\n\
                                             {} foo v0.5.0 ({})\n",
                                            COMPILING, git_project.url(),
                                            COMPILING, p.url())));
})

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
    }).assert();

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
    assert_that(p.cargo_process("cargo-build"),
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
    "#).assert();
    git_project.process("git").args(["add", "."]).exec_with_output().assert();
    git_project.process("git").args(["commit", "-m", "test"]).exec_with_output()
               .assert();
    assert_that(p.process(cargo_dir().join("cargo-update")).arg("dep1"),
                execs().with_stdout(format!("{} git repository `{}`",
                                            UPDATING,
                                            git_project.url())));

    // Make sure we still only compile one version of the git repo
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_stdout(format!("\
{compiling} bar v0.5.0 ({git}#[..])
{compiling} [..] v0.5.0 ({dir})
{compiling} [..] v0.5.0 ({dir})
{compiling} foo v0.5.0 ({dir})\n",
                    git = git_project.url(),
                    compiling = COMPILING, dir = p.url())));
})

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
    }).assert();
    let git_project2 = git_repo("dep2", |project| {
        project
            .file("lib.rs", "pub fn dep() {}")
    }).assert();

    git_project.process("git").args(["submodule", "add"])
               .arg(git_project2.root()).arg("src").exec_with_output().assert();
    git_project.process("git").args(["add", "."]).exec_with_output().assert();
    git_project.process("git").args(["commit", "-m", "test"]).exec_with_output()
               .assert();

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

    assert_that(project.cargo_process("cargo-build"),
        execs().with_stderr("").with_status(0));
})

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
    }).assert();
    let git2 = git_repo("dep2", |project| {
        project
            .file("Cargo.toml", r#"
                [package]
                name = "dep2"
                version = "0.5.0"
                authors = ["carlhuda@example.com"]
            "#)
            .file("src/lib.rs", "")
    }).assert();

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

    assert_that(project.cargo_process("cargo-build"),
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
    "#).assert();
    git1.process("git").args(["add", "."]).exec_with_output().assert();
    git1.process("git").args(["commit", "-m", "test"]).exec_with_output()
        .assert();

    assert_that(project.process(cargo_dir().join("cargo-update")).arg("dep1"),
        execs()
        .with_stdout(format!("{} git repository `{}`\n",
                             UPDATING, git1.url()))
        .with_stderr(""));
})

test!(stale_cached_version {
    let bar = git_repo("meta-dep", |project| {
        project.file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn bar() -> int { 1 }")
    }).assert();

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

    assert_that(foo.cargo_process("cargo-build"), execs().with_status(0));
    assert_that(foo.process(foo.bin("foo")), execs().with_status(0));

    // Update the repo, and simulate someone else updating the lockfile and then
    // us pulling it down.
    File::create(&bar.root().join("src/lib.rs")).write_str(r#"
        pub fn bar() -> int { 1 + 0 }
    "#).assert();
    bar.process("git").args(["add", "."]).exec_with_output().assert();
    bar.process("git").args(["commit", "-m", "test"]).exec_with_output()
       .assert();
    let rev = bar.process("git").args(["rev-parse", "HEAD"])
                 .exec_with_output().assert();
    let rev = String::from_utf8(rev.output).unwrap();

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
    "#, url = bar.url(), hash = rev).as_slice()).assert();

    // Now build!
    assert_that(foo.process(cargo_dir().join("cargo-build")),
                execs().with_status(0)
                       .with_stdout(format!("\
{updating} git repository `{bar}`
{compiling} bar v0.0.0 ({bar}#[..])
{compiling} foo v0.0.0 ({foo})
", updating = UPDATING, compiling = COMPILING, bar = bar.url(), foo = foo.url())));
    assert_that(foo.process(foo.bin("foo")), execs().with_status(0));
})
