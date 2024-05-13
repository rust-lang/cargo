use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .arg("metadata")
        .arg("--help")
        .assert()
        .success()
        .stdout_matches(file!["stdout.term.svg"])
        .stderr_matches(str![""]);
}

#[cfg(windows)]
#[cargo_test]
fn windows_verbatim_disk_case() {
    // `canonicalize` func on Windows will return a path starts with `r"\\?\"`,
    // which is called as `Verbatim disk prefix`.
    // See: https://doc.rust-lang.org/std/path/enum.Prefix.html#variant.VerbatimDisk
    snapbox::cmd::Command::cargo_ui()
        .arg("metadata")
        .args([
            "--manifest-path",
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("Cargo.toml")
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap(),
            "--no-deps",
            "--offline",
        ])
        .assert()
        .success();
}
