pub fn cargo_exe() -> &'static std::path::Path {
    snapbox::cmd::cargo_bin!("cargo")
}

pub fn cargo_command() -> snapbox::cmd::Command {
    let mut cmd = snapbox::cmd::Command::new(cargo_exe()).with_assert(assert());

    // In general just clear out all cargo-specific configuration already in the
    // environment. Our tests all assume a "default configuration" unless
    // specified otherwise.
    for (k, _v) in std::env::vars() {
        if k.starts_with("CARGO_") {
            cmd = cmd.env_remove(&k);
        }
    }

    cmd = cmd
        .env("HOME", cargo_test_support::paths::home())
        .env(
            "CARGO_HOME",
            cargo_test_support::paths::home().join(".cargo"),
        )
        .env("__CARGO_TEST_ROOT", cargo_test_support::paths::root())
        // Force Cargo to think it's on the stable channel for all tests, this
        // should hopefully not surprise us as we add cargo features over time and
        // cargo rides the trains.
        .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "stable")
        // For now disable incremental by default as support hasn't ridden to the
        // stable channel yet. Once incremental support hits the stable compiler we
        // can switch this to one and then fix the tests.
        .env("CARGO_INCREMENTAL", "0")
        .env_remove("__CARGO_DEFAULT_LIB_METADATA")
        .env_remove("RUSTC")
        .env_remove("RUSTDOC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTFLAGS")
        .env_remove("RUSTDOCFLAGS")
        .env_remove("XDG_CONFIG_HOME") // see #2345
        .env("GIT_CONFIG_NOSYSTEM", "1") // keep trying to sandbox ourselves
        .env_remove("EMAIL")
        .env_remove("USER") // not set on some rust-lang docker images
        .env_remove("MFLAGS")
        .env_remove("MAKEFLAGS")
        .env_remove("GIT_AUTHOR_NAME")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_NAME")
        .env_remove("GIT_COMMITTER_EMAIL")
        .env_remove("MSYSTEM"); // assume cmd.exe everywhere on windows

    cmd
}

pub fn project_from_template(template_path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    let root = cargo_test_support::paths::root();
    let project_root = root.join("case");
    snapbox::path::copy_template(template_path.as_ref(), &project_root).unwrap();
    project_root
}

pub fn assert() -> snapbox::Assert {
    let root = cargo_test_support::paths::root();
    // Use `from_file_path` instead of `from_dir_path` so the trailing slash is
    // put in the users output, rather than hidden in the variable
    let root_url = url::Url::from_file_path(&root).unwrap().to_string();
    let root = root.display().to_string();

    let mut subs = snapbox::Substitutions::new();
    subs.extend([
        (
            "[EXE]",
            std::borrow::Cow::Borrowed(std::env::consts::EXE_SUFFIX),
        ),
        ("[ROOT]", std::borrow::Cow::Owned(root)),
        ("[ROOTURL]", std::borrow::Cow::Owned(root_url)),
    ])
    .unwrap();
    snapbox::Assert::new()
        .action_env(snapbox::DEFAULT_ACTION_ENV)
        .substitutions(subs)
}

fn init_registry() {
    cargo_test_support::registry::init();
    add_registry_packages(false);
}

fn init_alt_registry() {
    cargo_test_support::registry::alt_init();
    add_registry_packages(true);
}

fn add_registry_packages(alt: bool) {
    for name in [
        "my-package",
        "my-package1",
        "my-package2",
        "my-dev-package1",
        "my-dev-package2",
        "my-build-package1",
        "my-build-package2",
        "toml",
        "versioned-package",
        "cargo-list-test-fixture-dependency",
        "unrelateed-crate",
    ] {
        cargo_test_support::registry::Package::new(name, "0.1.1+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "0.2.3+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "0.4.1+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "20.0.0+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "99999.0.0+my-package")
            .alternative(alt)
            .publish();
        cargo_test_support::registry::Package::new(name, "99999.0.0-alpha.1+my-package")
            .alternative(alt)
            .publish();
    }

    cargo_test_support::registry::Package::new("prerelease_only", "0.2.0-alpha.1")
        .alternative(alt)
        .publish();
    cargo_test_support::registry::Package::new("test_breaking", "0.2.0")
        .alternative(alt)
        .publish();
    cargo_test_support::registry::Package::new("test_nonbreaking", "0.1.1")
        .alternative(alt)
        .publish();

    // Normalization
    cargo_test_support::registry::Package::new("linked-hash-map", "0.5.4")
        .alternative(alt)
        .feature("clippy", &[])
        .feature("heapsize", &[])
        .feature("heapsize_impl", &[])
        .feature("nightly", &[])
        .feature("serde", &[])
        .feature("serde_impl", &[])
        .feature("serde_test", &[])
        .publish();
    cargo_test_support::registry::Package::new("inflector", "0.11.4")
        .alternative(alt)
        .feature("default", &["heavyweight", "lazy_static", "regex"])
        .feature("heavyweight", &[])
        .feature("lazy_static", &[])
        .feature("regex", &[])
        .feature("unstable", &[])
        .publish();

    cargo_test_support::registry::Package::new("your-face", "99999.0.0+my-package")
        .alternative(alt)
        .feature("nose", &[])
        .feature("mouth", &[])
        .feature("eyes", &[])
        .feature("ears", &[])
        .publish();
}

