use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;

use cargo::util::paths::remove_dir_all;
use support::cargo_process;
use support::git;
use support::paths::{self, CargoPathExt};
use support::registry::{self, Package};
use support::{basic_manifest, execs, project};
use support::hamcrest::assert_that;
use url::Url;

fn registry_path() -> PathBuf {
    paths::root().join("registry")
}
fn registry() -> Url {
    Url::from_file_path(&*registry_path()).ok().unwrap()
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `{reg}`
[DOWNLOADING] bar v0.0.1 (registry `file://[..]`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url(),
            reg = registry::registry()
        )),
    );

    assert_that(p.cargo("clean"), execs());

    // Don't download a second time
    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `{reg}`
[DOWNLOADING] [..] v0.0.1 (registry `file://[..]`)
[DOWNLOADING] [..] v0.0.1 (registry `file://[..]`)
[COMPILING] baz v0.0.1
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url(),
            reg = registry::registry()
        )),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[UPDATING] registry [..]
error: no matching package named `nonexistent` found
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        ),
    );
}

#[test]
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
    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[UPDATING] registry [..]
error: no matching package named `Init` found
location searched: registry [..]
did you mean: init
required by package `foo v0.0.1 ([..])`
",
        ),
    );
}

#[test]
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
    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[UPDATING] registry [..]
error: no matching package named `mis_hyphenated` found
location searched: registry [..]
did you mean: mis-hyphenated
required by package `foo v0.0.1 ([..])`
",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "\
error: no matching version `>= 1.0.0` found for package `foo`
location searched: registry [..]
versions found: 0.0.2, 0.0.1
required by package `foo v0.0.1 ([..])`
",
        ),
    );

    Package::new("foo", "0.0.3").publish();
    Package::new("foo", "0.0.4").publish();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "\
error: no matching version `>= 1.0.0` found for package `foo`
location searched: registry [..]
versions found: 0.0.4, 0.0.3, 0.0.2, ...
required by package `foo v0.0.1 ([..])`
",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("build -v"),
        execs().with_status(101).with_stderr(
            "\
[UPDATING] registry [..]
[DOWNLOADING] bad-cksum [..]
[ERROR] unable to get packages from source

Caused by:
  failed to download replaced source registry `https://[..]`

Caused by:
  failed to verify the checksum of `bad-cksum v0.0.1 (registry `file://[..]`)`
",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "\
error: no matching package named `notyet` found
location searched: registry `[..]`
required by package `foo v0.0.1 ([..])`
",
        ),
    );

    Package::new("notyet", "0.0.1").publish();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `{reg}`
[DOWNLOADING] notyet v0.0.1 (registry `file://[..]`)
[COMPILING] notyet v0.0.1
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url(),
            reg = registry::registry()
        )),
    );
}

#[test]
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

    assert_that(
        p.cargo("package -v"),
        execs().with_status(101).with_stderr_contains(
            "\
[ERROR] failed to verify package tarball

Caused by:
  no matching package named `notyet` found
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        ),
    );

    Package::new("notyet", "0.0.1").publish();

    assert_that(
        p.cargo("package"),
        execs().with_stderr(format!(
            "\
[PACKAGING] foo v0.0.1 ({dir})
[VERIFYING] foo v0.0.1 ({dir})
[UPDATING] registry `[..]`
[DOWNLOADING] notyet v0.0.1 (registry `file://[..]`)
[COMPILING] notyet v0.0.1
[COMPILING] foo v0.0.1 ({dir}[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] bar v0.0.1 (registry `file://[..]`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );

    p.root().move_into_the_past();
    Package::new("bar", "0.0.2").publish();

    assert_that(p.cargo("build"), execs().with_stdout(""));
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] [..] v0.0.1 (registry `file://[..]`)
[DOWNLOADING] [..] v0.0.1 (registry `file://[..]`)
[COMPILING] baz v0.0.1
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );

    p.root().move_into_the_past();
    Package::new("baz", "0.0.2").publish();
    Package::new("bar", "0.0.2").dep("baz", "*").publish();

    assert_that(p.cargo("build"), execs().with_stdout(""));
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] [..] v0.0.1 (registry `file://[..]`)
[DOWNLOADING] [..] v0.0.1 (registry `file://[..]`)
[COMPILING] baz v0.0.1
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "\
error: no matching version `= 0.0.2` found for package `baz`
location searched: registry `[..]`
versions found: 0.0.1
required by package `bar v0.0.1`
",
        ),
    );
}

