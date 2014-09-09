use std::io::{mod, fs, File};
use url::Url;
use git2;
use serialize::hex::ToHex;

use support::{ResultTest, project, execs, cargo_dir};
use support::{UPDATING, DOWNLOADING, COMPILING};
use support::paths;
use cargo::util::Sha256;

use hamcrest::assert_that;

fn registry_path() -> Path { paths::root().join("registry") }
fn registry() -> Url { Url::from_file_path(&registry_path()).unwrap() }
fn dl_path() -> Path { paths::root().join("dl") }
fn dl_url() -> Url { Url::from_file_path(&dl_path()).unwrap() }

fn cksum(s: &[u8]) -> String {
    let mut sha = Sha256::new();
    sha.update(s);
    sha.final().to_hex()
}

fn setup() {
    let config = paths::root().join(".cargo/config");
    fs::mkdir_recursive(&config.dir_path(), io::UserDir).assert();
    File::create(&config).write_str(format!(r#"
        [registry]
            host = "{reg}"
            token = "api-token"
    "#, reg = registry()).as_slice()).assert();

    fs::mkdir(&registry_path(), io::UserDir).assert();

    // Init a new registry
    let repo = git2::Repository::init(&registry_path()).unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "name").unwrap();
    config.set_str("user.email", "email").unwrap();
    let mut index = repo.index().unwrap();

    // Prepare the "to download" artifacts
    let foo = include_bin!("fixtures/foo-0.0.1.tar.gz");
    let bar = include_bin!("fixtures/bar-0.0.1.tar.gz");
    let notyet = include_bin!("fixtures/notyet-0.0.1.tar.gz");
    let foo_cksum = dl("pkg/foo/foo-0.0.1.tar.gz", foo);
    let bar_cksum = dl("pkg/bar/bar-0.0.1.tar.gz", bar);
    dl("pkg/bad-cksum/bad-cksum-0.0.1.tar.gz", foo);
    let notyet = dl("pkg/notyet/notyet-0.0.1.tar.gz", notyet);

    // Prepare the registry's git repo
    file(&mut index, "config.json", format!(r#"
        {{"dl_url":"{}"}}
    "#, dl_url()).as_slice());
    file(&mut index, "fo/oX/foo",
         format!(r#"{{"name":"foo","vers":"0.0.1","deps":[],"cksum":"{}"}}"#,
                 foo_cksum).as_slice());
    file(&mut index, "ba/rX/bar",
         format!(r#"{{"name":"bar","vers":"0.0.1","deps":["foo|>=0.0.0"],"cksum":"{}"}}"#,
                 bar_cksum).as_slice());
    file(&mut index, "ba/d-/bad-cksum",
         format!(r#"{{"name":"bad-cksum","vers":"0.0.1","deps":[],"cksum":"{}"}}"#,
                 bar_cksum).as_slice());
    file(&mut index, "no/ty/notyet",
         format!(r#"{{"name":"notyet","vers":"0.0.1","deps":[],"cksum":"{}"}}"#,
                 notyet).as_slice());
    index.remove_path(&Path::new("no/ty/notyet")).unwrap();

    // Commit!
    index.write().unwrap();
    let id = index.write_tree().unwrap();
    let tree = git2::Tree::lookup(&repo, id).unwrap();
    let sig = git2::Signature::default(&repo).unwrap();
    git2::Commit::new(&repo, Some("HEAD"), &sig, &sig,
                      "Initial commit", &tree, []).unwrap();

    fn file(index: &mut git2::Index, path: &str, contents: &str) {
        let dst = index.path().unwrap().dir_path().dir_path().join(path);
        fs::mkdir_recursive(&dst.dir_path(), io::UserDir).assert();
        File::create(&dst).write_str(contents).unwrap();
        index.add_path(&Path::new(path)).unwrap();
    }

    fn dl(path: &str, contents: &[u8]) -> String {
        let dst = dl_path().join(path);
        fs::mkdir_recursive(&dst.dir_path(), io::UserDir).assert();
        File::create(&dst).write(contents).unwrap();
        cksum(contents)
    }
}

test!(simple {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = ">= 0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
{downloading} foo v0.0.1 (the package registry)
{compiling} foo v0.0.1 (the package registry)
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = registry()).as_slice()));
})

test!(deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = ">= 0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
{downloading} [..] v0.0.1 (the package registry)
{downloading} [..] v0.0.1 (the package registry)
{compiling} foo v0.0.1 (the package registry)
{compiling} bar v0.0.1 (the package registry)
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = registry()).as_slice()));
})

test!(nonexistent {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            nonexistent = ">= 0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
No package named `nonexistent` found (required by `foo`).
Location searched: the package registry
Version required: >= 0.0.0
"));
})

test!(bad_cksum {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bad-cksum = ">= 0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
Unable to get packages from source

Caused by:
  Failed to download package `bad-cksum v0.0.1 (the package registry)` from [..]

Caused by:
  Failed to verify the checksum of `bad-cksum v0.0.1 (the package registry)`
"));
})

test!(update_registry {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            notyet = ">= 0.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
No package named `notyet` found (required by `foo`).
Location searched: the package registry
Version required: >= 0.0.0
"));

    // Add the package and commit
    let repo = git2::Repository::open(&registry_path()).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(&Path::new("no/ty/notyet")).unwrap();
    let id = index.write_tree().unwrap();
    let tree = git2::Tree::lookup(&repo, id).unwrap();
    let sig = git2::Signature::default(&repo).unwrap();
    let parent = git2::Reference::name_to_id(&repo, "refs/heads/master").unwrap();
    let parent = git2::Commit::lookup(&repo, parent).unwrap();
    git2::Commit::new(&repo, Some("HEAD"), &sig, &sig,
                      "Another commit", &tree,
                      [&parent]).unwrap();

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
{downloading} notyet v0.0.1 (the package registry)
{compiling} notyet v0.0.1 (the package registry)
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = registry()).as_slice()));
})
