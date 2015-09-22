use std::fs::{self, File};
use std::io::prelude::*;
use cargo::util::process;

use support::{project, execs, cargo_dir};
use support::{UPDATING, DOWNLOADING, COMPILING, PACKAGING, VERIFYING, ADDING, REMOVING};
use support::paths::{self, CargoPathExt};
use support::registry as r;
use support::git;

use hamcrest::assert_that;

fn setup() {
    r::init();
}

test!(simple {
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

    r::mock_pkg("bar", "0.0.1", &[]);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `{reg}`
{downloading} bar v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = r::registry())));

    // Don't download a second time
    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `{reg}`
[..] bar v0.0.1 (registry file://[..])
[..] foo v0.0.1 ({dir})
",
        updating = UPDATING,
        dir = p.url(),
        reg = r::registry())));
});

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

    r::mock_pkg("baz", "0.0.1", &[]);
    r::mock_pkg("bar", "0.0.1", &[("baz", "*", "normal")]);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `{reg}`
{downloading} [..] v0.0.1 (registry file://[..])
{downloading} [..] v0.0.1 (registry file://[..])
{compiling} baz v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = r::registry())));
});

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
no matching package named `nonexistent` found (required by `foo`)
location searched: registry file://[..]
version required: >= 0.0.0
"));
});

test!(wrong_version {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = ">= 1.0.0"
        "#)
        .file("src/main.rs", "fn main() {}");

    r::mock_pkg("foo", "0.0.1", &[]);
    r::mock_pkg("foo", "0.0.2", &[]);

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
no matching package named `foo` found (required by `foo`)
location searched: registry file://[..]
version required: >= 1.0.0
versions found: 0.0.2, 0.0.1
"));

    r::mock_pkg("foo", "0.0.3", &[]);
    r::mock_pkg("foo", "0.0.4", &[]);

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
no matching package named `foo` found (required by `foo`)
location searched: registry file://[..]
version required: >= 1.0.0
versions found: 0.0.4, 0.0.3, 0.0.2, ...
"));
});

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

    r::mock_pkg("bad-cksum", "0.0.1", &[]);
    File::create(&r::mock_archive_dst("bad-cksum", "0.0.1")).unwrap();

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
unable to get packages from source

Caused by:
  Failed to download package `bad-cksum v0.0.1 (registry file://[..])` from [..]

Caused by:
  Failed to verify the checksum of `bad-cksum v0.0.1 (registry file://[..])`
"));
});

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
no matching package named `notyet` found (required by `foo`)
location searched: registry file://[..]
version required: >= 0.0.0
"));

    r::mock_pkg("notyet", "0.0.1", &[]);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `{reg}`
{downloading} notyet v0.0.1 (registry file://[..])
{compiling} notyet v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = r::registry())));
});

test!(package_with_path_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            repository = "bar"

            [dependencies.notyet]
            version = "0.0.1"
            path = "notyet"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("notyet/Cargo.toml", r#"
            [package]
            name = "notyet"
            version = "0.0.1"
            authors = []
        "#)
        .file("notyet/src/lib.rs", "");
    p.build();

    assert_that(p.cargo("package").arg("-v"),
                execs().with_status(101).with_stderr("\
failed to verify package tarball

Caused by:
  no matching package named `notyet` found (required by `foo`)
location searched: registry file://[..]
version required: ^0.0.1
"));

    r::mock_pkg("notyet", "0.0.1", &[]);

    assert_that(p.cargo("package"),
                execs().with_status(0).with_stdout(format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{updating} registry `[..]`
{downloading} notyet v0.0.1 (registry file://[..])
{compiling} notyet v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir}[..])
",
    packaging = PACKAGING,
    verifying = VERIFYING,
    updating = UPDATING,
    downloading = DOWNLOADING,
    compiling = COMPILING,
    dir = p.url(),
)));
});

test!(lockfile_locks {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("bar", "0.0.1", &[]);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{downloading} bar v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));

    p.root().move_into_the_past().unwrap();
    r::mock_pkg("bar", "0.0.2", &[]);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(""));
});

test!(lockfile_locks_transitively {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("baz", "0.0.1", &[]);
    r::mock_pkg("bar", "0.0.1", &[("baz", "*", "normal")]);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{downloading} [..] v0.0.1 (registry file://[..])
{downloading} [..] v0.0.1 (registry file://[..])
{compiling} baz v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));

    p.root().move_into_the_past().unwrap();
    r::mock_pkg("baz", "0.0.2", &[]);
    r::mock_pkg("bar", "0.0.2", &[("baz", "*", "normal")]);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(""));
});

