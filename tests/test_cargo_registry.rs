use std::io::{self, fs, File};
use cargo::util::process;

use support::{project, execs, cargo_dir};
use support::{UPDATING, DOWNLOADING, COMPILING, PACKAGING, VERIFYING};
use support::paths::{self, PathExt};
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
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
{downloading} bar v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = r::registry()).as_slice()));

    // Don't download a second time
    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
[..] bar v0.0.1 (registry file://[..])
[..] foo v0.0.1 ({dir})
",
        updating = UPDATING,
        dir = p.url(),
        reg = r::registry()).as_slice()));
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
                execs().with_status(0).with_stdout(format!("\
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
        reg = r::registry()).as_slice()));
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
no package named `nonexistent` found (required by `foo`)
location searched: registry file://[..]
version required: >= 0.0.0
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
Unable to get packages from source

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
no package named `notyet` found (required by `foo`)
location searched: registry file://[..]
version required: >= 0.0.0
"));

    r::mock_pkg("notyet", "0.0.1", &[]);

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
{downloading} notyet v0.0.1 (registry file://[..])
{compiling} notyet v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = r::registry()).as_slice()));
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("package").arg("-v"),
                execs().with_status(101).with_stderr("\
failed to verify package tarball

Caused by:
  no package named `notyet` found (required by `foo`)
location searched: registry file://[..]
version required: ^0.0.1
"));

    r::mock_pkg("notyet", "0.0.1", &[]);

    assert_that(p.process(cargo_dir().join("cargo")).arg("package"),
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
{downloading} bar v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));

    p.root().move_into_the_past().unwrap();
    r::mock_pkg("bar", "0.0.2", &[]);

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
{downloading} [..] v0.0.1 (registry file://[..])
{downloading} [..] v0.0.1 (registry file://[..])
{compiling} baz v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));

    p.root().move_into_the_past().unwrap();
    r::mock_pkg("baz", "0.0.2", &[]);
    r::mock_pkg("bar", "0.0.2", &[("baz", "*", "normal")]);

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
{downloading} [..] v0.0.1 (registry file://[..])
{downloading} [..] v0.0.1 (registry file://[..])
{compiling} baz v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(101).with_stderr("\
no package named `baz` found (required by `bar`)
location searched: registry file://[..]
version required: = 0.0.2
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));

    fs::rmdir_recursive(&r::registry_path().join("3")).unwrap();

    r::mock_pkg_yank("bar", "0.0.1", &[], true);

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(""));

    assert_that(p.process(cargo_dir().join("cargo")).arg("update"),
                execs().with_status(101).with_stderr("\
no package named `bar` found (required by `foo`)
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
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));
    p.root().move_into_the_past().unwrap();

    fs::rmdir_recursive(&paths::home().join(".cargo/registry")).unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
{downloading} bar v0.0.1 (registry file://[..])
", updating = UPDATING, downloading = DOWNLOADING).as_slice()));
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

    r::mock_pkg("bar", "0.0.1", &[]);
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));

    r::mock_pkg("bar", "0.0.2", &[]);
    fs::rmdir_recursive(&paths::home().join(".cargo/registry")).unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("update")
                 .arg("-p").arg("bar"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
", updating = UPDATING).as_slice()));

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{downloading} [..] v0.0.2 (registry file://[..])
{compiling} bar v0.0.2 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
{downloading} [..] v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));
});

test!(login_with_no_cargo_dir {
    let home = paths::home().join("new-home");
    fs::mkdir(&home, io::USER_DIR).unwrap();
    assert_that(process(cargo_dir().join("cargo")).unwrap()
                       .arg("login").arg("foo").arg("-v")
                       .cwd(paths::root())
                       .env("HOME", Some(home)),
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
    assert_that(p.cargo_process("publish"),
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
{downloading} bar v0.0.1 (registry file://[..])
{compiling} bar v0.0.1 (registry file://[..])
{compiling} a v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));

    File::create(&p.root().join("a/Cargo.toml")).write_str(r#"
        [project]
        name = "a"
        version = "0.0.1"
        authors = []

        [dependencies]
        bar = "0.1.0"
    "#).unwrap();
    r::mock_pkg("bar", "0.1.0", &[]);

    println!("second");
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
{downloading} bar v0.1.0 (registry file://[..])
{compiling} bar v0.1.0 (registry file://[..])
{compiling} a v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));
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
        .file("Cargo.toml", format!(r#"
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
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} [..]
{updating} [..]
{downloading} a v0.0.1 (registry file://[..])
{compiling} a v0.0.1 (registry [..])
{compiling} b v0.0.1 ([..])
{compiling} foo v0.0.1 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));
    p.root().move_into_the_past().unwrap();

    println!("second");
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
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

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));


    r::mock_pkg("a", "0.1.1", &[]);

    let lock = p.root().join("Cargo.lock");
    let s = File::open(&lock).unwrap().read_to_string().unwrap();
    File::create(&lock).unwrap().write_str(s.replace("0.1.0", "0.1.1").as_slice())
                       .unwrap();
    println!("second");

    fs::rmdir_recursive(&p.root().join("target")).unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} [..]
{downloading} a v0.1.1 (registry file://[..])
{compiling} a v0.1.1 (registry [..])
{compiling} foo v0.5.0 ({dir})
", updating = UPDATING, downloading = DOWNLOADING, compiling = COMPILING,
   dir = p.url()).as_slice()));

});
