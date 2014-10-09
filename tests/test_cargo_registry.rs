use std::io::{mod, fs, File};
use url::Url;
use git2;
use serialize::hex::ToHex;

use support::{ResultTest, project, execs, cargo_dir};
use support::{UPDATING, DOWNLOADING, COMPILING};
use support::paths;
use support::git::repo;
use cargo::util::Sha256;

use hamcrest::assert_that;

fn registry_path() -> Path { paths::root().join("registry") }
fn registry() -> Url { Url::from_file_path(&registry_path()).unwrap() }
fn dl_path() -> Path { paths::root().join("dl") }
fn dl_url() -> Url { Url::from_file_path(&dl_path()).unwrap() }

fn cksum(s: &[u8]) -> String {
    let mut sha = Sha256::new();
    sha.update(s);
    sha.finish().to_hex()
}

fn setup() {
    let config = paths::root().join(".cargo/config");
    fs::mkdir_recursive(&config.dir_path(), io::USER_DIR).assert();
    File::create(&config).write_str(format!(r#"
        [registry]
            host = "{reg}"
            token = "api-token"
    "#, reg = registry()).as_slice()).assert();

    // Prepare the "to download" artifacts
    let foo = include_bin!("fixtures/foo-0.0.1.tar.gz");
    let bar = include_bin!("fixtures/bar-0.0.1.tar.gz");
    let notyet = include_bin!("fixtures/notyet-0.0.1.tar.gz");
    let foo_cksum = dl("pkg/foo/foo-0.0.1.tar.gz", foo);
    let bar_cksum = dl("pkg/bar/bar-0.0.1.tar.gz", bar);
    dl("pkg/bad-cksum/bad-cksum-0.0.1.tar.gz", foo);
    let notyet = dl("pkg/notyet/notyet-0.0.1.tar.gz", notyet);

    // Init a new registry
    repo(&registry_path())
        .file("config.json", format!(r#"
            {{"dl":"{}","upload":""}}
        "#, dl_url()).as_slice())
        .file("3/f/foo", pkg("foo", "0.0.1", [], &foo_cksum))
        .file("3/b/bar", pkg("bar", "0.0.1", ["foo||>=0.0.0"], &bar_cksum))
        .file("ba/d-/bad-cksum", pkg("bad-cksum", "0.0.1", [], &bar_cksum))
        .nocommit_file("no/ty/notyet", pkg("notyet", "0.0.1", [], &notyet))
        .build();

    fn pkg(name: &str, vers: &str, deps: &[&str], cksum: &String) -> String {
        let deps: Vec<String> = deps.iter().map(|s| {
            format!("\"{}\"", s)
        }).collect();
        let deps = deps.connect(",");

        format!(r#"{{"name":"{}","vers":"{}","deps":[{}],"cksum":"{}","features":{{}}}}"#,
                name, vers, deps, cksum)
    }
    fn dl(path: &str, contents: &[u8]) -> String {
        let dst = dl_path().join(path);
        fs::mkdir_recursive(&dst.dir_path(), io::USER_DIR).assert();
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

    // Don't download a second time
    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
[..] foo v0.0.1 (the package registry)
[..] foo v0.0.1 ({dir})
",
        updating = UPDATING,
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
    let tree = repo.find_tree(id).unwrap();
    let sig = repo.signature().unwrap();
    let parent = repo.refname_to_id("refs/heads/master").unwrap();
    let parent = repo.find_commit(parent).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig,
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
