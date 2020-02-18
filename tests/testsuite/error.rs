//! General error tests that don't belong anywhere else.

use cargo_test_support::cargo_process;

#[cargo_test]
fn internal_error() {
    cargo_process("init")
        .env("__CARGO_TEST_INTERNAL_ERROR", "1")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] internal error test
[NOTE] this is an unexpected cargo internal error
[NOTE] we would appreciate a bug report: https://github.com/rust-lang/cargo/issues/
[NOTE] cargo [..]
",
        )
        .run();
}
