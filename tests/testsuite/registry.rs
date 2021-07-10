//! Tests for normal registry dependencies.

use cargo::core::SourceId;
use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::{self, registry_path, Dependency, Package};
use cargo_test_support::{basic_manifest, project};
use cargo_test_support::{cargo_process, registry::registry_url};
use cargo_test_support::{git, install::cargo_home, t};
use cargo_util::paths::remove_dir_all;
use std::fs::{self, File};
use std::path::Path;

#[cargo_test]
fn simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry_path().to_str().unwrap()
        ))
        .run();

    p.cargo("clean").run();

    // Don't download a second time
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dep("baz", "*").publish();

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry_path().to_str().unwrap()
        ))
        .run();
}

#[cargo_test]
fn nonexistent() {
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                nonexistent = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
error: no matching package named `nonexistent` found
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        )
        .run();
}

#[cargo_test]
fn wrong_case() {
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                Init = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // #5678 to make this work
    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
error: no matching package found
searched package name: `Init`
perhaps you meant:      init
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        )
        .run();
}

#[cargo_test]
fn mis_hyphenated() {
    Package::new("mis-hyphenated", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                mis_hyphenated = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // #2775 to make this work
    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
error: no matching package found
searched package name: `mis_hyphenated`
perhaps you meant:      mis-hyphenated
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        )
        .run();
}

#[cargo_test]
fn wrong_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                foo = ">= 1.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.0.1").publish();
    Package::new("foo", "0.0.2").publish();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "\
error: failed to select a version for the requirement `foo = \">=1.0.0\"`
candidate versions found which didn't match: 0.0.2, 0.0.1
location searched: `[..]` index (which is replacing registry `[..]`)
required by package `foo v0.0.1 ([..])`
",
        )
        .run();

    Package::new("foo", "0.0.3").publish();
    Package::new("foo", "0.0.4").publish();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "\
error: failed to select a version for the requirement `foo = \">=1.0.0\"`
candidate versions found which didn't match: 0.0.4, 0.0.3, 0.0.2, ...
location searched: `[..]` index (which is replacing registry `[..]`)
required by package `foo v0.0.1 ([..])`
",
        )
        .run();
}

#[cargo_test]
fn bad_cksum() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bad-cksum = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let pkg = Package::new("bad-cksum", "0.0.1");
    pkg.publish();
    t!(File::create(&pkg.archive_dst()));

    p.cargo("build -v")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
[DOWNLOADING] crates ...
[DOWNLOADED] bad-cksum [..]
[ERROR] failed to download replaced source registry `https://[..]`

Caused by:
  failed to verify the checksum of `bad-cksum v0.0.1 (registry `[ROOT][..]`)`
",
        )
        .run();
}

#[cargo_test]
fn update_registry() {
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                notyet = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "\
error: no matching package named `notyet` found
location searched: registry `[..]`
required by package `foo v0.0.1 ([..])`
",
        )
        .run();

    Package::new("notyet", "0.0.1").publish();

    p.cargo("build")
        .with_stderr(format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] notyet v0.0.1 (registry `[ROOT][..]`)
[COMPILING] notyet v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry_path().to_str().unwrap()
        ))
        .run();
}