test!(yanks_are_not_used {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("baz", "0.0.1", &[]);
    r::mock_pkg_yank("baz", "0.0.2", &[], true);
    r::mock_pkg("bar", "0.0.1", &[("baz", "*", "normal")]);
    r::mock_pkg_yank("bar", "0.0.2", &[("baz", "*", "normal")], true);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{downloading} [..] v0.0.1 (registry file://[..])
{downloading} [..] v0.0.1 (registry file://[..])
{compiling} baz v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));
});

test!(relying_on_a_yank_is_bad {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("baz", "0.0.1", &[]);
    r::mock_pkg_yank("baz", "0.0.2", &[], true);
    r::mock_pkg("bar", "0.0.1", &[("baz", "=0.0.2", "normal")]);

    assert_that(p.cargo("build"),
                execs().with_status(101).with_stderr("\
no matching package named `baz` found (required by `bar`)
location searched: registry file://[..]
version required: = 0.0.2
versions found: 0.0.1
"));
});

test!(yanks_in_lockfiles_are_ok {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("bar", "0.0.1", &[]);

    assert_that(p.cargo("build"),
                execs().with_status(0));

    fs::remove_dir_all(&r::registry_path().join("3")).unwrap();

    r::mock_pkg_yank("bar", "0.0.1", &[], true);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(""));

    assert_that(p.cargo("update"),
                execs().with_status(101).with_stderr("\
no matching package named `bar` found (required by `foo`)
location searched: registry file://[..]
version required: *
"));
});

test!(update_with_lockfile_if_packages_missing {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("bar", "0.0.1", &[]);
    assert_that(p.cargo("build"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    paths::home().join(".cargo/registry").rm_rf().unwrap();
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{downloading} bar v0.0.1 (registry file://[..])
", updating = UPDATING, downloading = DOWNLOADING)));
});

test!(update_lockfile {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    println!("0.0.1");
    r::mock_pkg("bar", "0.0.1", &[]);
    assert_that(p.cargo("build"),
                execs().with_status(0));

    r::mock_pkg("bar", "0.0.2", &[]);
    r::mock_pkg("bar", "0.0.3", &[]);
    paths::home().join(".cargo/registry").rm_rf().unwrap();
    println!("0.0.2 update");
    assert_that(p.cargo("update")
                 .arg("-p").arg("bar").arg("--precise").arg("0.0.2"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{updating} bar v0.0.1 (registry file://[..]) -> v0.0.2
", updating = UPDATING)));

    println!("0.0.2 build");
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{downloading} [..] v0.0.2 (registry file://[..])
{compiling} bar v0.0.2 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));

    println!("0.0.3 update");
    assert_that(p.cargo("update")
                 .arg("-p").arg("bar"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{updating} bar v0.0.2 (registry file://[..]) -> v0.0.3
", updating = UPDATING)));

    println!("0.0.3 build");
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{downloading} [..] v0.0.3 (registry file://[..])
{compiling} bar v0.0.3 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));

   println!("new dependencies update");
   r::mock_pkg("bar", "0.0.4", &[("spam", "0.2.5", "")]);
   r::mock_pkg("spam", "0.2.5", &[]);
   assert_that(p.cargo("update")
                .arg("-p").arg("bar"),
               execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{updating} bar v0.0.3 (registry file://[..]) -> v0.0.4
{adding} spam v0.2.5 (registry file://[..])
", updating = UPDATING, adding = ADDING)));

   println!("new dependencies update");
   r::mock_pkg("bar", "0.0.5", &[]);
   assert_that(p.cargo("update")
                .arg("-p").arg("bar"),
               execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{updating} bar v0.0.4 (registry file://[..]) -> v0.0.5
{removing} spam v0.2.5 (registry file://[..])
", updating = UPDATING, removing = REMOVING)));
});

test!(dev_dependency_not_used {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("baz", "0.0.1", &[]);
    r::mock_pkg("bar", "0.0.1", &[("baz", "*", "dev")]);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{downloading} [..] v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));
});

test!(login_with_no_cargo_dir {
    let home = paths::home().join("new-home");
    fs::create_dir(&home).unwrap();
    assert_that(process(&cargo_dir().join("cargo")).unwrap()
                       .arg("login").arg("foo").arg("-v")
                       .cwd(&paths::root())
                       .env("HOME", &home),
                execs().with_status(0));
});

test!(bad_license_file {
    let p = project("all")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license-file = "foo"
            description = "bar"
            repository = "baz"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("publish").arg("-v"),
                execs().with_status(101)
                       .with_stderr("\
the license file `foo` does not exist"));
});

test!(updating_a_dep {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.a]
            path = "a"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
        "#)
        .file("a/src/lib.rs", "");
    p.build();

    r::mock_pkg("bar", "0.0.1", &[]);

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{downloading} bar v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} a v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));

    File::create(&p.root().join("a/Cargo.toml")).unwrap().write_all(br#"
        [project]
        name = "a"
        version = "0.0.1"
        authors = []

        [dependencies]
        bar = "0.1.0"
    "#).unwrap();
    r::mock_pkg("bar", "0.1.0", &[]);

    println!("second");
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} registry `[..]`
{downloading} bar v0.1.0 (registry file://[..])
{compiling} bar v0.1.0 (registry file://[..])
{compiling} a v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));
});

test!(git_and_registry_dep {
    let b = git::repo(&paths::root().join("b"))
        .file("Cargo.toml", r#"
            [project]
            name = "b"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = "0.0.1"
        "#)
        .file("src/lib.rs", "");
    b.build();
    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = "0.0.1"

            [dependencies.b]
            git = '{}'
        "#, b.url()))
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("a", "0.0.1", &[]);

    p.root().move_into_the_past().unwrap();
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} [..]
{updating} [..]
{downloading} a v0.0.1 (registry file://[..])
{compiling} a v0.0.1 (registry [..])
{compiling} b v0.0.1 ([..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));
    p.root().move_into_the_past().unwrap();

    println!("second");
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(""));
});

test!(update_publish_then_update {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            a = "0.1.0"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("a", "0.1.0", &[]);

    assert_that(p.cargo("build"),
                execs().with_status(0));


    r::mock_pkg("a", "0.1.1", &[]);

    let lock = p.root().join("Cargo.lock");
    let mut s = String::new();
    File::open(&lock).unwrap().read_to_string(&mut s).unwrap();
    File::create(&lock).unwrap()
         .write_all(s.replace("0.1.0", "0.1.1").as_bytes()).unwrap();
    println!("second");

    fs::remove_dir_all(&p.root().join("target")).unwrap();
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stdout(&format!("\
{updating} [..]
{downloading} a v0.1.1 (registry file://[..])
{compiling} a v0.1.1 (registry [..])
{compiling} foo v0.5.0 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url())));

});

test!(fetch_downloads {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            a = "0.1.0"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("a", "0.1.0", &[]);

    assert_that(p.cargo("fetch"),
                execs().with_status(0)
                       .with_stdout(format!("\
{updating} registry `[..]`
{downloading} a v0.1.0 (registry [..])
", updating = UPDATING, downloading = DOWNLOADING)));
});

test!(update_transitive_dependency {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            a = "0.1.0"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("a", "0.1.0", &[("b", "*", "normal")]);
    r::mock_pkg("b", "0.1.0", &[]);

    assert_that(p.cargo("fetch"),
                execs().with_status(0));

    r::mock_pkg("b", "0.1.1", &[]);

    assert_that(p.cargo("update").arg("-pb"),
                execs().with_status(0)
                       .with_stdout(format!("\
{updating} registry `[..]`
{updating} b v0.1.0 (registry [..]) -> v0.1.1
", updating = UPDATING)));

    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{downloading} b v0.1.1 (registry file://[..])
{compiling} b v0.1.1 (registry [..])
{compiling} a v0.1.0 (registry [..])
{compiling} foo v0.5.0 ([..])
", downloading = DOWNLOADING, compiling = COMPILING)));
});

test!(update_backtracking_ok {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = []

            [dependencies]
            webdriver = "0.1"
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    r::mock_pkg("webdriver", "0.1.0", &[("hyper", "0.6", "normal")]);
    r::mock_pkg("hyper", "0.6.5", &[("openssl", "0.1", "normal"),
                                    ("cookie", "0.1", "normal")]);
    r::mock_pkg("cookie", "0.1.0", &[("openssl", "0.1", "normal")]);
    r::mock_pkg("openssl", "0.1.0", &[]);

    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));

    r::mock_pkg("openssl", "0.1.1", &[]);
    r::mock_pkg("hyper", "0.6.6", &[("openssl", "0.1.1", "normal"),
                                    ("cookie", "0.1.0", "normal")]);

    assert_that(p.cargo("update").arg("-p").arg("hyper"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{updating} registry `[..]`
", updating = UPDATING)));
});
