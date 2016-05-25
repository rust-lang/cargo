use support::{project, execs, main_file, basic_bin_manifest};
use hamcrest::{assert_that};

fn verify_project_success_output() -> String {
    r#"{"success":"true"}"#.into()
}

#[test]
fn cargo_verify_project_path_to_cargo_toml_relative() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("verify-project")
                 .arg("--manifest-path").arg("foo/Cargo.toml")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0)
                       .with_stdout(verify_project_success_output()));
}

#[test]
fn cargo_verify_project_path_to_cargo_toml_absolute() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("verify-project")
                 .arg("--manifest-path").arg(p.root().join("Cargo.toml"))
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0)
                       .with_stdout(verify_project_success_output()));
}

#[test]
fn cargo_verify_project_cwd() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("verify-project")
                 .cwd(p.root()),
                execs().with_status(0)
                       .with_stdout(verify_project_success_output()));
}