#[cargo_test]
fn package_with_path_deps() {
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("notyet/Cargo.toml", &basic_manifest("notyet", "0.0.1"))
        .file("notyet/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr_contains(
            "\
[PACKAGING] foo [..]
[UPDATING] [..]
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `notyet` found
  location searched: registry `https://github.com/rust-lang/crates.io-index`
  required by package `foo v0.0.1 [..]`
",
        )
        .run();

    Package::new("notyet", "0.0.1").publish();

    p.cargo("package")
        .with_stderr(
            "\
[PACKAGING] foo v0.0.1 ([CWD])
[UPDATING] `[..]` index
[VERIFYING] foo v0.0.1 ([CWD])
[DOWNLOADING] crates ...
[DOWNLOADED] notyet v0.0.1 (registry `[ROOT][..]`)
[COMPILING] notyet v0.0.1
[COMPILING] foo v0.0.1 ([CWD][..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn lockfile_locks() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    p.root().move_into_the_past();
    Package::new("bar", "0.0.2").publish();

    p.cargo("build").with_stdout("").run();
}

#[cargo_test]
fn lockfile_locks_transitively() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dep("baz", "*").publish();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    p.root().move_into_the_past();
    Package::new("baz", "0.0.2").publish();
    Package::new("bar", "0.0.2").dep("baz", "*").publish();

    p.cargo("build").with_stdout("").run();
}

#[cargo_test]
fn yanks_are_not_used() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("baz", "0.0.2").yanked(true).publish();
    Package::new("bar", "0.0.1").dep("baz", "*").publish();
    Package::new("bar", "0.0.2")
        .dep("baz", "*")
        .yanked(true)
        .publish();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn relying_on_a_yank_is_bad() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("baz", "0.0.2").yanked(true).publish();
    Package::new("bar", "0.0.1").dep("baz", "=0.0.2").publish();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "\
error: failed to select a version for the requirement `baz = \"=0.0.2\"`
candidate versions found which didn't match: 0.0.1
location searched: `[..]` index (which is replacing registry `[..]`)
required by package `bar v0.0.1`
    ... which is depended on by `foo [..]`
",
        )
        .run();
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("build").run();

    registry_path().join("3").rm_rf();

    Package::new("bar", "0.0.1").yanked(true).publish();

    p.cargo("build").with_stdout("").run();

    p.cargo("update")
        .with_status(101)
        .with_stderr_contains(
            "\
error: no matching package named `bar` found
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        )
        .run();
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok_for_other_update() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
                baz = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();
    Package::new("baz", "0.0.1").publish();

    p.cargo("build").run();

    registry_path().join("3").rm_rf();

    Package::new("bar", "0.0.1").yanked(true).publish();
    Package::new("baz", "0.0.1").publish();

    p.cargo("build").with_stdout("").run();

    Package::new("baz", "0.0.2").publish();

    p.cargo("update")
        .with_status(101)
        .with_stderr_contains(
            "\
error: no matching package named `bar` found
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        )
        .run();

    p.cargo("update -p baz")
        .with_stderr_contains(
            "\
[UPDATING] `[..]` index
[UPDATING] baz v0.0.1 -> v0.0.2
",
        )
        .run();
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok_with_new_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("build").run();

    registry_path().join("3").rm_rf();

    Package::new("bar", "0.0.1").yanked(true).publish();
    Package::new("baz", "0.0.1").publish();

    p.change_file(
        "Cargo.toml",
        r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "*"
            baz = "*"
        "#,
    );

    p.cargo("build").with_stdout("").run();
}

#[cargo_test]
fn update_with_lockfile_if_packages_missing() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();
    p.cargo("build").run();
    p.root().move_into_the_past();

    paths::home().join(".cargo/registry").rm_rf();
    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn update_lockfile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    println!("0.0.1");
    Package::new("bar", "0.0.1").publish();
    p.cargo("build").run();

    Package::new("bar", "0.0.2").publish();
    Package::new("bar", "0.0.3").publish();
    paths::home().join(".cargo/registry").rm_rf();
    println!("0.0.2 update");
    p.cargo("update -p bar --precise 0.0.2")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] bar v0.0.1 -> v0.0.2
",
        )
        .run();

    println!("0.0.2 build");
    p.cargo("build")
        .with_stderr(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.2 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.2
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    println!("0.0.3 update");
    p.cargo("update -p bar")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] bar v0.0.2 -> v0.0.3
