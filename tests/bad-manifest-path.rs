extern crate hamcrest;
extern crate cargotest;

use cargotest::support::{project, execs, main_file, basic_bin_manifest};
use hamcrest::{assert_that};

fn assert_not_a_cargo_toml(command: &str, manifest_path_argument: &str) {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process(command)
                 .arg("--manifest-path").arg(manifest_path_argument)
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(101)
                       .with_stderr("[ERROR] the manifest-path must be a path \
                                             to a Cargo.toml file"));
}


fn assert_cargo_toml_doesnt_exist(command: &str, manifest_path_argument: &str) {
    let p = project("foo");
    let expected_path = manifest_path_argument
        .split('/').collect::<Vec<_>>().join("[..]");

    assert_that(p.cargo_process(command)
                 .arg("--manifest-path").arg(manifest_path_argument)
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(101)
                       .with_stderr(
                           format!("[ERROR] manifest path `{}` does not exist",
                                   expected_path)
                       ));
}

#[test]
fn bench_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("bench", "foo");
}

#[test]
fn bench_dir_plus_file() {
    assert_not_a_cargo_toml("bench", "foo/bar");
}

#[test]
fn bench_dir_plus_path() {
    assert_not_a_cargo_toml("bench", "foo/bar/baz");
}

#[test]
fn bench_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("bench", "foo/bar/baz/Cargo.toml");
}

#[test]
fn build_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("build", "foo");
}

#[test]
fn build_dir_plus_file() {
    assert_not_a_cargo_toml("bench", "foo/bar");
}

#[test]
fn build_dir_plus_path() {
    assert_not_a_cargo_toml("bench", "foo/bar/baz");
}

#[test]
fn build_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("build", "foo/bar/baz/Cargo.toml");
}

#[test]
fn clean_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("clean", "foo");
}

#[test]
fn clean_dir_plus_file() {
    assert_not_a_cargo_toml("clean", "foo/bar");
}

#[test]
fn clean_dir_plus_path() {
    assert_not_a_cargo_toml("clean", "foo/bar/baz");
}

#[test]
fn clean_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("clean", "foo/bar/baz/Cargo.toml");
}

#[test]
fn doc_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("doc", "foo");
}

#[test]
fn doc_dir_plus_file() {
    assert_not_a_cargo_toml("doc", "foo/bar");
}

#[test]
fn doc_dir_plus_path() {
    assert_not_a_cargo_toml("doc", "foo/bar/baz");
}

#[test]
fn doc_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("doc", "foo/bar/baz/Cargo.toml");
}

#[test]
fn fetch_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("fetch", "foo");
}

#[test]
fn fetch_dir_plus_file() {
    assert_not_a_cargo_toml("fetch", "foo/bar");
}

#[test]
fn fetch_dir_plus_path() {
    assert_not_a_cargo_toml("fetch", "foo/bar/baz");
}

#[test]
fn fetch_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("fetch", "foo/bar/baz/Cargo.toml");
}

#[test]
fn generate_lockfile_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("generate-lockfile", "foo");
}

#[test]
fn generate_lockfile_dir_plus_file() {
    assert_not_a_cargo_toml("generate-lockfile", "foo/bar");
}

#[test]
fn generate_lockfile_dir_plus_path() {
    assert_not_a_cargo_toml("generate-lockfile", "foo/bar/baz");
}

#[test]
fn generate_lockfile_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("generate-lockfile", "foo/bar/baz/Cargo.toml");
}

#[test]
fn package_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("package", "foo");
}

#[test]
fn package_dir_plus_file() {
    assert_not_a_cargo_toml("package", "foo/bar");
}

#[test]
fn package_dir_plus_path() {
    assert_not_a_cargo_toml("package", "foo/bar/baz");
}

#[test]
fn package_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("package", "foo/bar/baz/Cargo.toml");
}

#[test]
fn pkgid_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("pkgid", "foo");
}

#[test]
fn pkgid_dir_plus_file() {
    assert_not_a_cargo_toml("pkgid", "foo/bar");
}

