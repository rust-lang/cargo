use support::{basic_bin_manifest, execs, main_file, project};
use support::hamcrest::assert_that;

fn verify_project_success_output() -> String {
    r#"{"success":"true"}"#.into()
}

#[test]
fn cargo_verify_project_path_to_cargo_toml_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(
        p.cargo("verify-project --manifest-path foo/Cargo.toml")
            .cwd(p.root().parent().unwrap()),
        execs()
            .with_stdout(verify_project_success_output()),
    );
}

#[test]
fn cargo_verify_project_path_to_cargo_toml_absolute() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(
        p.cargo("verify-project --manifest-path")
            .arg(p.root().join("Cargo.toml"))
            .cwd(p.root().parent().unwrap()),
        execs()
            .with_stdout(verify_project_success_output()),
    );
}

#[test]
fn cargo_verify_project_cwd() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    assert_that(
        p.cargo("verify-project").cwd(p.root()),
        execs()
            .with_stdout(verify_project_success_output()),
    );
}
