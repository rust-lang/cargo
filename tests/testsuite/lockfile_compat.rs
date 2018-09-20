use support::git;
use support::registry::Package;
use support::{basic_manifest, lines_match, project};

#[test]
fn oldest_lockfile_still_works() {
    let cargo_commands = vec!["build", "update"];
    for cargo_command in cargo_commands {
        oldest_lockfile_still_works_with_command(cargo_command);
    }
}

fn oldest_lockfile_still_works_with_command(cargo_command: &str) {
    Package::new("bar", "0.1.0").publish();

    let expected_lockfile = r#"[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[metadata]
"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "[..]"
"#;

    let old_lockfile = r#"[root]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        ).file("src/lib.rs", "")
        .file("Cargo.lock", old_lockfile)
        .build();

    p.cargo(cargo_command).run();

    let lock = p.read_lockfile();
    for (l, r) in expected_lockfile.lines().zip(lock.lines()) {
        assert!(lines_match(l, r), "Lines differ:\n{}\n\n{}", l, r);
    }

    assert_eq!(lock.lines().count(), expected_lockfile.lines().count());
}

#[test]
fn frozen_flag_preserves_old_lockfile() {
    let cksum = Package::new("bar", "0.1.0").publish();

    let old_lockfile = format!(
        r#"[root]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "{}"
"#,
        cksum,
    );

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        ).file("src/lib.rs", "")
        .file("Cargo.lock", &old_lockfile)
        .build();

    p.cargo("build --locked").run();

    let lock = p.read_lockfile();
    for (l, r) in old_lockfile.lines().zip(lock.lines()) {
        assert!(lines_match(l, r), "Lines differ:\n{}\n\n{}", l, r);
    }

    assert_eq!(lock.lines().count(), old_lockfile.lines().count());
}

#[test]
fn totally_wild_checksums_works() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        ).file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum baz 0.1.2 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"checksum bar 0.1.2 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"#,
        );

    let p = p.build();

    p.cargo("build").run();

    let lock = p.read_lockfile();
    assert!(
        lock.starts_with(
            r#"
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[metadata]
"#.trim()
        )
    );
}

#[test]
fn wrong_checksum_is_an_error() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        ).file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"#,
        );

    let p = p.build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
error: checksum for `bar v0.1.0` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g. a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `bar v0.1.0` is the same as when the lockfile was generated

",
        ).run();
}

// If the checksum is unlisted in the lockfile (e.g. <none>) yet we can
// calculate it (e.g. it's a registry dep), then we should in theory just fill
// it in.
#[test]
fn unlisted_checksum_is_bad_if_we_calculate() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        ).file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "<none>"
"#,
        );
    let p = p.build();

    p.cargo("fetch")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
error: checksum for `bar v0.1.0` was not previously calculated, but a checksum \
could now be calculated

this could be indicative of a few possible situations:

    * the source `[..]` did not previously support checksums,
      but was replaced with one that does
    * newer Cargo implementations know how to checksum this source, but this
      older implementation does not
    * the lock file is corrupt

",
        ).run();
}

// If the checksum is listed in the lockfile yet we cannot calculate it (e.g.
// git dependencies as of today), then make sure we choke.
#[test]
fn listed_checksum_bad_if_we_cannot_compute() {
    let git = git::new("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
            .file("src/lib.rs", "")
    }).unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = {{ git = '{}' }}
        "#,
                git.url()
            ),
        ).file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            &format!(
                r#"
[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (git+{0})"
]

[[package]]
name = "bar"
version = "0.1.0"
source = "git+{0}"

[metadata]
"checksum bar 0.1.0 (git+{0})" = "checksum"
"#,
                git.url()
            ),
        );

    let p = p.build();

    p.cargo("fetch")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] git repository `[..]`
error: checksum for `bar v0.1.0 ([..])` could not be calculated, but a \
checksum is listed in the existing lock file[..]

this could be indicative of a few possible situations:

    * the source `[..]` supports checksums,
      but was replaced with one that doesn't
    * the lock file is corrupt

unable to verify that `bar v0.1.0 ([..])` is the same as when the lockfile was generated

",
        ).run();
}

#[test]
fn current_lockfile_format() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        ).file("src/lib.rs", "");
    let p = p.build();

    p.cargo("build").run();

    let actual = p.read_lockfile();

    let expected = "\
[[package]]
name = \"bar\"
version = \"0.1.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"

[[package]]
name = \"foo\"
version = \"0.0.1\"
dependencies = [
 \"bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)\",
]

[metadata]
\"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)\" = \"[..]\"";

    for (l, r) in expected.lines().zip(actual.lines()) {
        assert!(lines_match(l, r), "Lines differ:\n{}\n\n{}", l, r);
    }

    assert_eq!(actual.lines().count(), expected.lines().count());
}

#[test]
fn lockfile_without_root() {
    Package::new("bar", "0.1.0").publish();

    let lockfile = r#"[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]
"#;

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        ).file("src/lib.rs", "")
        .file("Cargo.lock", lockfile);

    let p = p.build();

    p.cargo("build").run();

    let lock = p.read_lockfile();
    assert!(lock.starts_with(lockfile.trim()));
}

#[test]
fn locked_correct_error() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        ).file("src/lib.rs", "");
    let p = p.build();

    p.cargo("build --locked")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
error: the lock file [CWD]/Cargo.lock needs to be updated but --locked was passed to prevent this
",
        ).run();
}