#[cargo_test]
fn add_basic() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/add_basic.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/add_basic.stdout")
        .stderr_matches_path("tests/snapshots/add/add_basic.stderr");

    assert().subset_matches("tests/snapshots/add/add_basic.out", &project_root);
}

#[cargo_test]
fn add_multiple() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/add_multiple.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/add_multiple.stdout")
        .stderr_matches_path("tests/snapshots/add/add_multiple.stderr");

    assert().subset_matches("tests/snapshots/add/add_multiple.out", &project_root);
}

#[cargo_test]
fn quiet() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/quiet.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["--quiet", "your-face"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/quiet.stdout")
        .stderr_matches_path("tests/snapshots/add/quiet.stderr");

    assert().subset_matches("tests/snapshots/add/quiet.out", &project_root);
}

#[cargo_test]
fn add_normalized_name_external() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/add_normalized_name_external.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["linked_hash_map", "Inflector"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/add_normalized_name_external.stdout")
        .stderr_matches_path("tests/snapshots/add/add_normalized_name_external.stderr");

    assert().subset_matches(
        "tests/snapshots/add/add_normalized_name_external.out",
        &project_root,
    );
}

#[cargo_test]
fn infer_prerelease() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/infer_prerelease.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["prerelease_only"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/infer_prerelease.stdout")
        .stderr_matches_path("tests/snapshots/add/infer_prerelease.stderr");

    assert().subset_matches("tests/snapshots/add/infer_prerelease.out", &project_root);
}

#[cargo_test]
fn build() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/build.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["--build", "my-build-package1", "my-build-package2"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/build.stdout")
        .stderr_matches_path("tests/snapshots/add/build.stderr");

    assert().subset_matches("tests/snapshots/add/build.out", &project_root);
}

#[cargo_test]
fn build_prefer_existing_version() {
    init_alt_registry();
    let project_root =
        project_from_template("tests/snapshots/add/build_prefer_existing_version.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["cargo-list-test-fixture-dependency", "--build"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/build_prefer_existing_version.stdout")
        .stderr_matches_path("tests/snapshots/add/build_prefer_existing_version.stderr");

    assert().subset_matches(
        "tests/snapshots/add/build_prefer_existing_version.out",
        &project_root,
    );
}

#[cargo_test]
fn default_features() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/default_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--default-features"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/default_features.stdout")
        .stderr_matches_path("tests/snapshots/add/default_features.stderr");

    assert().subset_matches("tests/snapshots/add/default_features.out", &project_root);
}

#[cargo_test]
fn require_weak() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/require_weak.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--no-optional"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/require_weak.stdout")
        .stderr_matches_path("tests/snapshots/add/require_weak.stderr");

    assert().subset_matches("tests/snapshots/add/require_weak.out", &project_root);
}

#[cargo_test]
fn dev() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/dev.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["--dev", "my-dev-package1", "my-dev-package2"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/dev.stdout")
        .stderr_matches_path("tests/snapshots/add/dev.stderr");

    assert().subset_matches("tests/snapshots/add/dev.out", &project_root);
}

#[cargo_test]
fn dev_build_conflict() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/dev_build_conflict.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package", "--dev", "--build"])
        .current_dir(cwd)
        .assert()
        .code(1)
        .stdout_matches_path("tests/snapshots/add/dev_build_conflict.stdout")
        .stderr_matches_path("tests/snapshots/add/dev_build_conflict.stderr");

    assert().subset_matches("tests/snapshots/add/dev_build_conflict.out", &project_root);
}

