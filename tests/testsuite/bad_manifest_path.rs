//! Tests for invalid --manifest-path arguments.

use cargo_test_support::{basic_bin_manifest, main_file, project};

#[track_caller]
fn assert_not_a_cargo_toml(command: &str, manifest_path_argument: &str) {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo(command)
        .arg("--manifest-path")
        .arg(manifest_path_argument)
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr(
            "[ERROR] the manifest-path must be a path \
             to a Cargo.toml file",
        )
        .run();
}

#[track_caller]
fn assert_cargo_toml_doesnt_exist(command: &str, manifest_path_argument: &str) {
    let p = project().build();
    let expected_path = manifest_path_argument
        .split('/')
        .collect::<Vec<_>>()
        .join("[..]");

    p.cargo(command)
        .arg("--manifest-path")
        .arg(manifest_path_argument)
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr(format!(
            "[ERROR] manifest path `{}` does not exist",
            expected_path
        ))
        .run();
}

#[cargo_test]
fn bench_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("bench", "foo");
}

#[cargo_test]
fn bench_dir_plus_file() {
    assert_not_a_cargo_toml("bench", "foo/bar");
}

#[cargo_test]
fn bench_dir_plus_path() {
    assert_not_a_cargo_toml("bench", "foo/bar/baz");
}

#[cargo_test]
fn bench_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("bench", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn build_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("check", "foo");
}

#[cargo_test]
fn build_dir_plus_file() {
    assert_not_a_cargo_toml("bench", "foo/bar");
}

#[cargo_test]
fn build_dir_plus_path() {
    assert_not_a_cargo_toml("bench", "foo/bar/baz");
}

#[cargo_test]
fn build_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("check", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn clean_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("clean", "foo");
}

#[cargo_test]
fn clean_dir_plus_file() {
    assert_not_a_cargo_toml("clean", "foo/bar");
}

#[cargo_test]
fn clean_dir_plus_path() {
    assert_not_a_cargo_toml("clean", "foo/bar/baz");
}

#[cargo_test]
fn clean_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("clean", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn doc_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("doc", "foo");
}

#[cargo_test]
fn doc_dir_plus_file() {
    assert_not_a_cargo_toml("doc", "foo/bar");
}

#[cargo_test]
fn doc_dir_plus_path() {
    assert_not_a_cargo_toml("doc", "foo/bar/baz");
}

#[cargo_test]
fn doc_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("doc", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn fetch_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("fetch", "foo");
}

#[cargo_test]
fn fetch_dir_plus_file() {
    assert_not_a_cargo_toml("fetch", "foo/bar");
}

#[cargo_test]
fn fetch_dir_plus_path() {
    assert_not_a_cargo_toml("fetch", "foo/bar/baz");
}

#[cargo_test]
fn fetch_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("fetch", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn generate_lockfile_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("generate-lockfile", "foo");
}

#[cargo_test]
fn generate_lockfile_dir_plus_file() {
    assert_not_a_cargo_toml("generate-lockfile", "foo/bar");
}

#[cargo_test]
fn generate_lockfile_dir_plus_path() {
    assert_not_a_cargo_toml("generate-lockfile", "foo/bar/baz");
}

#[cargo_test]
fn generate_lockfile_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("generate-lockfile", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn package_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("package", "foo");
}

#[cargo_test]
fn package_dir_plus_file() {
    assert_not_a_cargo_toml("package", "foo/bar");
}

#[cargo_test]
fn package_dir_plus_path() {
    assert_not_a_cargo_toml("package", "foo/bar/baz");
}

#[cargo_test]
fn package_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("package", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn pkgid_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("pkgid", "foo");
}

#[cargo_test]
fn pkgid_dir_plus_file() {
    assert_not_a_cargo_toml("pkgid", "foo/bar");
}

#[cargo_test]
fn pkgid_dir_plus_path() {
    assert_not_a_cargo_toml("pkgid", "foo/bar/baz");
}

#[cargo_test]
fn pkgid_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("pkgid", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn publish_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("publish", "foo");
}

#[cargo_test]
fn publish_dir_plus_file() {
    assert_not_a_cargo_toml("publish", "foo/bar");
}

#[cargo_test]
fn publish_dir_plus_path() {
    assert_not_a_cargo_toml("publish", "foo/bar/baz");
}

#[cargo_test]
fn publish_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("publish", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn read_manifest_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("read-manifest", "foo");
}

#[cargo_test]
fn read_manifest_dir_plus_file() {
    assert_not_a_cargo_toml("read-manifest", "foo/bar");
}

#[cargo_test]
fn read_manifest_dir_plus_path() {
    assert_not_a_cargo_toml("read-manifest", "foo/bar/baz");
}

#[cargo_test]
fn read_manifest_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("read-manifest", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn run_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("run", "foo");
}

#[cargo_test]
fn run_dir_plus_file() {
    assert_not_a_cargo_toml("run", "foo/bar");
}

#[cargo_test]
fn run_dir_plus_path() {
    assert_not_a_cargo_toml("run", "foo/bar/baz");
}

#[cargo_test]
fn run_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("run", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn rustc_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("rustc", "foo");
}

#[cargo_test]
fn rustc_dir_plus_file() {
    assert_not_a_cargo_toml("rustc", "foo/bar");
}

#[cargo_test]
fn rustc_dir_plus_path() {
    assert_not_a_cargo_toml("rustc", "foo/bar/baz");
}

#[cargo_test]
fn rustc_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("rustc", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn test_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("test", "foo");
}

#[cargo_test]
fn test_dir_plus_file() {
    assert_not_a_cargo_toml("test", "foo/bar");
}

#[cargo_test]
fn test_dir_plus_path() {
    assert_not_a_cargo_toml("test", "foo/bar/baz");
}

#[cargo_test]
fn test_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("test", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn update_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("update", "foo");
}

#[cargo_test]
fn update_dir_plus_file() {
    assert_not_a_cargo_toml("update", "foo/bar");
}

#[cargo_test]
fn update_dir_plus_path() {
    assert_not_a_cargo_toml("update", "foo/bar/baz");
}

#[cargo_test]
fn update_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("update", "foo/bar/baz/Cargo.toml");
}

#[cargo_test]
fn verify_project_dir_containing_cargo_toml() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("verify-project --manifest-path foo")
        .cwd(p.root().parent().unwrap())
        .with_status(1)
        .with_stdout(
            "{\"invalid\":\"the manifest-path must be a path to a Cargo.toml file\"}\
             ",
        )
        .run();
}

#[cargo_test]
fn verify_project_dir_plus_file() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("verify-project --manifest-path foo/bar")
        .cwd(p.root().parent().unwrap())
        .with_status(1)
        .with_stdout(
            "{\"invalid\":\"the manifest-path must be a path to a Cargo.toml file\"}\
             ",
        )
        .run();
}

#[cargo_test]
fn verify_project_dir_plus_path() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("verify-project --manifest-path foo/bar/baz")
        .cwd(p.root().parent().unwrap())
        .with_status(1)
        .with_stdout(
            "{\"invalid\":\"the manifest-path must be a path to a Cargo.toml file\"}\
             ",
        )
        .run();
}

#[cargo_test]
fn verify_project_dir_to_nonexistent_cargo_toml() {
    let p = project().build();
    p.cargo("verify-project --manifest-path foo/bar/baz/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .with_status(1)
        .with_stdout(
            "{\"invalid\":\"manifest path `foo[..]bar[..]baz[..]Cargo.toml` does not exist\"}\
             ",
        )
        .run();
}