",
        )
        .run();

    println!("0.0.3 build");
    p.cargo("build")
        .with_stderr(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.3 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.3
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    println!("new dependencies update");
    Package::new("bar", "0.0.4").dep("spam", "0.2.5").publish();
    Package::new("spam", "0.2.5").publish();
    p.cargo("update -p bar")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] bar v0.0.3 -> v0.0.4
[ADDING] spam v0.2.5
",
        )
        .run();

    println!("new dependencies update");
    Package::new("bar", "0.0.5").publish();
    p.cargo("update -p bar")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] bar v0.0.4 -> v0.0.5
[REMOVING] spam v0.2.5
",
        )
        .run();
}

#[cargo_test]
fn dev_dependency_not_used() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dev_dep("baz", "*").publish();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn login_with_no_cargo_dir() {
    // Create a config in the root directory because `login` requires the
    // index to be updated, and we don't want to hit crates.io.
    registry::init();
    fs::rename(paths::home().join(".cargo"), paths::root().join(".cargo")).unwrap();
    paths::home().rm_rf();
    cargo_process("login foo -v").run();
    let credentials = fs::read_to_string(paths::home().join(".cargo/credentials")).unwrap();
    assert_eq!(credentials, "[registry]\ntoken = \"foo\"\n");
}

#[cargo_test]
fn login_with_differently_sized_token() {
    // Verify that the configuration file gets properly truncated.
    registry::init();
    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();
    cargo_process("login lmaolmaolmao -v").run();
    cargo_process("login lmao -v").run();
    cargo_process("login lmaolmaolmao -v").run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert_eq!(credentials, "[registry]\ntoken = \"lmaolmaolmao\"\n");
}

#[cargo_test]
fn bad_license_file() {
    Package::new("foo", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license-file = "foo"
                description = "bar"
                repository = "baz"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("publish -v --token sekrit")
        .with_status(101)
        .with_stderr_contains("[ERROR] the license file `foo` does not exist")
        .run();
}

#[cargo_test]
fn updating_a_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1
[COMPILING] a v0.0.1 ([CWD]/a)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    p.change_file(
        "a/Cargo.toml",
        r#"
        [project]
        name = "a"
        version = "0.0.1"
        authors = []

        [dependencies]
        bar = "0.1.0"
        "#,
    );
    Package::new("bar", "0.1.0").publish();

    println!("second");
    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 (registry `[ROOT][..]`)
[COMPILING] bar v0.1.0
[COMPILING] a v0.0.1 ([CWD]/a)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn git_and_registry_dep() {
    let b = git::repo(&paths::root().join("b"))
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "b"
                version = "0.0.1"
                authors = []

                [dependencies]
                a = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
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
                    a = "0.0.1"

                    [dependencies.b]
                    git = '{}'
                "#,
                b.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("a", "0.0.1").publish();

    p.root().move_into_the_past();
    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] [..]
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.0.1 (registry `[ROOT][..]`)
[COMPILING] a v0.0.1
[COMPILING] b v0.0.1 ([..])
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
    p.root().move_into_the_past();

    println!("second");
    p.cargo("build").with_stdout("").run();
}

#[cargo_test]
fn update_publish_then_update() {
    // First generate a Cargo.lock and a clone of the registry index at the
    // "head" of the current registry.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    Package::new("a", "0.1.0").publish();
    p.cargo("build").run();

    // Next, publish a new package and back up the copy of the registry we just
    // created.
    Package::new("a", "0.1.1").publish();
    let registry = paths::home().join(".cargo/registry");
    let backup = paths::root().join("registry-backup");
    t!(fs::rename(&registry, &backup));

    // Generate a Cargo.lock with the newer version, and then move the old copy
    // of the registry back into place.
    let p2 = project()
        .at("foo2")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "0.1.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p2.cargo("build").run();
    registry.rm_rf();
    t!(fs::rename(&backup, &registry));
    t!(fs::rename(
        p2.root().join("Cargo.lock"),
        p.root().join("Cargo.lock")
    ));

    // Finally, build the first project again (with our newer Cargo.lock) which
    // should force an update of the old registry, download the new crate, and
    // then build everything again.
    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.1 (registry `[ROOT][..]`)
[COMPILING] a v0.1.1
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn fetch_downloads() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("a", "0.1.0").publish();

    p.cargo("fetch")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.0 (registry [..])
",
        )
        .run();
}

