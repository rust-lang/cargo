//! Tests for invalid --manifest-path arguments.

use crate::prelude::*;
use cargo_test_support::{basic_bin_manifest, main_file, project, str};

#[track_caller]
fn assert_bad_manifest_path(command: &str, manifest_path_argument: &str, stderr: impl IntoData) {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo(command)
        .arg("--manifest-path")
        .arg(manifest_path_argument)
        .cwd(p.root().parent().unwrap())
        .with_status(101)
        .with_stderr_data(stderr)
        .run();
}

#[cargo_test]
fn bench_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "bench",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn bench_dir_plus_file() {
    assert_bad_manifest_path(
        "bench",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn bench_dir_plus_path() {
    assert_bad_manifest_path(
        "bench",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn bench_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "bench",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn build_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "check",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn build_dir_plus_file() {
    assert_bad_manifest_path(
        "bench",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn build_dir_plus_path() {
    assert_bad_manifest_path(
        "bench",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn build_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "check",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn clean_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "clean",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn clean_dir_plus_file() {
    assert_bad_manifest_path(
        "clean",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn clean_dir_plus_path() {
    assert_bad_manifest_path(
        "clean",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn clean_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "clean",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn doc_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "doc",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn doc_dir_plus_file() {
    assert_bad_manifest_path(
        "doc",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn doc_dir_plus_path() {
    assert_bad_manifest_path(
        "doc",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn doc_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "doc",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn fetch_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "fetch",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn fetch_dir_plus_file() {
    assert_bad_manifest_path(
        "fetch",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn fetch_dir_plus_path() {
    assert_bad_manifest_path(
        "fetch",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn fetch_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "fetch",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn generate_lockfile_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "generate-lockfile",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn generate_lockfile_dir_plus_file() {
    assert_bad_manifest_path(
        "generate-lockfile",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn generate_lockfile_dir_plus_path() {
    assert_bad_manifest_path(
        "generate-lockfile",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn generate_lockfile_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "generate-lockfile",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn package_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "package",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn package_dir_plus_file() {
    assert_bad_manifest_path(
        "package",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn package_dir_plus_path() {
    assert_bad_manifest_path(
        "package",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn package_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "package",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn pkgid_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "pkgid",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn pkgid_dir_plus_file() {
    assert_bad_manifest_path(
        "pkgid",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn pkgid_dir_plus_path() {
    assert_bad_manifest_path(
        "pkgid",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn pkgid_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "pkgid",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn publish_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "publish",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn publish_dir_plus_file() {
    assert_bad_manifest_path(
        "publish",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn publish_dir_plus_path() {
    assert_bad_manifest_path(
        "publish",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn publish_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "publish",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn read_manifest_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "read-manifest",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn read_manifest_dir_plus_file() {
    assert_bad_manifest_path(
        "read-manifest",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn read_manifest_dir_plus_path() {
    assert_bad_manifest_path(
        "read-manifest",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn read_manifest_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "read-manifest",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn run_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "run",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn run_dir_plus_file() {
    assert_bad_manifest_path(
        "run",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn run_dir_plus_path() {
    assert_bad_manifest_path(
        "run",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn run_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "run",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn rustc_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "rustc",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn rustc_dir_plus_file() {
    assert_bad_manifest_path(
        "rustc",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn rustc_dir_plus_path() {
    assert_bad_manifest_path(
        "rustc",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn rustc_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "rustc",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn test_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "test",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn test_dir_plus_file() {
    assert_bad_manifest_path(
        "test",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn test_dir_plus_path() {
    assert_bad_manifest_path(
        "test",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn test_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "test",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
}

#[cargo_test]
fn update_dir_containing_cargo_toml() {
    assert_bad_manifest_path(
        "update",
        "foo",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`

"#]],
    );
}

#[cargo_test]
fn update_dir_plus_file() {
    assert_bad_manifest_path(
        "update",
        "foo/bar",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`

"#]],
    );
}

#[cargo_test]
fn update_dir_plus_path() {
    assert_bad_manifest_path(
        "update",
        "foo/bar/baz",
        str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`

"#]],
    );
}

#[cargo_test]
fn update_dir_to_nonexistent_cargo_toml() {
    assert_bad_manifest_path(
        "update",
        "foo/bar/baz/Cargo.toml",
        str![[r#"
[ERROR] manifest path `foo/bar/baz/Cargo.toml` does not exist

"#]],
    );
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
        .with_stdout_data(
            str![[r#"
[
  {
    "invalid": "the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo`"
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
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
        .with_stdout_data(
            str![[r#"
[
  {
    "invalid": "the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar`"
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
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
        .with_stdout_data(
            str![[r#"
[
  {
    "invalid": "the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/bar/baz`"
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}

#[cargo_test]
fn verify_project_dir_to_nonexistent_cargo_toml() {
    let p = project().build();
    p.cargo("verify-project --manifest-path foo/bar/baz/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .with_status(1)
        .with_stdout_data(
            str![[r#"
[
  {
    "invalid": "manifest path `foo/bar/baz/Cargo.toml` does not exist"
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}
