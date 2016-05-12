use std::{env, str};
use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::process::Stdio;
use std::thread;

use git2;
use hamcrest::{assert_that, existing_file};

use support::{execs, project};
use support::git;
use support::registry::Package;
use test_cargo_install::{cargo_home, has_installed_exe};

fn setup() {}

fn pkg(name: &str, vers: &str) {
    Package::new(name, vers)
        .file("src/main.rs", "fn main() {{}}")
        .publish();
}

test!(multiple_installs {
    let p = project("foo")
        .file("a/Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
        "#)
        .file("a/src/main.rs", "fn main() {}")
        .file("b/Cargo.toml", r#"
            [package]
            name = "bar"
            authors = []
            version = "0.0.0"
        "#)
        .file("b/src/main.rs", "fn main() {}");
    p.build();

    let mut a = p.cargo("install").cwd(p.root().join("a")).build_command();
    let mut b = p.cargo("install").cwd(p.root().join("b")).build_command();

    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());

    let a = a.spawn().unwrap();
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    assert_that(a, execs().with_status(0));
    assert_that(b, execs().with_status(0));

    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), has_installed_exe("bar"));
});

test!(concurrent_installs {
    const LOCKED_BUILD: &'static str = "waiting for file lock on build directory";

    pkg("foo", "0.0.1");
    pkg("bar", "0.0.1");

    let mut a = ::cargo_process().arg("install").arg("foo").build_command();
    let mut b = ::cargo_process().arg("install").arg("bar").build_command();

    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());

    let a = a.spawn().unwrap();
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    assert!(!str::from_utf8(&a.stderr).unwrap().contains(LOCKED_BUILD));
    assert!(!str::from_utf8(&b.stderr).unwrap().contains(LOCKED_BUILD));

    assert_that(a, execs().with_status(0));
    assert_that(b, execs().with_status(0));

    assert_that(cargo_home(), has_installed_exe("foo"));
    assert_that(cargo_home(), has_installed_exe("bar"));
});

