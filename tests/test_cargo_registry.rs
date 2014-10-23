use std::io::File;

use support::{project, execs, cargo_dir};
use support::{UPDATING, DOWNLOADING, COMPILING, PACKAGING, VERIFYING};
use support::registry as r;

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

    r::mock_pkg("bar", "0.0.1", []);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
{downloading} bar v0.0.1 (the package registry)
{compiling} bar v0.0.1 (the package registry)
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
[..] bar v0.0.1 (the package registry)
[..] foo v0.0.1 ({dir})
",
        updating = UPDATING,
        dir = p.url(),
        reg = r::registry()).as_slice()));
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

    r::mock_pkg("baz", "0.0.1", []);
    r::mock_pkg("bar", "0.0.1", [("baz", "*")]);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `{reg}`
{downloading} [..] v0.0.1 (the package registry)
{downloading} [..] v0.0.1 (the package registry)
{compiling} baz v0.0.1 (the package registry)
{compiling} bar v0.0.1 (the package registry)
{compiling} foo v0.0.1 ({dir})
",
        updating = UPDATING,
        downloading = DOWNLOADING,
        compiling = COMPILING,
        dir = p.url(),
        reg = r::registry()).as_slice()));
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
no package named `nonexistent` found (required by `foo`)
location searched: the package registry
version required: >= 0.0.0
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

    r::mock_pkg("bad-cksum", "0.0.1", []);
    File::create(&r::mock_archive_dst("bad-cksum", "0.0.1")).unwrap();

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
no package named `notyet` found (required by `foo`)
location searched: the package registry
version required: >= 0.0.0
"));

    r::mock_pkg("notyet", "0.0.1", []);

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
        reg = r::registry()).as_slice()));
})

test!(package_with_path_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

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
location searched: the package registry
version required: ^0.0.1
"));

    r::mock_pkg("notyet", "0.0.1", []);

    assert_that(p.process(cargo_dir().join("cargo")).arg("package"),
                execs().with_status(0).with_stdout(format!("\
{packaging} foo v0.0.1 ({dir})
{verifying} foo v0.0.1 ({dir})
{updating} registry `[..]`
{downloading} notyet v0.0.1 (the package registry)
{compiling} notyet v0.0.1 (the package registry)
{compiling} foo v0.0.1 ({dir})
",
    packaging = PACKAGING,
    verifying = VERIFYING,
    updating = UPDATING,
    downloading = DOWNLOADING,
    compiling = COMPILING,
    dir = p.url(),
)));
})
