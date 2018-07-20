use cargo;
use cargotest::support::{execs, project};
use hamcrest::assert_that;

#[test]
fn simple() {
    let p = project().build();

    assert_that(
        p.cargo("version"),
        execs()
            .with_status(0)
            .with_stdout(&format!("{}\n", cargo::version())),
    );

    assert_that(
        p.cargo("--version"),
        execs()
            .with_status(0)
            .with_stdout(&format!("{}\n", cargo::version())),
    );
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn version_works_without_rustc() {
    let p = project().build();
    assert_that(p.cargo("version").env("PATH", ""), execs().with_status(0));
}

#[test]
fn version_works_with_bad_config() {
    let p = project()
        .file(".cargo/config", "this is not toml")
        .build();
    assert_that(p.cargo("version"), execs().with_status(0));
}

#[test]
fn version_works_with_bad_target_dir() {
    let p = project()
        .file(
            ".cargo/config",
            r#"
            [build]
            target-dir = 4
        "#,
        )
        .build();
    assert_that(p.cargo("version"), execs().with_status(0));
}