#[cargo_test]
fn dev_prefer_existing_version() {
    init_alt_registry();
    let project_root = project_from_template("tests/snapshots/add/dev_prefer_existing_version.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["cargo-list-test-fixture-dependency", "--dev"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/dev_prefer_existing_version.stdout")
        .stderr_matches_path("tests/snapshots/add/dev_prefer_existing_version.stderr");

    assert().subset_matches(
        "tests/snapshots/add/dev_prefer_existing_version.out",
        &project_root,
    );
}

#[cargo_test]
fn dry_run() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/dry_run.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package", "--dry-run"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/dry_run.stdout")
        .stderr_matches_path("tests/snapshots/add/dry_run.stderr");

    assert().subset_matches("tests/snapshots/add/dry_run.out", &project_root);
}

#[cargo_test]
fn features() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--features", "eyes"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/features.stdout")
        .stderr_matches_path("tests/snapshots/add/features.stderr");

    assert().subset_matches("tests/snapshots/add/features.out", &project_root);
}

#[cargo_test]
fn features_empty() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/features_empty.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--features", ""])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/features_empty.stdout")
        .stderr_matches_path("tests/snapshots/add/features_empty.stderr");

    assert().subset_matches("tests/snapshots/add/features_empty.out", &project_root);
}

#[cargo_test]
fn features_multiple_occurrences() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/features_multiple_occurrences.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--features", "eyes", "--features", "nose"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/features_multiple_occurrences.stdout")
        .stderr_matches_path("tests/snapshots/add/features_multiple_occurrences.stderr");

    assert().subset_matches(
        "tests/snapshots/add/features_multiple_occurrences.out",
        &project_root,
    );
}

#[cargo_test]
fn features_preserve() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/features_preserve.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/features_preserve.stdout")
        .stderr_matches_path("tests/snapshots/add/features_preserve.stderr");

    assert().subset_matches("tests/snapshots/add/features_preserve.out", &project_root);
}

#[cargo_test]
fn features_spaced_values() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/features_spaced_values.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--features", "eyes nose"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/features_spaced_values.stdout")
        .stderr_matches_path("tests/snapshots/add/features_spaced_values.stderr");

    assert().subset_matches(
        "tests/snapshots/add/features_spaced_values.out",
        &project_root,
    );
}

#[cargo_test]
fn features_unknown() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/features_unknown.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--features", "noze"])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/features_unknown.stdout")
        .stderr_matches_path("tests/snapshots/add/features_unknown.stderr");

    assert().subset_matches("tests/snapshots/add/features_unknown.out", &project_root);
}

#[cargo_test]
fn git() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("git-package", "0.3.0+git-package"),
            )
            .file("src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["git-package", "--git", &git_url])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/git.stdout")
        .stderr_matches_path("tests/snapshots/add/git.stderr");

    assert().subset_matches("tests/snapshots/add/git.out", &project_root);
}

#[cargo_test]
fn git_inferred_name() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_inferred_name.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("git-package", "0.3.0+git-package"),
            )
            .file("src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["--git", &git_url])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/git_inferred_name.stdout")
        .stderr_matches_path("tests/snapshots/add/git_inferred_name.stderr");

    assert().subset_matches("tests/snapshots/add/git_inferred_name.out", &project_root);
}

#[cargo_test]
fn git_inferred_name_multiple() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_inferred_name_multiple.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file(
                "p1/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package1", "0.3.0+my-package1"),
            )
            .file("p1/src/lib.rs", "")
            .file(
                "p2/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package2", "0.3.0+my-package2"),
            )
            .file("p2/src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["--git", &git_url])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/git_inferred_name_multiple.stdout")
        .stderr_matches_path("tests/snapshots/add/git_inferred_name_multiple.stderr");

    assert().subset_matches(
        "tests/snapshots/add/git_inferred_name_multiple.out",
        &project_root,
    );
}

#[cargo_test]
fn git_normalized_name() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_normalized_name.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("git-package", "0.3.0+git-package"),
            )
            .file("src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["git_package", "--git", &git_url])
        .current_dir(cwd)
        .assert()
        .failure() // Fuzzy searching for paths isn't supported at this time
        .stdout_matches_path("tests/snapshots/add/git_normalized_name.stdout")
        .stderr_matches_path("tests/snapshots/add/git_normalized_name.stderr");

    assert().subset_matches("tests/snapshots/add/git_normalized_name.out", &project_root);
}

#[cargo_test]
fn invalid_git_name() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_git_name.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("git-package", "0.3.0+git-package"),
            )
            .file("src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["not-in-git", "--git", &git_url])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/invalid_git_name.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_git_name.stderr");

    assert().subset_matches("tests/snapshots/add/invalid_git_name.out", &project_root);
}