#[cargo_test]
fn update_transitive_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("a", "0.1.0").dep("b", "*").publish();
    Package::new("b", "0.1.0").publish();

    p.cargo("fetch").run();

    Package::new("b", "0.1.1").publish();

    p.cargo("update -pb")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] b v0.1.0 -> v0.1.1
",
        )
        .run();

    p.cargo("build")
        .with_stderr(
            "\
[DOWNLOADING] crates ...
[DOWNLOADED] b v0.1.1 (registry `[ROOT][..]`)
[COMPILING] b v0.1.1
[COMPILING] a v0.1.0
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn update_backtracking_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                webdriver = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("webdriver", "0.1.0")
        .dep("hyper", "0.6")
        .publish();
    Package::new("hyper", "0.6.5")
        .dep("openssl", "0.1")
        .dep("cookie", "0.1")
        .publish();
    Package::new("cookie", "0.1.0")
        .dep("openssl", "0.1")
        .publish();
    Package::new("openssl", "0.1.0").publish();

    p.cargo("generate-lockfile").run();

    Package::new("openssl", "0.1.1").publish();
    Package::new("hyper", "0.6.6")
        .dep("openssl", "0.1.1")
        .dep("cookie", "0.1.0")
        .publish();

    p.cargo("update -p hyper")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] hyper v0.6.5 -> v0.6.6
[UPDATING] openssl v0.1.0 -> v0.1.1
",
        )
        .run();
}

#[cargo_test]
fn update_multiple_packages() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "*"
                b = "*"
                c = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("a", "0.1.0").publish();
    Package::new("b", "0.1.0").publish();
    Package::new("c", "0.1.0").publish();

    p.cargo("fetch").run();

    Package::new("a", "0.1.1").publish();
    Package::new("b", "0.1.1").publish();
    Package::new("c", "0.1.1").publish();

    p.cargo("update -pa -pb")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] a v0.1.0 -> v0.1.1
[UPDATING] b v0.1.0 -> v0.1.1
",
        )
        .run();

    p.cargo("update -pb -pc")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] c v0.1.0 -> v0.1.1
",
        )
        .run();

    p.cargo("build")
        .with_stderr_contains("[DOWNLOADED] a v0.1.1 (registry `[ROOT][..]`)")
        .with_stderr_contains("[DOWNLOADED] b v0.1.1 (registry `[ROOT][..]`)")
        .with_stderr_contains("[DOWNLOADED] c v0.1.1 (registry `[ROOT][..]`)")
        .with_stderr_contains("[COMPILING] a v0.1.1")
        .with_stderr_contains("[COMPILING] b v0.1.1")
        .with_stderr_contains("[COMPILING] c v0.1.1")
        .with_stderr_contains("[COMPILING] foo v0.5.0 ([..])")
        .run();
}

#[cargo_test]
fn bundled_crate_in_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = "0.1"
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0")
        .dep("bar", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "bar", version = "0.1.0" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .publish();

    p.cargo("run").run();
}

#[cargo_test]
fn update_same_prefix_oh_my_how_was_this_a_bug() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "ugh"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foobar", "0.2.0").publish();
    Package::new("foo", "0.1.0")
        .dep("foobar", "0.2.0")
        .publish();

    p.cargo("generate-lockfile").run();
    p.cargo("update -pfoobar --precise=0.2.0").run();
}

#[cargo_test]
fn use_semver() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "1.2.3-alpha.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "1.2.3-alpha.0").publish();

    p.cargo("build").run();
}