test!(one_install_should_be_bad {
    let p = project("foo")
        .file("a/Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
        "#)
        .file("a/src/main.rs", "fn main() {}")
        .file("b/Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
        "#)
        .file("b/src/main.rs", "fn main() {}");
    p.build();

    let mut a = p.cargo("install").cwd(p.root().join("a")).build_command();
    let mut b = p.cargo("install").cwd(p.root().join("b")).build_command();

    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());

    let a = a.spawn().unwrap();
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    let (bad, good) = if a.status.code() == Some(101) {(a, b)} else {(b, a)};
    assert_that(bad, execs().with_status(101).with_stderr_contains("\
[ERROR] binary `foo[..]` already exists in destination as part of `[..]`
"));
    assert_that(good, execs().with_status(0).with_stderr_contains("\
warning: be sure to add `[..]` to your PATH [..]
"));

    assert_that(cargo_home(), has_installed_exe("foo"));
});

test!(multiple_registry_fetches {
    let mut pkg = Package::new("bar", "1.0.2");
    for i in 0..10 {
        let name = format!("foo{}", i);
        Package::new(&name, "1.0.0").publish();
        pkg.dep(&name, "*");
    }
    pkg.publish();

    let p = project("foo")
        .file("a/Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [dependencies]
            bar = "*"
        "#)
        .file("a/src/main.rs", "fn main() {}")
        .file("b/Cargo.toml", r#"
            [package]
            name = "bar"
            authors = []
            version = "0.0.0"

            [dependencies]
            bar = "*"
        "#)
        .file("b/src/main.rs", "fn main() {}");
    p.build();

    let mut a = p.cargo("build").cwd(p.root().join("a")).build_command();
    let mut b = p.cargo("build").cwd(p.root().join("b")).build_command();

    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());

    let a = a.spawn().unwrap();
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    assert_that(a, execs().with_status(0));
    assert_that(b, execs().with_status(0));

    let suffix = env::consts::EXE_SUFFIX;
    assert_that(&p.root().join("a/target/debug").join(format!("foo{}", suffix)),
                existing_file());
    assert_that(&p.root().join("b/target/debug").join(format!("bar{}", suffix)),
                existing_file());
});

test!(git_same_repo_different_tags {
    let a = git::new("dep", |project| {
        project.file("Cargo.toml", r#"
            [project]
            name = "dep"
            version = "0.5.0"
            authors = []
        "#).file("src/lib.rs", "pub fn tag1() {}")
    }).unwrap();

    let repo = git2::Repository::open(&a.root()).unwrap();
    git::tag(&repo, "tag1");

    File::create(a.root().join("src/lib.rs")).unwrap()
         .write_all(b"pub fn tag2() {}").unwrap();
    git::add(&repo);
    git::commit(&repo);
    git::tag(&repo, "tag2");

    let p = project("foo")
        .file("a/Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [dependencies]
            dep = {{ git = '{}', tag = 'tag1' }}
        "#, a.url()))
        .file("a/src/main.rs", "extern crate dep; fn main() { dep::tag1(); }")
        .file("b/Cargo.toml", &format!(r#"
            [package]
            name = "bar"
            authors = []
            version = "0.0.0"

            [dependencies]
            dep = {{ git = '{}', tag = 'tag2' }}
        "#, a.url()))
        .file("b/src/main.rs", "extern crate dep; fn main() { dep::tag2(); }");
    p.build();

    let mut a = p.cargo("build").arg("-v").cwd(p.root().join("a")).build_command();
    let mut b = p.cargo("build").arg("-v").cwd(p.root().join("b")).build_command();

    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());

    let a = a.spawn().unwrap();
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    assert_that(a, execs().with_status(0));
    assert_that(b, execs().with_status(0));
});

test!(git_same_branch_different_revs {
    let a = git::new("dep", |project| {
        project.file("Cargo.toml", r#"
            [project]
            name = "dep"
            version = "0.5.0"
            authors = []
        "#).file("src/lib.rs", "pub fn f1() {}")
    }).unwrap();

    let p = project("foo")
        .file("a/Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [dependencies]
            dep = {{ git = '{}' }}
        "#, a.url()))
        .file("a/src/main.rs", "extern crate dep; fn main() { dep::f1(); }")
        .file("b/Cargo.toml", &format!(r#"
            [package]
            name = "bar"
            authors = []
            version = "0.0.0"

            [dependencies]
            dep = {{ git = '{}' }}
        "#, a.url()))
        .file("b/src/main.rs", "extern crate dep; fn main() { dep::f2(); }");
    p.build();

    // Generate a Cargo.lock pointing at the current rev, then clear out the
    // target directory
    assert_that(p.cargo("build").cwd(p.root().join("a")),
                execs().with_status(0));
    fs::remove_dir_all(p.root().join("a/target")).unwrap();

    // Make a new commit on the master branch
    let repo = git2::Repository::open(&a.root()).unwrap();
    File::create(a.root().join("src/lib.rs")).unwrap()
         .write_all(b"pub fn f2() {}").unwrap();
    git::add(&repo);
    git::commit(&repo);

    // Now run both builds in parallel. The build of `b` should pick up the
    // newest commit while the build of `a` should use the locked old commit.
    let mut a = p.cargo("build").cwd(p.root().join("a")).build_command();
    let mut b = p.cargo("build").cwd(p.root().join("b")).build_command();

    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());

    let a = a.spawn().unwrap();
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    assert_that(a, execs().with_status(0));
    assert_that(b, execs().with_status(0));
});

test!(same_project {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "");
    p.build();

    let mut a = p.cargo("build").build_command();
    let mut b = p.cargo("build").build_command();

    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());

    let a = a.spawn().unwrap();
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    assert_that(a, execs().with_status(0));
    assert_that(b, execs().with_status(0));
});

// Make sure that if Cargo dies while holding a lock that it's released and the
// next Cargo to come in will take over cleanly.
test!(killing_cargo_releases_the_lock {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            build = "build.rs"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("build.rs", r#"
            use std::net::TcpStream;

            fn main() {
                if std::env::var("A").is_ok() {
                    TcpStream::connect(&std::env::var("ADDR").unwrap()[..])
                              .unwrap();
                    std::thread::sleep(std::time::Duration::new(10, 0));
                }
            }
        "#);
    p.build();

    // Our build script will connect to our local TCP socket to inform us that
    // it's started  and that's how we know that `a` will have the lock
    // when we kill it.
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut a = p.cargo("build").build_command();
    let mut b = p.cargo("build").build_command();
    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());
    a.env("ADDR", l.local_addr().unwrap().to_string()).env("A", "a");
    b.env("ADDR", l.local_addr().unwrap().to_string()).env_remove("A");

    // Spawn `a`, wait for it to get to the build script (at which point the
    // lock is held), then kill it.
    let mut a = a.spawn().unwrap();
    l.accept().unwrap();
    a.kill().unwrap();

    // Spawn `b`, then just finish the output of a/b the same way the above
    // tests does.
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    // We killed `a`, so it shouldn't succeed, but `b` should have succeeded.
    assert!(!a.status.success());
    assert_that(b, execs().with_status(0));
});

test!(debug_release_ok {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo("build"), execs().with_status(0));
    fs::remove_dir_all(p.root().join("target")).unwrap();

    let mut a = p.cargo("build").build_command();
    let mut b = p.cargo("build").arg("--release").build_command();
    a.stdout(Stdio::piped()).stderr(Stdio::piped());
    b.stdout(Stdio::piped()).stderr(Stdio::piped());
    let a = a.spawn().unwrap();
    let b = b.spawn().unwrap();
    let a = thread::spawn(move || a.wait_with_output().unwrap());
    let b = b.wait_with_output().unwrap();
    let a = a.join().unwrap();

    assert_that(a, execs().with_status(0).with_stdout("\
[COMPILING] foo v0.0.0 [..]
"));
    assert_that(b, execs().with_status(0).with_stdout("\
[COMPILING] foo v0.0.0 [..]
"));
});