#[cargo_test]
fn git_branch() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_branch.in");
    let cwd = &project_root;
    let (git_dep, git_repo) = cargo_test_support::git::new_repo("git-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("git-package", "0.3.0+git-package"),
            )
            .file("src/lib.rs", "")
    });
    let branch = "dev";
    let find_head = || (git_repo.head().unwrap().peel_to_commit().unwrap());
    git_repo.branch(branch, &find_head(), false).unwrap();
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["git-package", "--git", &git_url, "--branch", branch])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/git_branch.stdout")
        .stderr_matches_path("tests/snapshots/add/git_branch.stderr");

    assert().subset_matches("tests/snapshots/add/git_branch.out", &project_root);
}

#[cargo_test]
fn git_conflicts_namever() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_conflicts_namever.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args([
            "my-package@0.4.3",
            "--git",
            "https://github.com/dcjanus/invalid",
        ])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/git_conflicts_namever.stdout")
        .stderr_matches_path("tests/snapshots/add/git_conflicts_namever.stderr");

    assert().subset_matches(
        "tests/snapshots/add/git_conflicts_namever.out",
        &project_root,
    );
}

#[cargo_test]
fn git_registry() {
    init_alt_registry();
    let project_root = project_from_template("tests/snapshots/add/git_registry.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("versioned-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("versioned-package", "0.3.0+versioned-package"),
            )
            .file("src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args([
            "versioned-package",
            "--git",
            &git_url,
            "--registry",
            "alternative",
        ])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/git_registry.stdout")
        .stderr_matches_path("tests/snapshots/add/git_registry.stderr");

    assert().subset_matches("tests/snapshots/add/git_registry.out", &project_root);
}

#[cargo_test]
fn git_dev() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_dev.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("git-package", "0.3.0+git-package"),
            )
            .file("src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["git-package", "--git", &git_url, "--dev"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/git_dev.stdout")
        .stderr_matches_path("tests/snapshots/add/git_dev.stderr");

    assert().subset_matches("tests/snapshots/add/git_dev.out", &project_root);
}

#[cargo_test]
fn git_rev() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_rev.in");
    let cwd = &project_root;
    let (git_dep, git_repo) = cargo_test_support::git::new_repo("git-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("git-package", "0.3.0+git-package"),
            )
            .file("src/lib.rs", "")
    });
    let find_head = || (git_repo.head().unwrap().peel_to_commit().unwrap());
    let head = find_head().id().to_string();
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["git-package", "--git", &git_url, "--rev", &head])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/git_rev.stdout")
        .stderr_matches_path("tests/snapshots/add/git_rev.stderr");

    assert().subset_matches("tests/snapshots/add/git_rev.out", &project_root);
}

#[cargo_test]
fn git_tag() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_tag.in");
    let cwd = &project_root;
    let (git_dep, git_repo) = cargo_test_support::git::new_repo("git-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("git-package", "0.3.0+git-package"),
            )
            .file("src/lib.rs", "")
    });
    let tag = "v1.0.0";
    cargo_test_support::git::tag(&git_repo, tag);
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["git-package", "--git", &git_url, "--tag", tag])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/git_tag.stdout")
        .stderr_matches_path("tests/snapshots/add/git_tag.stderr");

    assert().subset_matches("tests/snapshots/add/git_tag.out", &project_root);
}

#[cargo_test]
fn path() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/path.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "cargo-list-test-fixture-dependency",
            "--path",
            "../dependency",
        ])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/path.stdout")
        .stderr_matches_path("tests/snapshots/add/path.stderr");

    assert().subset_matches("tests/snapshots/add/path.out", &project_root);
}

#[cargo_test]
fn path_inferred_name() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/path_inferred_name.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "cargo-list-test-fixture-dependency",
            "--path",
            "../dependency",
        ])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/path_inferred_name.stdout")
        .stderr_matches_path("tests/snapshots/add/path_inferred_name.stderr");

    assert().subset_matches("tests/snapshots/add/path_inferred_name.out", &project_root);
}

#[cargo_test]
fn path_inferred_name_conflicts_full_feature() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/path_inferred_name_conflicts_full_feature.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args(["--path", "../dependency", "--features", "your-face/nose"])
        .current_dir(&cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/path_inferred_name_conflicts_full_feature.stdout")
        .stderr_matches_path(
            "tests/snapshots/add/path_inferred_name_conflicts_full_feature.stderr",
        );

    assert().subset_matches(
        "tests/snapshots/add/path_inferred_name_conflicts_full_feature.out",
        &project_root,
    );
}

