//! Tests for the `cargo init` command.

use cargo_test_support::compare::assert;
use cargo_test_support::prelude::*;
use cargo_test_support::{command_is_available, paths, Project};
use std::fs;
use std::process::Command;

fn mercurial_available() -> bool {
    let result = Command::new("hg")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !result {
        println!("`hg` not available, skipping test");
    }
    result
}

#[cargo_test]
fn simple_lib() {
    let project = Project::from_template("tests/snapshots/init/simple_lib.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --vcs none --edition 2015")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/simple_lib.stdout")
        .stderr_matches_path("tests/snapshots/init/simple_lib.stderr");

    assert().subset_matches("tests/snapshots/init/simple_lib.out", project_root);
    assert!(!project_root.join(".gitignore").is_file());

    snapbox::cmd::Command::cargo()
        .current_dir(project_root)
        .arg("build")
        .assert()
        .success();
    assert!(!project.bin("foo").is_file());
}

#[cargo_test]
fn simple_bin() {
    let project = Project::from_template("tests/snapshots/init/simple_bin.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --bin --vcs none --edition 2015")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/simple_bin.stdout")
        .stderr_matches_path("tests/snapshots/init/simple_bin.stderr");

    assert().subset_matches("tests/snapshots/init/simple_bin.out", project_root);
    assert!(!project_root.join(".gitignore").is_file());

    snapbox::cmd::Command::cargo()
        .current_dir(project_root)
        .arg("build")
        .assert()
        .success();
    assert!(project.bin("case").is_file());
}

#[cargo_test]
fn simple_git_ignore_exists() {
    let project = Project::from_template("tests/snapshots/init/simple_git_ignore_exists.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --edition 2015")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/simple_git_ignore_exists.stdout")
        .stderr_matches_path("tests/snapshots/init/simple_git_ignore_exists.stderr");

    assert().subset_matches(
        "tests/snapshots/init/simple_git_ignore_exists.out",
        project_root,
    );
    assert!(project_root.join(".git").is_dir());

    snapbox::cmd::Command::cargo()
        .current_dir(project_root)
        .arg("build")
        .assert()
        .success();
}

#[cargo_test]
fn git_ignore_exists_no_conflicting_entries() {
    let project =
        Project::from_template("tests/snapshots/init/git_ignore_exists_no_conflicting_entries.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --edition 2015")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/git_ignore_exists_no_conflicting_entries.stdout")
        .stderr_matches_path(
            "tests/snapshots/init/git_ignore_exists_no_conflicting_entries.stderr",
        );

    assert().subset_matches(
        "tests/snapshots/init/git_ignore_exists_no_conflicting_entries.out",
        project_root,
    );
    assert!(project_root.join(".git").is_dir());
}

