//! Tests for the `cargo sync-lockfile` command.

use cargo_test_support::project;

#[cargo_test]
fn sync_after_generate() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("generate-lockfile").run();
    let lock1 = p.read_lockfile();

    // add a dep
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.2"
        "#,
    );
    p.cargo("sync-lockfile").run();
    let lock2 = p.read_lockfile();
    assert_ne!(lock1, lock2);
    assert!(lock1.contains("0.0.1"));
    assert!(lock2.contains("0.0.2"));
    assert!(!lock1.contains("0.0.2"));
    assert!(!lock2.contains("0.0.1"));
}