#[cargo_test]
fn path_normalized_name() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/path_normalized_name.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "cargo_list_test_fixture_dependency",
            "--path",
            "../dependency",
        ])
        .current_dir(&cwd)
        .assert()
        .failure() // Fuzzy searching for paths isn't supported at this time
        .stdout_matches_path("tests/snapshots/add/path_normalized_name.stdout")
        .stderr_matches_path("tests/snapshots/add/path_normalized_name.stderr");

    assert().subset_matches(
        "tests/snapshots/add/path_normalized_name.out",
        &project_root,
    );
}

#[cargo_test]
fn invalid_path_name() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_path_name.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args(["not-at-path", "--path", "../dependency"])
        .current_dir(&cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/invalid_path_name.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_path_name.stderr");

    assert().subset_matches("tests/snapshots/add/invalid_path_name.out", &project_root);
}

#[cargo_test]
fn path_dev() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/path_dev.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "cargo-list-test-fixture-dependency",
            "--path",
            "../dependency",
            "--dev",
        ])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/path_dev.stdout")
        .stderr_matches_path("tests/snapshots/add/path_dev.stderr");

    assert().subset_matches("tests/snapshots/add/path_dev.out", &project_root);
}

#[cargo_test]
fn invalid_arg() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_arg.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package", "--flag"])
        .current_dir(cwd)
        .assert()
        .code(1)
        .stdout_matches_path("tests/snapshots/add/invalid_arg.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_arg.stderr");

    assert().subset_matches("tests/snapshots/add/invalid_arg.out", &project_root);
}

#[cargo_test]
fn invalid_git_external() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_git_external.in");
    let cwd = &project_root;
    let git_url = url::Url::from_directory_path(cwd.join("does-not-exist"))
        .unwrap()
        .to_string();

    cargo_command()
        .arg("add")
        .args(["fake-git", "--git", &git_url])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/invalid_git_external.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_git_external.stderr");

    assert().subset_matches(
        "tests/snapshots/add/invalid_git_external.out",
        &project_root,
    );
}

#[cargo_test]
fn invalid_path() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_path.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args([
            "cargo-list-test-fixture",
            "--path",
            "./tests/fixtures/local",
        ])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/invalid_path.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_path.stderr");

    assert().subset_matches("tests/snapshots/add/invalid_path.out", &project_root);
}

#[cargo_test]
fn invalid_path_self() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_path_self.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["cargo-list-test-fixture", "--path", "."])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/invalid_path_self.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_path_self.stderr");

    assert().subset_matches("tests/snapshots/add/invalid_path_self.out", &project_root);
}

#[cargo_test]
fn invalid_manifest() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_manifest.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package"])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/invalid_manifest.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_manifest.stderr");

    assert().subset_matches("tests/snapshots/add/invalid_manifest.out", &project_root);
}

#[cargo_test]
fn invalid_name_external() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_name_external.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["lets_hope_nobody_ever_publishes_this_crate"])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/invalid_name_external.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_name_external.stderr");

    assert().subset_matches(
        "tests/snapshots/add/invalid_name_external.out",
        &project_root,
    );
}

#[cargo_test]
fn invalid_target_empty() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_target_empty.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package", "--target", ""])
        .current_dir(cwd)
        .assert()
        .code(1)
        .stdout_matches_path("tests/snapshots/add/invalid_target_empty.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_target_empty.stderr");

    assert().subset_matches(
        "tests/snapshots/add/invalid_target_empty.out",
        &project_root,
    );
}

#[cargo_test]
fn invalid_vers() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/invalid_vers.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package@invalid version string"])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/invalid_vers.stdout")
        .stderr_matches_path("tests/snapshots/add/invalid_vers.stderr");

    assert().subset_matches("tests/snapshots/add/invalid_vers.out", &project_root);
}

#[cargo_test]
fn list_features() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/list_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/list_features.stdout")
        .stderr_matches_path("tests/snapshots/add/list_features.stderr");

    assert().subset_matches("tests/snapshots/add/list_features.out", &project_root);
}

#[cargo_test]
fn list_features_path() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/list_features_path.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args(["your-face", "--path", "../dependency"])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/list_features_path.stdout")
        .stderr_matches_path("tests/snapshots/add/list_features_path.stderr");

    assert().subset_matches("tests/snapshots/add/list_features_path.out", &project_root);
}