#[cargo_test]
fn use_semver_package_incorrectly() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [project]
            name = "a"
            version = "0.1.1-alpha.0"
            authors = []
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
            [project]
            name = "b"
            version = "0.1.0"
            authors = []

            [dependencies]
            a = { version = "^0.1", path = "../a" }
            "#,
        )
        .file("a/src/main.rs", "fn main() {}")
        .file("b/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: no matching package named `a` found
prerelease package needs to be specified explicitly
a = { version = \"0.1.1-alpha.0\" }
location searched: [..]
required by package `b v0.1.0 ([..])`
",
        )
        .run();
}

#[cargo_test]
fn only_download_relevant() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [target.foo.dependencies]
                foo = "*"
                [dev-dependencies]
                bar = "*"
                [dependencies]
                baz = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.1.0").publish();
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 ([..])
[COMPILING] baz v0.1.0
[COMPILING] bar v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn resolve_and_backtracking() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.1.1")
        .feature_dep("bar", "0.1", &["a", "b"])
        .publish();
    Package::new("foo", "0.1.0").publish();

    p.cargo("build").run();
}

#[cargo_test]
fn upstream_warnings_on_extra_verbose() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.1.0")
        .file("src/lib.rs", "fn unused() {}")
        .publish();

    p.cargo("build -vv")
        .with_stderr_contains("[..]warning: function is never used[..]")
        .run();
}

#[cargo_test]
fn disallow_network() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --frozen")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to get `foo` as a dependency of package `bar v0.5.0 [..]`

Caused by:
  failed to load source for dependency `foo`

Caused by:
  Unable to update registry [..]

Caused by:
  attempting to make an HTTP request, but --frozen was specified
",
        )
        .run();
}

