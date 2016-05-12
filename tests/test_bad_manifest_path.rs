use support::{project, execs, main_file, basic_bin_manifest};
use hamcrest::{assert_that};

fn setup() {}

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

#[allow(deprecated)] // connect => join in 1.3
fn assert_cargo_toml_doesnt_exist(command: &str, manifest_path_argument: &str) {
    let p = project("foo");
    let expected_path = manifest_path_argument
        .split("/").collect::<Vec<_>>().connect("[..]");

    assert_that(p.cargo_process(command)
                 .arg("--manifest-path").arg(manifest_path_argument)
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(101)
                       .with_stderr(
                           format!("[ERROR] manifest path `{}` does not exist",
                                   expected_path)
                       ));
}

test!(bench_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("bench", "foo");
});

test!(bench_dir_plus_file {
    assert_not_a_cargo_toml("bench", "foo/bar");
});

test!(bench_dir_plus_path {
    assert_not_a_cargo_toml("bench", "foo/bar/baz");
});

test!(bench_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("bench", "foo/bar/baz/Cargo.toml");
});

test!(build_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("build", "foo");
});

test!(build_dir_plus_file {
    assert_not_a_cargo_toml("bench", "foo/bar");
});

test!(build_dir_plus_path {
    assert_not_a_cargo_toml("bench", "foo/bar/baz");
});

test!(build_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("build", "foo/bar/baz/Cargo.toml");
});

test!(clean_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("clean", "foo");
});

test!(clean_dir_plus_file {
    assert_not_a_cargo_toml("clean", "foo/bar");
});

test!(clean_dir_plus_path {
    assert_not_a_cargo_toml("clean", "foo/bar/baz");
});

test!(clean_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("clean", "foo/bar/baz/Cargo.toml");
});

test!(doc_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("doc", "foo");
});

test!(doc_dir_plus_file {
    assert_not_a_cargo_toml("doc", "foo/bar");
});

test!(doc_dir_plus_path {
    assert_not_a_cargo_toml("doc", "foo/bar/baz");
});

test!(doc_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("doc", "foo/bar/baz/Cargo.toml");
});

test!(fetch_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("fetch", "foo");
});

test!(fetch_dir_plus_file {
    assert_not_a_cargo_toml("fetch", "foo/bar");
});

test!(fetch_dir_plus_path {
    assert_not_a_cargo_toml("fetch", "foo/bar/baz");
});

test!(fetch_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("fetch", "foo/bar/baz/Cargo.toml");
});

test!(generate_lockfile_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("generate-lockfile", "foo");
});

test!(generate_lockfile_dir_plus_file {
    assert_not_a_cargo_toml("generate-lockfile", "foo/bar");
});

test!(generate_lockfile_dir_plus_path {
    assert_not_a_cargo_toml("generate-lockfile", "foo/bar/baz");
});

test!(generate_lockfile_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("generate-lockfile", "foo/bar/baz/Cargo.toml");
});

test!(package_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("package", "foo");
});

test!(package_dir_plus_file {
    assert_not_a_cargo_toml("package", "foo/bar");
});

test!(package_dir_plus_path {
    assert_not_a_cargo_toml("package", "foo/bar/baz");
});

test!(package_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("package", "foo/bar/baz/Cargo.toml");
});

test!(pkgid_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("pkgid", "foo");
});

test!(pkgid_dir_plus_file {
    assert_not_a_cargo_toml("pkgid", "foo/bar");
});

test!(pkgid_dir_plus_path {
    assert_not_a_cargo_toml("pkgid", "foo/bar/baz");
});

test!(pkgid_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("pkgid", "foo/bar/baz/Cargo.toml");
});

test!(publish_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("publish", "foo");
});

test!(publish_dir_plus_file {
    assert_not_a_cargo_toml("publish", "foo/bar");
});

test!(publish_dir_plus_path {
    assert_not_a_cargo_toml("publish", "foo/bar/baz");
});

test!(publish_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("publish", "foo/bar/baz/Cargo.toml");
});

test!(read_manifest_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("read-manifest", "foo");
});

test!(read_manifest_dir_plus_file {
    assert_not_a_cargo_toml("read-manifest", "foo/bar");
});

test!(read_manifest_dir_plus_path {
    assert_not_a_cargo_toml("read-manifest", "foo/bar/baz");
});

test!(read_manifest_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("read-manifest", "foo/bar/baz/Cargo.toml");
});

test!(run_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("run", "foo");
});

test!(run_dir_plus_file {
    assert_not_a_cargo_toml("run", "foo/bar");
});

test!(run_dir_plus_path {
    assert_not_a_cargo_toml("run", "foo/bar/baz");
});

test!(run_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("run", "foo/bar/baz/Cargo.toml");
});

test!(rustc_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("rustc", "foo");
});

test!(rustc_dir_plus_file {
    assert_not_a_cargo_toml("rustc", "foo/bar");
});

test!(rustc_dir_plus_path {
    assert_not_a_cargo_toml("rustc", "foo/bar/baz");
});

test!(rustc_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("rustc", "foo/bar/baz/Cargo.toml");
});

test!(test_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("test", "foo");
});

test!(test_dir_plus_file {
    assert_not_a_cargo_toml("test", "foo/bar");
});

test!(test_dir_plus_path {
    assert_not_a_cargo_toml("test", "foo/bar/baz");
});

test!(test_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("test", "foo/bar/baz/Cargo.toml");
});

test!(update_dir_containing_cargo_toml {
    assert_not_a_cargo_toml("update", "foo");
});

test!(update_dir_plus_file {
    assert_not_a_cargo_toml("update", "foo/bar");
});

test!(update_dir_plus_path {
    assert_not_a_cargo_toml("update", "foo/bar/baz");
});

test!(update_dir_to_nonexistent_cargo_toml {
    assert_cargo_toml_doesnt_exist("update", "foo/bar/baz/Cargo.toml");
});

test!(verify_project_dir_containing_cargo_toml {
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
});

test!(verify_project_dir_plus_file {
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
});

test!(verify_project_dir_plus_path {
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
});

test!(verify_project_dir_to_nonexistent_cargo_toml {
    let p = project("foo");
    assert_that(p.cargo_process("verify-project")
                 .arg("--manifest-path").arg("foo/bar/baz/Cargo.toml")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(1)
                       .with_stdout("\
{\"invalid\":\"manifest path `foo[..]bar[..]baz[..]Cargo.toml` does not exist\"}\
                        "));
});