#[cargo_test]
fn list_features_path_no_default() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/list_features_path_no_default.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "your-face",
            "--path",
            "../dependency",
            "--no-default-features",
        ])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/list_features_path_no_default.stdout")
        .stderr_matches_path("tests/snapshots/add/list_features_path_no_default.stderr");

    assert().subset_matches(
        "tests/snapshots/add/list_features_path_no_default.out",
        &project_root,
    );
}

#[cargo_test]
fn manifest_path_package() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/manifest_path_package.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args([
            "--manifest-path",
            "Cargo.toml",
            "--package",
            "cargo-list-test-fixture",
            "cargo-list-test-fixture-dependency",
        ])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/manifest_path_package.stdout")
        .stderr_matches_path("tests/snapshots/add/manifest_path_package.stderr");

    assert().subset_matches(
        "tests/snapshots/add/manifest_path_package.out",
        &project_root,
    );
}

#[cargo_test]
fn multiple_conflicts_with_features() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/multiple_conflicts_with_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "your-face", "--features", "nose"])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/multiple_conflicts_with_features.stdout")
        .stderr_matches_path("tests/snapshots/add/multiple_conflicts_with_features.stderr");

    assert().subset_matches(
        "tests/snapshots/add/multiple_conflicts_with_features.out",
        &project_root,
    );
}

#[cargo_test]
fn git_multiple_names() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/git_multiple_names.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file(
                "p1/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package1", "0.3.0+my-package1"),
            )
            .file("p1/src/lib.rs", "")
            .file(
                "p2/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package2", "0.3.0+my-package2"),
            )
            .file("p2/src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2", "--git", &git_url])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/git_multiple_names.stdout")
        .stderr_matches_path("tests/snapshots/add/git_multiple_names.stderr");

    assert().subset_matches("tests/snapshots/add/git_multiple_names.out", &project_root);
}

#[cargo_test]
fn multiple_conflicts_with_rename() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/multiple_conflicts_with_rename.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2", "--rename", "renamed"])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path("tests/snapshots/add/multiple_conflicts_with_rename.stdout")
        .stderr_matches_path("tests/snapshots/add/multiple_conflicts_with_rename.stderr");

    assert().subset_matches(
        "tests/snapshots/add/multiple_conflicts_with_rename.out",
        &project_root,
    );
}

#[cargo_test]
fn namever() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/namever.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1@>=0.1.1", "my-package2@0.2.3", "my-package"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/namever.stdout")
        .stderr_matches_path("tests/snapshots/add/namever.stderr");

    assert().subset_matches("tests/snapshots/add/namever.out", &project_root);
}

#[cargo_test]
fn no_args() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/no_args.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .current_dir(cwd)
        .assert()
        .code(1)
        .stdout_matches_path("tests/snapshots/add/no_args.stdout")
        .stderr_matches_path("tests/snapshots/add/no_args.stderr");

    assert().subset_matches("tests/snapshots/add/no_args.out", &project_root);
}

#[cargo_test]
fn no_default_features() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/no_default_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--no-default-features"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/no_default_features.stdout")
        .stderr_matches_path("tests/snapshots/add/no_default_features.stderr");

    assert().subset_matches("tests/snapshots/add/no_default_features.out", &project_root);
}

#[cargo_test]
fn no_optional() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/no_optional.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--no-optional"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/no_optional.stdout")
        .stderr_matches_path("tests/snapshots/add/no_optional.stderr");

    assert().subset_matches("tests/snapshots/add/no_optional.out", &project_root);
}

#[cargo_test]
fn optional() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/optional.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--optional"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/optional.stdout")
        .stderr_matches_path("tests/snapshots/add/optional.stderr");

    assert().subset_matches("tests/snapshots/add/optional.out", &project_root);
}

#[cargo_test]
fn overwrite_default_features() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_default_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--default-features"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_default_features.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_default_features.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_default_features.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_default_features_with_no_default_features() {
    init_registry();
    let project_root = project_from_template(
        "tests/snapshots/add/overwrite_default_features_with_no_default_features.in",
    );
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--no-default-features"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path(
            "tests/snapshots/add/overwrite_default_features_with_no_default_features.stdout",
        )
        .stderr_matches_path(
            "tests/snapshots/add/overwrite_default_features_with_no_default_features.stderr",
        );

    assert().subset_matches(
        "tests/snapshots/add/overwrite_default_features_with_no_default_features.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_features() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--features", "nose"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_features.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_features.stderr");

    assert().subset_matches("tests/snapshots/add/overwrite_features.out", &project_root);
}

