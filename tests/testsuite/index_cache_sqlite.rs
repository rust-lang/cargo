//! Tests for the `-Zindex-cache-sqlite`.

use std::collections::HashSet;

use cargo_test_support::paths;
use cargo_test_support::project;
use cargo_test_support::registry;
use cargo_test_support::registry::Package;

#[cargo_test]
fn gated() {
    project()
        .build()
        .cargo("fetch")
        .arg("-Zindex-cache-sqlite")
        .with_status(101)
        .with_stderr_contains("[ERROR] the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel")
        .run();
}

#[cargo_test]
fn crates_io() {
    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                dep2 = "0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("dep1", "0.0.0").publish();
    Package::new("dep2", "0.0.0").dep("dep1", "0.0.0").publish();
    Package::new("dep3", "0.0.0").publish();

    p.cargo("fetch")
        .masquerade_as_nightly_cargo(&["index-cache-sqlite"])
        .arg("-Zindex-cache-sqlite")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep1 v0.0.0 (registry `dummy-registry`)
[DOWNLOADED] dep2 v0.0.0 (registry `dummy-registry`)
",
        )
        .run();

    assert_rows_inserted(&["dep1", "dep2"]);

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            edition = "2015"

            [dependencies]
            dep2 = "0.0.0"
            dep3 = "0.0.0"
        "#,
    );

    p.cargo("fetch")
        .masquerade_as_nightly_cargo(&["index-cache-sqlite"])
        .arg("-Zindex-cache-sqlite")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep3 v0.0.0 (registry `dummy-registry`)
",
        )
        .run();

    assert_rows_inserted(&["dep1", "dep2", "dep3"]);
}

#[track_caller]
fn assert_rows_inserted(names: &[&str]) {
    let pattern = paths::home().join(".cargo/registry/index/*/.cache/index-cache.db");
    let pattern = pattern.to_str().unwrap();
    let db_path = glob::glob(pattern).unwrap().next().unwrap().unwrap();

    let set: HashSet<String> = rusqlite::Connection::open(&db_path)
        .unwrap()
        .prepare("SELECT name FROM summaries")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(set, HashSet::from_iter(names.iter().map(|n| n.to_string())));
}
