extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn net_retry_loads_from_config() {
    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = "https://127.0.0.1:11/foo/bar"
        "#))
        .file("src/main.rs", "").file(".cargo/config", r#"
        [net]
        retry=1
        [http]
        timeout=1
         "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                .with_stderr_contains(&format!("[WARNING] spurious network error \
(1 tries remaining): [2/-1] [..]")));
}

#[test]
fn net_retry_git_outputs_warning() {
    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            git = "https://127.0.0.1:11/foo/bar"
        "#))
        .file(".cargo/config", r#"
        [http]
        timeout=1
         "#)
        .file("src/main.rs", "");

    assert_that(p.cargo_process("build").arg("-v").arg("-j").arg("1"),
                execs().with_status(101)
                .with_stderr_contains(&format!("[WARNING] spurious network error \
(2 tries remaining): [2/-1] [..]"))
                .with_stderr_contains(&format!("\
[WARNING] spurious network error (1 tries remaining): [2/-1] [..]")));
}