#[cargo_test]
fn overwrite_git_with_path() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_git_with_path.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "cargo-list-test-fixture-dependency",
            "--path",
            "../dependency",
        ])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_git_with_path.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_git_with_path.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_git_with_path.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_inline_features() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_inline_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args([
            "unrelateed-crate",
            "your-face",
            "--features",
            "your-face/nose,your-face/mouth",
            "-Fyour-face/ears",
        ])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_inline_features.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_inline_features.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_inline_features.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_name_dev_noop() {
    init_alt_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_name_dev_noop.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--dev"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_name_dev_noop.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_name_dev_noop.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_name_dev_noop.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_name_noop() {
    init_alt_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_name_noop.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_name_noop.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_name_noop.stderr");

    assert().subset_matches("tests/snapshots/add/overwrite_name_noop.out", &project_root);
}

#[cargo_test]
fn overwrite_no_default_features() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/overwrite_no_default_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--no-default-features"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_no_default_features.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_no_default_features.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_no_default_features.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_no_default_features_with_default_features() {
    init_registry();
    let project_root = project_from_template(
        "tests/snapshots/add/overwrite_no_default_features_with_default_features.in",
    );
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--default-features"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path(
            "tests/snapshots/add/overwrite_no_default_features_with_default_features.stdout",
        )
        .stderr_matches_path(
            "tests/snapshots/add/overwrite_no_default_features_with_default_features.stderr",
        );

    assert().subset_matches(
        "tests/snapshots/add/overwrite_no_default_features_with_default_features.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_no_optional() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_no_optional.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--no-optional"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_no_optional.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_no_optional.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_no_optional.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_no_optional_with_optional() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/overwrite_no_optional_with_optional.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--optional"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_no_optional_with_optional.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_no_optional_with_optional.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_no_optional_with_optional.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_optional() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_optional.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--optional"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_optional.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_optional.stderr");

    assert().subset_matches("tests/snapshots/add/overwrite_optional.out", &project_root);
}

#[cargo_test]
fn overwrite_optional_with_no_optional() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/overwrite_optional_with_no_optional.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2@0.4.1", "--no-optional"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_optional_with_no_optional.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_optional_with_no_optional.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_optional_with_no_optional.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_path_noop() {
    init_alt_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_path_noop.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["your-face", "--path", "./dependency"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_path_noop.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_path_noop.stderr");

    assert().subset_matches("tests/snapshots/add/overwrite_path_noop.out", &project_root);
}

#[cargo_test]
fn overwrite_path_with_version() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_path_with_version.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args(["cargo-list-test-fixture-dependency@20.0"])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_path_with_version.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_path_with_version.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_path_with_version.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_rename_with_no_rename() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/overwrite_rename_with_no_rename.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["versioned-package"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_rename_with_no_rename.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_rename_with_no_rename.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_rename_with_no_rename.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_rename_with_rename() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_rename_with_rename.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["versioned-package", "--rename", "a2"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_rename_with_rename.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_rename_with_rename.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_rename_with_rename.out",
        &project_root,
    );
}

#[cargo_test]
fn change_rename_target() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/change_rename_target.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package2", "--rename", "some-package"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/change_rename_target.stdout")
        .stderr_matches_path("tests/snapshots/add/change_rename_target.stderr");

    assert().subset_matches(
        "tests/snapshots/add/change_rename_target.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_rename_with_rename_noop() {
    init_registry();
    let project_root =
        project_from_template("tests/snapshots/add/overwrite_rename_with_rename_noop.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["versioned-package", "--rename", "a1"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_rename_with_rename_noop.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_rename_with_rename_noop.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_rename_with_rename_noop.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_version_with_git() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_version_with_git.in");
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("versioned-package", |project| {
        project
            .file(
                "Cargo.toml",
                &cargo_test_support::basic_manifest("versioned-package", "0.3.0+versioned-package"),
            )
            .file("src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    cargo_command()
        .arg("add")
        .args(["versioned-package", "--git", &git_url])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_version_with_git.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_version_with_git.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_version_with_git.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_version_with_path() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_version_with_path.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "cargo-list-test-fixture-dependency",
            "--path",
            "../dependency",
        ])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_version_with_path.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_version_with_path.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_version_with_path.out",
        &project_root,
    );
}

#[cargo_test]
fn overwrite_with_rename() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/overwrite_with_rename.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["versioned-package", "--rename", "renamed"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/overwrite_with_rename.stdout")
        .stderr_matches_path("tests/snapshots/add/overwrite_with_rename.stderr");

    assert().subset_matches(
        "tests/snapshots/add/overwrite_with_rename.out",
        &project_root,
    );
}