#[test]
fn pkgid_dir_plus_path() {
    assert_not_a_cargo_toml("pkgid", "foo/bar/baz");
}

#[test]
fn pkgid_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("pkgid", "foo/bar/baz/Cargo.toml");
}

#[test]
fn publish_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("publish", "foo");
}

#[test]
fn publish_dir_plus_file() {
    assert_not_a_cargo_toml("publish", "foo/bar");
}

#[test]
fn publish_dir_plus_path() {
    assert_not_a_cargo_toml("publish", "foo/bar/baz");
}

#[test]
fn publish_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("publish", "foo/bar/baz/Cargo.toml");
}

#[test]
fn read_manifest_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("read-manifest", "foo");
}

#[test]
fn read_manifest_dir_plus_file() {
    assert_not_a_cargo_toml("read-manifest", "foo/bar");
}

#[test]
fn read_manifest_dir_plus_path() {
    assert_not_a_cargo_toml("read-manifest", "foo/bar/baz");
}

#[test]
fn read_manifest_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("read-manifest", "foo/bar/baz/Cargo.toml");
}

#[test]
fn run_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("run", "foo");
}

#[test]
fn run_dir_plus_file() {
    assert_not_a_cargo_toml("run", "foo/bar");
}

#[test]
fn run_dir_plus_path() {
    assert_not_a_cargo_toml("run", "foo/bar/baz");
}

#[test]
fn run_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("run", "foo/bar/baz/Cargo.toml");
}

#[test]
fn rustc_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("rustc", "foo");
}

#[test]
fn rustc_dir_plus_file() {
    assert_not_a_cargo_toml("rustc", "foo/bar");
}

#[test]
fn rustc_dir_plus_path() {
    assert_not_a_cargo_toml("rustc", "foo/bar/baz");
}

#[test]
fn rustc_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("rustc", "foo/bar/baz/Cargo.toml");
}

#[test]
fn test_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("test", "foo");
}

#[test]
fn test_dir_plus_file() {
    assert_not_a_cargo_toml("test", "foo/bar");
}

#[test]
fn test_dir_plus_path() {
    assert_not_a_cargo_toml("test", "foo/bar/baz");
}

#[test]
fn test_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("test", "foo/bar/baz/Cargo.toml");
}

#[test]
fn update_dir_containing_cargo_toml() {
    assert_not_a_cargo_toml("update", "foo");
}

#[test]
fn update_dir_plus_file() {
    assert_not_a_cargo_toml("update", "foo/bar");
}

#[test]
fn update_dir_plus_path() {
    assert_not_a_cargo_toml("update", "foo/bar/baz");
}

#[test]
fn update_dir_to_nonexistent_cargo_toml() {
    assert_cargo_toml_doesnt_exist("update", "foo/bar/baz/Cargo.toml");
}

#[test]
fn verify_project_dir_containing_cargo_toml() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("verify-project")
                 .arg("--manifest-path").arg("foo")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(1)
                       .with_stdout("\
{\"invalid\":\"the manifest-path must be a path to a Cargo.toml file\"}\
                        "));
}

#[test]
fn verify_project_dir_plus_file() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("verify-project")
                 .arg("--manifest-path").arg("foo/bar")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(1)
                       .with_stdout("\
{\"invalid\":\"the manifest-path must be a path to a Cargo.toml file\"}\
                        "));
}

#[test]
fn verify_project_dir_plus_path() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("verify-project")
                 .arg("--manifest-path").arg("foo/bar/baz")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(1)
                       .with_stdout("\
{\"invalid\":\"the manifest-path must be a path to a Cargo.toml file\"}\
                        "));
}

#[test]
fn verify_project_dir_to_nonexistent_cargo_toml() {
    let p = project("foo");
    assert_that(p.cargo_process("verify-project")
                 .arg("--manifest-path").arg("foo/bar/baz/Cargo.toml")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(1)
                       .with_stdout("\
{\"invalid\":\"manifest path `foo[..]bar[..]baz[..]Cargo.toml` does not exist\"}\
                        "));
}