#[cargo_test]
fn add_dep_dont_update_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [project]
                name = "baz"
                version = "0.5.0"
                authors = []

                [dependencies]
                remote = "0.3"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    Package::new("remote", "0.3.4").publish();

    p.cargo("build").run();

    p.change_file(
        "Cargo.toml",
        r#"
        [project]
        name = "bar"
        version = "0.5.0"
        authors = []

        [dependencies]
        baz = { path = "baz" }
        remote = "0.3"
        "#,
    );

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.5.0 ([..])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn bump_version_dont_update_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [project]
                name = "baz"
                version = "0.5.0"
                authors = []

                [dependencies]
                remote = "0.3"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    Package::new("remote", "0.3.4").publish();

    p.cargo("build").run();

    p.change_file(
        "Cargo.toml",
        r#"
        [project]
        name = "bar"
        version = "0.6.0"
        authors = []

        [dependencies]
        baz = { path = "baz" }
        "#,
    );

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.6.0 ([..])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn toml_lies_but_index_is_truth() {
    Package::new("foo", "0.2.0").publish();
    Package::new("bar", "0.3.0")
        .dep("foo", "0.2.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.3.0"
                authors = []

                [dependencies]
                foo = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "extern crate foo;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = "0.3"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn vv_prints_warnings() {
    Package::new("foo", "0.2.0")
        .file(
            "src/lib.rs",
            "#![deny(warnings)] fn foo() {} // unused function",
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "fo"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "0.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -vv").run();
}

#[cargo_test]
fn bad_and_or_malicious_packages_rejected() {
    Package::new("foo", "0.2.0")
        .extra_file("foo-0.1.0/src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "fo"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "0.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -vv")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] [..]
error: failed to download [..]

Caused by:
  failed to unpack [..]

Caused by:
  [..] contains a file at \"foo-0.1.0/src/lib.rs\" which isn't under \"foo-0.2.0\"
",
        )
        .run();
}

#[cargo_test]
fn git_init_templatedir_missing() {
    Package::new("foo", "0.2.0").dep("bar", "*").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "fo"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "0.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();

    remove_dir_all(paths::home().join(".cargo/registry")).unwrap();
    fs::write(
        paths::home().join(".gitconfig"),
        r#"
            [init]
            templatedir = nowhere
        "#,
    )
    .unwrap();

    p.cargo("build").run();
    p.cargo("build").run();
}

#[cargo_test]
fn rename_deps_and_features() {
    Package::new("foo", "0.1.0")
        .file("src/lib.rs", "pub fn f1() {}")
        .publish();
    Package::new("foo", "0.2.0")
        .file("src/lib.rs", "pub fn f2() {}")
        .publish();
    Package::new("bar", "0.2.0")
        .add_dep(
            Dependency::new("foo01", "0.1.0")
                .package("foo")
                .optional(true),
        )
        .add_dep(Dependency::new("foo02", "0.2.0").package("foo"))
        .feature("another", &["foo01"])
        .file(
            "src/lib.rs",
            r#"
                extern crate foo02;
                #[cfg(feature = "foo01")]
                extern crate foo01;

                pub fn foo() {
                    foo02::f2();
                    #[cfg(feature = "foo01")]
                    foo01::f1();
                }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = "0.2"
            "#,
        )
        .file(
            "src/main.rs",
            "
                extern crate bar;
                fn main() { bar::foo(); }
            ",
        )
        .build();

    p.cargo("build").run();
    p.cargo("build --features bar/foo01").run();
    p.cargo("build --features bar/another").run();
}

#[cargo_test]
fn ignore_invalid_json_lines() {
    Package::new("foo", "0.1.0").publish();
    Package::new("foo", "0.1.1").invalid_json(true).publish();
    Package::new("foo", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = '0.1.0'
                foo02 = { version = '0.2.0', package = 'foo' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn readonly_registry_still_works() {
    Package::new("foo", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = '0.1.0'
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("fetch --locked").run();
    chmod_readonly(&paths::home(), true);
    p.cargo("build").run();
    // make sure we un-readonly the files afterwards so "cargo clean" can remove them (#6934)
    chmod_readonly(&paths::home(), false);

    fn chmod_readonly(path: &Path, readonly: bool) {
        for entry in t!(path.read_dir()) {
            let entry = t!(entry);
            let path = entry.path();
            if t!(entry.file_type()).is_dir() {
                chmod_readonly(&path, readonly);
            } else {
                set_readonly(&path, readonly);
            }
        }
        set_readonly(path, readonly);
    }

    fn set_readonly(path: &Path, readonly: bool) {
        let mut perms = t!(path.metadata()).permissions();
        perms.set_readonly(readonly);
        t!(fs::set_permissions(path, perms));
    }
}

#[cargo_test]
fn registry_index_rejected() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            ".cargo/config",
            r#"
            [registry]
            index = "https://example.com/"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  the `registry.index` config value is no longer supported
  Use `[source]` replacement to alter the default index for crates.io.
",
        )
        .run();

    p.cargo("login")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the `registry.index` config value is no longer supported
Use `[source]` replacement to alter the default index for crates.io.
",
        )
        .run();
}

#[cargo_test]
fn package_lock_inside_package_is_overwritten() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1")
        .file("src/lib.rs", "")
        .file(".cargo-ok", "")
        .publish();

    p.cargo("build").run();

    let id = SourceId::for_registry(&registry_url()).unwrap();
    let hash = cargo::util::hex::short_hash(&id);
    let ok = cargo_home()
        .join("registry")
        .join("src")
        .join(format!("-{}", hash))
        .join("bar-0.0.1")
        .join(".cargo-ok");

    assert_eq!(ok.metadata().unwrap().len(), 2);
}

#[cargo_test]
fn ignores_unknown_index_version() {
    // If the version field is not understood, it is ignored.
    Package::new("bar", "1.0.0").publish();
    Package::new("bar", "1.0.1").schema_version(9999).publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout(
            "foo v0.1.0 [..]\n\
             └── bar v1.0.0\n\
            ",
        )
        .run();
}
