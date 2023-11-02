use std::path::Path;

use snapbox::cmd::Command;

#[test]
fn stdout_redirected() {
    let bin = snapbox::cmd::compile_example("stdout-redirected", []).unwrap();

    let hello = r#"{"v":[1]}"#;
    let get_request = r#"{"v": 1, "registry": {"index-url":"sparse+https://test/","name":"alternative"},"kind": "get","operation": "read","args": []}"#;
    let err_not_supported = r#"{"Err":{"kind":"operation-not-supported"}}"#;

    Command::new(bin)
        .stdin(format!("{get_request}\n"))
        .arg("--cargo-plugin")
        .assert()
        .stdout_eq(format!("{hello}\n{err_not_supported}\n"))
        .stderr_eq("message on stderr should be sent the the parent process\n")
        .success();
}

#[test]
fn file_provider() {
    let bin = snapbox::cmd::compile_example("file-provider", []).unwrap();

    let hello = r#"{"v":[1]}"#;
    let login_request = r#"{"v": 1,"registry": {"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind": "login","token": "s3krit","args": []}"#;
    let login_response = r#"{"Ok":{"kind":"login"}}"#;

    let get_request = r#"{"v": 1,"registry": {"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind": "get","operation": "read","args": []}"#;
    let get_response =
        r#"{"Ok":{"kind":"get","token":"s3krit","cache":"session","operation_independent":true}}"#;

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cargo-credential-tests");
    std::fs::create_dir(&dir).unwrap();
    Command::new(bin)
        .current_dir(&dir)
        .stdin(format!("{login_request}\n{get_request}\n"))
        .arg("--cargo-plugin")
        .assert()
        .stdout_eq(format!("{hello}\n{login_response}\n{get_response}\n"))
        .stderr_eq("")
        .success();
    std::fs::remove_dir_all(&dir).unwrap();
}