#[test]
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

    assert_that(p.cargo("build"), execs());

    registry::registry_path().join("3").rm_rf();

    Package::new("bar", "0.0.1").yanked(true).publish();

    assert_that(p.cargo("build"), execs().with_stdout(""));

    assert_that(
        p.cargo("update"),
        execs().with_status(101).with_stderr_contains(
            "\
error: no matching package named `bar` found
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        ),
    );
}

#[test]
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
    assert_that(p.cargo("build"), execs());
    p.root().move_into_the_past();

    paths::home().join(".cargo/registry").rm_rf();
    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] bar v0.0.1 (registry `file://[..]`)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        ),
    );
}

#[test]
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
    assert_that(p.cargo("build"), execs());

    Package::new("bar", "0.0.2").publish();
    Package::new("bar", "0.0.3").publish();
    paths::home().join(".cargo/registry").rm_rf();
    println!("0.0.2 update");
    assert_that(
        p.cargo("update -p bar --precise 0.0.2"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] bar v0.0.1 -> v0.0.2
",
        ),
    );

    println!("0.0.2 build");
    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[DOWNLOADING] [..] v0.0.2 (registry `file://[..]`)
[COMPILING] bar v0.0.2
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );

    println!("0.0.3 update");
    assert_that(
        p.cargo("update -p bar"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] bar v0.0.2 -> v0.0.3
",
        ),
    );

    println!("0.0.3 build");
    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[DOWNLOADING] [..] v0.0.3 (registry `file://[..]`)
[COMPILING] bar v0.0.3
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );

    println!("new dependencies update");
    Package::new("bar", "0.0.4").dep("spam", "0.2.5").publish();
    Package::new("spam", "0.2.5").publish();
    assert_that(
        p.cargo("update -p bar"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] bar v0.0.3 -> v0.0.4
[ADDING] spam v0.2.5
",
        ),
    );

    println!("new dependencies update");
    Package::new("bar", "0.0.5").publish();
    assert_that(
        p.cargo("update -p bar"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] bar v0.0.4 -> v0.0.5
[REMOVING] spam v0.2.5
",
        ),
    );
}

#[test]
fn update_offline() {
    use support::ChannelChanger;
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
    assert_that(
        p.cargo("update -Zoffline")
            .masquerade_as_nightly_cargo(),
        execs()
            .with_status(101)
            .with_stderr("error: you can't update in the offline mode[..]"),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] [..] v0.0.1 (registry `file://[..]`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );
}

#[test]
fn login_with_no_cargo_dir() {
    let home = paths::home().join("new-home");
    t!(fs::create_dir(&home));
    assert_that(
        cargo_process("login foo -v"),
        execs(),
    );
}

#[test]
fn login_with_differently_sized_token() {
    // Verify that the configuration file gets properly trunchated.
    let home = paths::home().join("new-home");
    t!(fs::create_dir(&home));
    assert_that(
        cargo_process("login lmaolmaolmao -v"),
        execs(),
    );
    assert_that(
        cargo_process("login lmao -v"),
        execs(),
    );
    assert_that(
        cargo_process("login lmaolmaolmao -v"),
        execs(),
    );
}

#[test]
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
    assert_that(
        p.cargo("publish -v --index")
            .arg(registry().to_string()),
        execs()
            .with_status(101)
            .with_stderr_contains("[ERROR] the license file `foo` does not exist"),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] bar v0.0.1 (registry `file://[..]`)
[COMPILING] bar v0.0.1
[COMPILING] a v0.0.1 ({dir}/a)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );

    t!(t!(File::create(&p.root().join("a/Cargo.toml"))).write_all(
        br#"
        [project]
        name = "a"
        version = "0.0.1"
        authors = []

        [dependencies]
        bar = "0.1.0"
    "#
    ));
    Package::new("bar", "0.1.0").publish();

    println!("second");
    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] bar v0.1.0 (registry `file://[..]`)
[COMPILING] bar v0.1.0
[COMPILING] a v0.0.1 ({dir}/a)
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );
}

#[test]
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
    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] [..]