#[cargo_test]
fn preserve_sorted() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/preserve_sorted.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["toml"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/preserve_sorted.stdout")
        .stderr_matches_path("tests/snapshots/add/preserve_sorted.stderr");

    assert().subset_matches("tests/snapshots/add/preserve_sorted.out", &project_root);
}

#[cargo_test]
fn preserve_unsorted() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/preserve_unsorted.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["toml"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/preserve_unsorted.stdout")
        .stderr_matches_path("tests/snapshots/add/preserve_unsorted.stderr");

    assert().subset_matches("tests/snapshots/add/preserve_unsorted.out", &project_root);
}

#[cargo_test]
fn registry() {
    init_alt_registry();
    let project_root = project_from_template("tests/snapshots/add/registry.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2", "--registry", "alternative"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/registry.stdout")
        .stderr_matches_path("tests/snapshots/add/registry.stderr");

    assert().subset_matches("tests/snapshots/add/registry.out", &project_root);
}

#[cargo_test]
fn rename() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/rename.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package", "--rename", "renamed"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/rename.stdout")
        .stderr_matches_path("tests/snapshots/add/rename.stderr");

    assert().subset_matches("tests/snapshots/add/rename.out", &project_root);
}

#[cargo_test]
fn target() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/target.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args([
            "my-package1",
            "my-package2",
            "--target",
            "i686-unknown-linux-gnu",
        ])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/target.stdout")
        .stderr_matches_path("tests/snapshots/add/target.stderr");

    assert().subset_matches("tests/snapshots/add/target.out", &project_root);
}

#[cargo_test]
fn target_cfg() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/target_cfg.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package1", "my-package2", "--target", "cfg(unix)"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/target_cfg.stdout")
        .stderr_matches_path("tests/snapshots/add/target_cfg.stderr");

    assert().subset_matches("tests/snapshots/add/target_cfg.out", &project_root);
}

#[cargo_test]
fn vers() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/vers.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package@>=0.1.1"])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/vers.stdout")
        .stderr_matches_path("tests/snapshots/add/vers.stderr");

    assert().subset_matches("tests/snapshots/add/vers.out", &project_root);
}

#[cargo_test]
fn workspace_path() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/workspace_path.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "cargo-list-test-fixture-dependency",
            "--path",
            "../dependency",
        ])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/workspace_path.stdout")
        .stderr_matches_path("tests/snapshots/add/workspace_path.stderr");

    assert().subset_matches("tests/snapshots/add/workspace_path.out", &project_root);
}

#[cargo_test]
fn workspace_path_dev() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/workspace_path_dev.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args([
            "cargo-list-test-fixture-dependency",
            "--path",
            "../dependency",
            "--dev",
        ])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/workspace_path_dev.stdout")
        .stderr_matches_path("tests/snapshots/add/workspace_path_dev.stderr");

    assert().subset_matches("tests/snapshots/add/workspace_path_dev.out", &project_root);
}

#[cargo_test]
fn workspace_name() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/workspace_name.in");
    let cwd = project_root.join("primary");

    cargo_command()
        .arg("add")
        .args(["cargo-list-test-fixture-dependency"])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/snapshots/add/workspace_name.stdout")
        .stderr_matches_path("tests/snapshots/add/workspace_name.stderr");

    assert().subset_matches("tests/snapshots/add/workspace_name.out", &project_root);
}

#[cargo_test]
fn deprecated_default_features() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/deprecated_default_features.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package"])
        .current_dir(&cwd)
        .assert()
        .failure()
        .stdout_matches_path("tests/snapshots/add/deprecated_default_features.stdout")
        .stderr_matches_path("tests/snapshots/add/deprecated_default_features.stderr");

    assert().subset_matches(
        "tests/snapshots/add/deprecated_default_features.out",
        &project_root,
    );
}

#[cargo_test]
fn deprecated_section() {
    init_registry();
    let project_root = project_from_template("tests/snapshots/add/deprecated_section.in");
    let cwd = &project_root;

    cargo_command()
        .arg("add")
        .args(["my-package"])
        .current_dir(&cwd)
        .assert()
        .failure()
        .stdout_matches_path("tests/snapshots/add/deprecated_section.stdout")
        .stderr_matches_path("tests/snapshots/add/deprecated_section.stderr");

    assert().subset_matches("tests/snapshots/add/deprecated_section.out", &project_root);
}