#[cargo_test]
fn both_lib_and_bin() {
    let cwd = paths::root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --bin")
        .current_dir(&cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/init/both_lib_and_bin.stdout")
        .stderr_matches_path("tests/snapshots/init/both_lib_and_bin.stderr");

    assert!(!cwd.join("Cargo.toml").is_file());
}

#[cargo_test]
fn bin_already_exists_explicit() {
    let project = Project::from_template("tests/snapshots/init/bin_already_exists_explicit.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --bin --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/bin_already_exists_explicit.stdout")
        .stderr_matches_path("tests/snapshots/init/bin_already_exists_explicit.stderr");

    assert().subset_matches(
        "tests/snapshots/init/bin_already_exists_explicit.out",
        project_root,
    );
}

#[cargo_test]
fn bin_already_exists_implicit() {
    let project = Project::from_template("tests/snapshots/init/bin_already_exists_implicit.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/bin_already_exists_implicit.stdout")
        .stderr_matches_path("tests/snapshots/init/bin_already_exists_implicit.stderr");

    assert().subset_matches(
        "tests/snapshots/init/bin_already_exists_implicit.out",
        project_root,
    );
}

#[cargo_test]
fn bin_already_exists_explicit_nosrc() {
    let project =
        Project::from_template("tests/snapshots/init/bin_already_exists_explicit_nosrc.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --bin --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/bin_already_exists_explicit_nosrc.stdout")
        .stderr_matches_path("tests/snapshots/init/bin_already_exists_explicit_nosrc.stderr");

    assert().subset_matches(
        "tests/snapshots/init/bin_already_exists_explicit_nosrc.out",
        project_root,
    );
    assert!(!project_root.join("src").is_dir());
}

#[cargo_test]
fn bin_already_exists_implicit_nosrc() {
    let project =
        Project::from_template("tests/snapshots/init/bin_already_exists_implicit_nosrc.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/bin_already_exists_implicit_nosrc.stdout")
        .stderr_matches_path("tests/snapshots/init/bin_already_exists_implicit_nosrc.stderr");

    assert().subset_matches(
        "tests/snapshots/init/bin_already_exists_implicit_nosrc.out",
        project_root,
    );
    assert!(!project_root.join("src").is_dir());
}

#[cargo_test]
fn bin_already_exists_implicit_namenosrc() {
    let project =
        Project::from_template("tests/snapshots/init/bin_already_exists_implicit_namenosrc.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/bin_already_exists_implicit_namenosrc.stdout")
        .stderr_matches_path("tests/snapshots/init/bin_already_exists_implicit_namenosrc.stderr");

    assert().subset_matches(
        "tests/snapshots/init/bin_already_exists_implicit_namenosrc.out",
        project_root,
    );
    assert!(!project_root.join("src").is_dir());
}

#[cargo_test]
fn bin_already_exists_implicit_namesrc() {
    let project =
        Project::from_template("tests/snapshots/init/bin_already_exists_implicit_namesrc.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/bin_already_exists_implicit_namesrc.stdout")
        .stderr_matches_path("tests/snapshots/init/bin_already_exists_implicit_namesrc.stderr");

    assert().subset_matches(
        "tests/snapshots/init/bin_already_exists_implicit_namesrc.out",
        project_root,
    );
    assert!(!project_root.join("src/main.rs").is_file());
}

#[cargo_test]
fn confused_by_multiple_lib_files() {
    let project = Project::from_template("tests/snapshots/init/confused_by_multiple_lib_files.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs none")
        .current_dir(project_root)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/init/confused_by_multiple_lib_files.stdout")
        .stderr_matches_path("tests/snapshots/init/confused_by_multiple_lib_files.stderr");

    assert().subset_matches(
        "tests/snapshots/init/confused_by_multiple_lib_files.out",
        project_root,
    );
    assert!(!project_root.join("Cargo.toml").is_file());
}

#[cargo_test]
fn multibin_project_name_clash() {
    let project = Project::from_template("tests/snapshots/init/multibin_project_name_clash.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --vcs none")
        .current_dir(project_root)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/init/multibin_project_name_clash.stdout")
        .stderr_matches_path("tests/snapshots/init/multibin_project_name_clash.stderr");

    assert().subset_matches(
        "tests/snapshots/init/multibin_project_name_clash.out",
        project_root,
    );
    assert!(!project_root.join("Cargo.toml").is_file());
}

#[cargo_test]
fn lib_already_exists_src() {
    let project = Project::from_template("tests/snapshots/init/lib_already_exists_src.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/lib_already_exists_src.stdout")
        .stderr_matches_path("tests/snapshots/init/lib_already_exists_src.stderr");

    assert().subset_matches(
        "tests/snapshots/init/lib_already_exists_src.out",
        project_root,
    );
    assert!(!project_root.join("src/main.rs").is_file());
}

#[cargo_test]
fn lib_already_exists_nosrc() {
    let project = Project::from_template("tests/snapshots/init/lib_already_exists_nosrc.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/lib_already_exists_nosrc.stdout")
        .stderr_matches_path("tests/snapshots/init/lib_already_exists_nosrc.stderr");

    assert().subset_matches(
        "tests/snapshots/init/lib_already_exists_nosrc.out",
        project_root,
    );
    assert!(!project_root.join("src/main.rs").is_file());
}

#[cargo_test]
fn simple_git() {
    let project = Project::from_template("tests/snapshots/init/simple_git.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --vcs git")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/simple_git.stdout")
        .stderr_matches_path("tests/snapshots/init/simple_git.stderr");

    assert().subset_matches("tests/snapshots/init/simple_git.out", project_root);
    assert!(project_root.join(".git").is_dir());
}

#[cargo_test]
fn auto_git() {
    let project = Project::from_template("tests/snapshots/init/auto_git.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/auto_git.stdout")
        .stderr_matches_path("tests/snapshots/init/auto_git.stderr");

    assert().subset_matches("tests/snapshots/init/auto_git.out", &project_root);
    assert!(project_root.join(".git").is_dir());
}

#[cargo_test]
fn invalid_dir_name() {
    let foo = &paths::root().join("foo.bar");
    fs::create_dir_all(foo).unwrap();

    snapbox::cmd::Command::cargo()
        .arg_line("init")
        .current_dir(foo)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/init/invalid_dir_name.stdout")
        .stderr_matches_path("tests/snapshots/init/invalid_dir_name.stderr");

    assert!(!foo.join("Cargo.toml").is_file());
}

#[cargo_test]
fn reserved_name() {
    let project_root = &paths::root().join("test");
    fs::create_dir_all(project_root).unwrap();

    snapbox::cmd::Command::cargo()
        .arg_line("init")
        .current_dir(project_root)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/init/reserved_name.stdout")
        .stderr_matches_path("tests/snapshots/init/reserved_name.stderr");

    assert!(!project_root.join("Cargo.toml").is_file());
}

#[cargo_test]
fn git_autodetect() {
    let project_root = &paths::root().join("foo");
    fs::create_dir_all(project_root.join(".git")).unwrap();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/git_autodetect.stdout")
        .stderr_matches_path("tests/snapshots/init/git_autodetect.stderr");

    assert().subset_matches("tests/snapshots/init/git_autodetect.out", project_root);
    assert!(project_root.join(".git").is_dir());
}

#[cargo_test]
fn mercurial_autodetect() {
    let project = Project::from_template("tests/snapshots/init/mercurial_autodetect.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/mercurial_autodetect.stdout")
        .stderr_matches_path("tests/snapshots/init/mercurial_autodetect.stderr");

    assert().subset_matches(
        "tests/snapshots/init/mercurial_autodetect.out",
        project_root,
    );
    assert!(!project_root.join(".git").is_dir());
}

#[cargo_test]
fn fossil_autodetect() {
    let project = Project::from_template("tests/snapshots/init/fossil_autodetect.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/fossil_autodetect.stdout")
        .stderr_matches_path("tests/snapshots/init/fossil_autodetect.stderr");

    assert().subset_matches("tests/snapshots/init/fossil_autodetect.out", project_root);
    assert!(!project_root.join(".git").is_dir());
}

#[cargo_test]
fn pijul_autodetect() {
    let project = Project::from_template("tests/snapshots/init/pijul_autodetect.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/pijul_autodetect.stdout")
        .stderr_matches_path("tests/snapshots/init/pijul_autodetect.stderr");

    assert().subset_matches("tests/snapshots/init/pijul_autodetect.out", project_root);
    assert!(!project_root.join(".git").is_dir());
}

#[cargo_test]
fn simple_hg() {
    if !mercurial_available() {
        return;
    }

    let project = Project::from_template("tests/snapshots/init/simple_hg.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --vcs hg")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/simple_hg.stdout")
        .stderr_matches_path("tests/snapshots/init/simple_hg.stderr");

    assert().subset_matches("tests/snapshots/init/simple_hg.out", project_root);
    assert!(!project_root.join(".git").is_dir());
}

#[cargo_test]
fn simple_hg_ignore_exists() {
    let project = Project::from_template("tests/snapshots/init/simple_hg_ignore_exists.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/simple_hg_ignore_exists.stdout")
        .stderr_matches_path("tests/snapshots/init/simple_hg_ignore_exists.stderr");

    assert().subset_matches(
        "tests/snapshots/init/simple_hg_ignore_exists.out",
        project_root,
    );
    assert!(!project_root.join(".git").is_dir());
}

#[cargo_test]
fn inferred_lib_with_git() {
    let project = Project::from_template("tests/snapshots/init/inferred_lib_with_git.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs git")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/inferred_lib_with_git.stdout")
        .stderr_matches_path("tests/snapshots/init/inferred_lib_with_git.stderr");

    assert().subset_matches(
        "tests/snapshots/init/inferred_lib_with_git.out",
        project_root,
    );
}

#[cargo_test]
fn explicit_bin_with_git() {
    let project = Project::from_template("tests/snapshots/init/explicit_bin_with_git.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs git --bin")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/explicit_bin_with_git.stdout")
        .stderr_matches_path("tests/snapshots/init/explicit_bin_with_git.stderr");

    assert().subset_matches(
        "tests/snapshots/init/explicit_bin_with_git.out",
        project_root,
    );
}

#[cargo_test]
fn inferred_bin_with_git() {
    let project = Project::from_template("tests/snapshots/init/inferred_bin_with_git.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --vcs git")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/inferred_bin_with_git.stdout")
        .stderr_matches_path("tests/snapshots/init/inferred_bin_with_git.stderr");

    assert().subset_matches(
        "tests/snapshots/init/inferred_bin_with_git.out",
        project_root,
    );
}

#[cargo_test]
fn with_argument() {
    let project = Project::from_template("tests/snapshots/init/with_argument.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init foo --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/with_argument.stdout")
        .stderr_matches_path("tests/snapshots/init/with_argument.stderr");

    assert().subset_matches("tests/snapshots/init/with_argument.out", project_root);
}

#[cargo_test]
fn unknown_flags() {
    snapbox::cmd::Command::cargo()
        .arg_line("init foo --flag")
        .current_dir(paths::root())
        .assert()
        .code(1)
        .stdout_matches_path("tests/snapshots/init/unknown_flags.stdout")
        .stderr_matches_path("tests/snapshots/init/unknown_flags.stderr");
}

#[cfg(not(windows))]
#[cargo_test]
fn no_filename() {
    snapbox::cmd::Command::cargo()
        .arg_line("init /")
        .current_dir(paths::root())
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/init/no_filename.stdout")
        .stderr_matches_path("tests/snapshots/init/no_filename.stderr");
}

#[cargo_test]
fn formats_source() {
    if !command_is_available("rustfmt") {
        return;
    }

    let project = Project::from_template("tests/snapshots/init/formats_source.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/formats_source.stdout")
        .stderr_matches_path("tests/snapshots/init/formats_source.stderr");

    assert().subset_matches("tests/snapshots/init/formats_source.out", project_root);
}

#[cargo_test]
fn ignores_failure_to_format_source() {
    let project =
        Project::from_template("tests/snapshots/init/ignores_failure_to_format_source.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --vcs none")
        .env("PATH", "") // pretend that `rustfmt` is missing
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/ignores_failure_to_format_source.stdout")
        .stderr_matches_path("tests/snapshots/init/ignores_failure_to_format_source.stderr");

    assert().subset_matches(
        "tests/snapshots/init/ignores_failure_to_format_source.out",
        project_root,
    );
}

#[cargo_test]
fn creates_binary_when_instructed_and_has_lib_file() {
    let project = Project::from_template(
        "tests/snapshots/init/creates_binary_when_instructed_and_has_lib_file.in",
    );
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --bin --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path(
            "tests/snapshots/init/creates_binary_when_instructed_and_has_lib_file.stdout",
        )
        .stderr_matches_path(
            "tests/snapshots/init/creates_binary_when_instructed_and_has_lib_file.stderr",
        );

    assert().subset_matches(
        "tests/snapshots/init/creates_binary_when_instructed_and_has_lib_file.out",
        project_root,
    );
}

#[cargo_test]
fn creates_library_when_instructed_and_has_bin_file() {
    let project = Project::from_template(
        "tests/snapshots/init/creates_library_when_instructed_and_has_bin_file.in",
    );
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path(
            "tests/snapshots/init/creates_library_when_instructed_and_has_bin_file.stdout",
        )
        .stderr_matches_path(
            "tests/snapshots/init/creates_library_when_instructed_and_has_bin_file.stderr",
        );

    assert().subset_matches(
        "tests/snapshots/init/creates_library_when_instructed_and_has_bin_file.out",
        project_root,
    );
}

#[cargo_test]
fn creates_binary_when_both_binlib_present() {
    let project =
        Project::from_template("tests/snapshots/init/creates_binary_when_both_binlib_present.in");
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --bin --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/init/creates_binary_when_both_binlib_present.stdout")
        .stderr_matches_path("tests/snapshots/init/creates_binary_when_both_binlib_present.stderr");

    assert().subset_matches(
        "tests/snapshots/init/creates_binary_when_both_binlib_present.out",
        project_root,
    );
}

#[cargo_test]
fn cant_create_library_when_both_binlib_present() {
    let project = Project::from_template(
        "tests/snapshots/init/cant_create_library_when_both_binlib_present.in",
    );
    let project_root = &project.root();

    snapbox::cmd::Command::cargo()
        .arg_line("init --lib")
        .current_dir(project_root)
        .assert()
        .code(101)
        .stdout_matches_path(
            "tests/snapshots/init/cant_create_library_when_both_binlib_present.stdout",
        )
        .stderr_matches_path(
            "tests/snapshots/init/cant_create_library_when_both_binlib_present.stderr",
        );
}
