use crate::support::project;
use cargo;

#[test]
fn simple() {
    let p = project().build();

    p.cargo("version")
        .with_stdout(&format!("{}\n", cargo::version()))
        .run();

    p.cargo("--version")
        .with_stdout(&format!("{}\n", cargo::version()))
        .run();
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn version_works_without_rustc() {
    let p = project().build();
    p.cargo("version").env("PATH", "").run();
}

#[test]
fn version_works_with_bad_config() {
    let p = project().file(".cargo/config", "this is not toml").build();
    p.cargo("version").run();
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
    p.cargo("version").run();
}
