extern crate cargotest;
extern crate hamcrest;

use cargotest::support::git;
use cargotest::support::registry::Package;
use cargotest::support::{execs, project, lines_match};
use hamcrest::assert_that;

#[test]
fn oldest_lockfile_still_works() {
    Package::new("foo", "0.1.0").publish();

    let expected_lockfile =
r#"[[package]]
name = "bar"
version = "0.0.1"
dependencies = [
 "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "[..]"
"#;

    let old_lockfile = r#"
[root]
name = "bar"
version = "0.0.1"
dependencies = [
 "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "")
        .file("Cargo.lock", old_lockfile);
    p.build();

    let cargo_commands = vec![
        "build",
        "update"
    ];

    for cargo_command in cargo_commands {
        assert_that(p.cargo(cargo_command),
                    execs().with_status(0));

        let lock = p.read_lockfile();
        for (l, r) in expected_lockfile.lines().zip(lock.lines()) {
            assert!(lines_match(l, r), "Lines differ:\n{}\n\n{}", l, r);
        }

        assert_eq!(lock.lines().count(), expected_lockfile.lines().count());
    }
}

#[test]
fn totally_wild_checksums_works() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "")
        .file("Cargo.lock", r#"
[[package]]
name = "bar"
version = "0.0.1"
dependencies = [
 "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum baz 0.1.2 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"checksum foo 0.1.2 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"#);

    p.build();

    assert_that(p.cargo("build"),
                execs().with_status(0));

    let lock = p.read_lockfile();
    assert!(lock.starts_with(r#"
[[package]]
name = "bar"
version = "0.0.1"
dependencies = [
 "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"#.trim()));
}

#[test]
fn wrong_checksum_is_an_error() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "")
        .file("Cargo.lock", r#"
[[package]]
name = "bar"
version = "0.0.1"
dependencies = [
 "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"#);

    p.build();

    assert_that(p.cargo("build"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry `[..]`
error: checksum for `foo v0.1.0` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g. a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `foo v0.1.0` is the same as when the lockfile was generated

"));
}

// If the checksum is unlisted in the lockfile (e.g. <none>) yet we can
// calculate it (e.g. it's a registry dep), then we should in theory just fill
// it in.
#[test]
fn unlisted_checksum_is_bad_if_we_calculate() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "")
        .file("Cargo.lock", r#"
[[package]]
name = "bar"
version = "0.0.1"
dependencies = [
 "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "<none>"
"#);
    p.build();

    assert_that(p.cargo("fetch"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry `[..]`
error: checksum for `foo v0.1.0` was not previously calculated, but a checksum \
could now be calculated

this could be indicative of a few possible situations:

    * the source `[..]` did not previously support checksums,
      but was replaced with one that does
    * newer Cargo implementations know how to checksum this source, but this
      older implementation does not
    * the lock file is corrupt

"));
}

// If the checksum is listed in the lockfile yet we cannot calculate it (e.g.
// git dependencies as of today), then make sure we choke.
#[test]
fn listed_checksum_bad_if_we_cannot_compute() {
    let git = git::new("foo", |p| {
        p.file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
    }).unwrap();

    let p = project("bar")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = {{ git = '{}' }}
        "#, git.url()))
        .file("src/lib.rs", "")
        .file("Cargo.lock", &format!(r#"
[[package]]
name = "bar"
version = "0.0.1"
dependencies = [
 "foo 0.1.0 (git+{0})"
]

[[package]]
name = "foo"
version = "0.1.0"
source = "git+{0}"

[metadata]
"checksum foo 0.1.0 (git+{0})" = "checksum"
"#, git.url()));

    p.build();

    assert_that(p.cargo("fetch"),
                execs().with_status(101).with_stderr("\
[UPDATING] git repository `[..]`
error: checksum for `foo v0.1.0 ([..])` could not be calculated, but a \
checksum is listed in the existing lock file[..]

this could be indicative of a few possible situations:

    * the source `[..]` supports checksums,
      but was replaced with one that doesn't
    * the lock file is corrupt

unable to verify that `foo v0.1.0 ([..])` is the same as when the lockfile was generated

"));
}

#[test]
fn current_lockfile_format() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build"), execs().with_status(0));

    let actual = p.read_lockfile();

    let expected = "\
[[package]]
name = \"bar\"
version = \"0.0.1\"
dependencies = [
 \"foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)\",
]

[[package]]
name = \"foo\"
version = \"0.1.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"

[metadata]
\"checksum foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)\" = \"[..]\"";

    for (l, r) in expected.lines().zip(actual.lines()) {
        assert!(lines_match(l, r), "Lines differ:\n{}\n\n{}", l, r);
    }

    assert_eq!(actual.lines().count(), expected.lines().count());
}

#[test]
fn lockfile_without_root() {
    Package::new("foo", "0.1.0").publish();

    let lockfile = r#"[[package]]
name = "bar"
version = "0.0.1"
dependencies = [
 "foo 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "")
        .file("Cargo.lock", lockfile);

    p.build();

    assert_that(p.cargo("build"), execs().with_status(0));

    let lock = p.read_lockfile();
    assert!(lock.starts_with(lockfile.trim()));
}

#[test]
fn locked_correct_error() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build").arg("--locked"),
                execs().with_status(101).with_stderr("\
[UPDATING] registry `[..]`
error: the lock file needs to be updated but --locked was passed to prevent this
"));
}