[UPDATING] [..]
[DOWNLOADING] a v0.0.1 (registry `file://[..]`)
[COMPILING] a v0.0.1
[COMPILING] b v0.0.1 ([..])
[COMPILING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );
    p.root().move_into_the_past();

    println!("second");
    assert_that(p.cargo("build"), execs().with_stdout(""));
}

#[test]
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
    assert_that(p.cargo("build"), execs());

    // Next, publish a new package and back up the copy of the registry we just
    // created.
    Package::new("a", "0.1.1").publish();
    let registry = paths::home().join(".cargo/registry");
    let backup = paths::root().join("registry-backup");
    t!(fs::rename(&registry, &backup));

    // Generate a Cargo.lock with the newer version, and then move the old copy
    // of the registry back into place.
    let p2 = project().at("foo2")
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
    assert_that(p2.cargo("build"), execs());
    registry.rm_rf();
    t!(fs::rename(&backup, &registry));
    t!(fs::rename(
        p2.root().join("Cargo.lock"),
        p.root().join("Cargo.lock")
    ));

    // Finally, build the first project again (with our newer Cargo.lock) which
    // should force an update of the old registry, download the new crate, and
    // then build everything again.
    assert_that(
        p.cargo("build"),
        execs().with_stderr(&format!(
            "\
[UPDATING] [..]
[DOWNLOADING] a v0.1.1 (registry `file://[..]`)
[COMPILING] a v0.1.1
[COMPILING] foo v0.5.0 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            dir = p.url()
        )),
    );
}

#[test]
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

    assert_that(
        p.cargo("fetch"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] a v0.1.0 (registry [..])
",
        ),
    );
}

#[test]
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

    assert_that(p.cargo("fetch"), execs());

    Package::new("b", "0.1.1").publish();

    assert_that(
        p.cargo("update -pb"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] b v0.1.0 -> v0.1.1
",
        ),
    );

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[DOWNLOADING] b v0.1.1 (registry `file://[..]`)
[COMPILING] b v0.1.1
[COMPILING] a v0.1.0
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        ),
    );
}

#[test]
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

    assert_that(p.cargo("generate-lockfile"), execs());

    Package::new("openssl", "0.1.1").publish();
    Package::new("hyper", "0.6.6")
        .dep("openssl", "0.1.1")
        .dep("cookie", "0.1.0")
        .publish();

    assert_that(
        p.cargo("update -p hyper"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] hyper v0.6.5 -> v0.6.6
[UPDATING] openssl v0.1.0 -> v0.1.1
",
        ),
    );
}

#[test]
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

    assert_that(p.cargo("fetch"), execs());

    Package::new("a", "0.1.1").publish();
    Package::new("b", "0.1.1").publish();
    Package::new("c", "0.1.1").publish();

    assert_that(
        p.cargo("update -pa -pb"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] a v0.1.0 -> v0.1.1
[UPDATING] b v0.1.0 -> v0.1.1
",
        ),
    );

    assert_that(
        p.cargo("update -pb -pc"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] c v0.1.0 -> v0.1.1
",
        ),
    );

    assert_that(
        p.cargo("build"),
        execs()
            .with_stderr_contains("[DOWNLOADING] a v0.1.1 (registry `file://[..]`)")
            .with_stderr_contains("[DOWNLOADING] b v0.1.1 (registry `file://[..]`)")
            .with_stderr_contains("[DOWNLOADING] c v0.1.1 (registry `file://[..]`)")
            .with_stderr_contains("[COMPILING] a v0.1.1")
            .with_stderr_contains("[COMPILING] b v0.1.1")
            .with_stderr_contains("[COMPILING] c v0.1.1")
            .with_stderr_contains("[COMPILING] foo v0.5.0 ([..])"),
    );
}

#[test]
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

    assert_that(p.cargo("run"), execs());
}

#[test]
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

    assert_that(p.cargo("generate-lockfile"), execs());
    assert_that(
        p.cargo("update -pfoobar --precise=0.2.0"),
        execs(),
    );
}

#[test]
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

    assert_that(p.cargo("build"), execs());
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[DOWNLOADING] baz v0.1.0 ([..])
[COMPILING] baz v0.1.0
[COMPILING] bar v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        ),
    );
}

