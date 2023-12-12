use cargo_test_support::curr_dir;
use cargo_test_support::prelude::*;

#[cargo_test]
fn remove_case() {
    snapbox::cmd::Command::cargo_ui()
        .arg("owner")
        .arg("remove")
        .arg("--help")
        .assert()
        .success()
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));
}
