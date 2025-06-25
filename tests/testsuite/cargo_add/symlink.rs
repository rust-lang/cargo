use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry;
use std::fs;

#[cargo_test]
fn symlink_case() {
    if !cargo_test_support::symlink_supported() {
        return;
    }

    registry::init();
    registry::Package::new("test-dep", "1.0.0").publish();

    let project = project().file("src/lib.rs", "").build();

    let target_dir = project.root().join("target_dir");
    fs::create_dir_all(&target_dir).unwrap();

    fs::copy(
        project.root().join("Cargo.toml"),
        target_dir.join("Cargo.toml"),
    )
    .unwrap();

    fs::remove_file(project.root().join("Cargo.toml")).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(
            target_dir.join("Cargo.toml"),
            project.root().join("Cargo.toml"),
        )
        .unwrap();
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_file;
        symlink_file(
            target_dir.join("Cargo.toml"),
            project.root().join("Cargo.toml"),
        )
        .unwrap();
    }

    project.cargo("add test-dep").run();

    assert!(project.root().join("Cargo.toml").is_symlink());

    let target_content = fs::read_to_string(target_dir.join("Cargo.toml")).unwrap();
    assert!(target_content.contains("test-dep"));
}