#[test]
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

    assert_that(p.cargo("build"), execs());
}

#[test]
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

    assert_that(
        p.cargo("build -vv"),
        execs().with_stderr_contains("[..]warning: function is never used[..]"),
    );
}

#[test]
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

    assert_that(
        p.cargo("build --frozen"),
        execs().with_status(101).with_stderr(
            "\
error: failed to load source for a dependency on `foo`

Caused by:
  Unable to update registry [..]

Caused by:
  attempting to make an HTTP request, but --frozen was specified
",
        ),
    );
}

#[test]
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

    assert_that(p.cargo("build"), execs());

    t!(t!(File::create(p.root().join("Cargo.toml"))).write_all(
        br#"
        [project]
        name = "bar"
        version = "0.5.0"
        authors = []

        [dependencies]
        baz = { path = "baz" }
        remote = "0.3"
    "#
    ));

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[COMPILING] bar v0.5.0 ([..])
[FINISHED] [..]
",
        ),
    );
}

#[test]
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

    assert_that(p.cargo("build"), execs());

    t!(t!(File::create(p.root().join("Cargo.toml"))).write_all(
        br#"
        [project]
        name = "bar"
        version = "0.6.0"
        authors = []

        [dependencies]
        baz = { path = "baz" }
    "#
    ));

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[COMPILING] bar v0.6.0 ([..])
[FINISHED] [..]
",
        ),
    );
}

#[test]
fn old_version_req() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.5.0"
            authors = []

            [dependencies]
            remote = "0.2*"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("remote", "0.2.0").publish();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
warning: parsed version requirement `0.2*` is no longer valid

Previous versions of Cargo accepted this malformed requirement,
but it is being deprecated. This was found when parsing the manifest
of bar 0.5.0, and the correct version requirement is `0.2.*`.

This will soon become a hard error, so it's either recommended to
update to a fixed version or contact the upstream maintainer about
this warning.

warning: parsed version requirement `0.2*` is no longer valid

Previous versions of Cargo accepted this malformed requirement,
but it is being deprecated. This was found when parsing the manifest
of bar 0.5.0, and the correct version requirement is `0.2.*`.

This will soon become a hard error, so it's either recommended to
update to a fixed version or contact the upstream maintainer about
this warning.

[UPDATING] [..]
[DOWNLOADING] [..]
[COMPILING] [..]
[COMPILING] [..]
[FINISHED] [..]
",
        ),
    );
}

#[test]
fn old_version_req_upstream() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.5.0"
            authors = []

            [dependencies]
            remote = "0.3"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("remote", "0.3.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "remote"
                version = "0.3.0"
                authors = []

                [dependencies]
                bar = "0.2*"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    Package::new("bar", "0.2.0").publish();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
warning: parsed version requirement `0.2*` is no longer valid

Previous versions of Cargo accepted this malformed requirement,
but it is being deprecated. This was found when parsing the manifest
of remote 0.3.0, and the correct version requirement is `0.2.*`.

This will soon become a hard error, so it's either recommended to
update to a fixed version or contact the upstream maintainer about
this warning.

[COMPILING] [..]
[COMPILING] [..]
[FINISHED] [..]
",
        ),
    );
}

#[test]
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

    assert_that(p.cargo("build -v"), execs());
}

#[test]
fn vv_prints_warnings() {
    Package::new("foo", "0.2.0")
        .file("src/lib.rs", "#![deny(warnings)] fn foo() {} // unused function")
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

    assert_that(p.cargo("build -vv"), execs());
}

#[test]
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

    assert_that(
        p.cargo("build -vv"),
        execs().with_status(101).with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
error: unable to get packages from source

Caused by:
  failed to download [..]

Caused by:
  failed to unpack [..]

Caused by:
  [..] contains a file at \"foo-0.1.0/src/lib.rs\" which isn't under \"foo-0.2.0\"
",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("build"),
        execs()
    );

    remove_dir_all(paths::home().join(".cargo/registry")).unwrap();
    File::create(paths::home().join(".gitconfig"))
        .unwrap()
        .write_all(br#"
            [init]
            templatedir = nowhere
        "#)
        .unwrap();

    assert_that(
        p.cargo("build"),
        execs()
    );
    assert_that(
        p.cargo("build"),
        execs()
    );
}
