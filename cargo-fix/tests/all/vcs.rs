use super::project;

#[test]
fn warns_if_no_vcs_detected() {
    let p = project()
        .use_temp_dir()
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {
            }
        "#,
        )
        .build();

    p.expect_cmd("cargo-fix fix")
        .stderr(
            "warning: Could not detect a version control system\n\
             You should consider using a VCS so you can easily see and revert rustfix' changes.\n\
             error: No VCS found, aborting. Overwrite this behavior with `--allow-no-vcs`.\n\
             ",
        )
        .run();
}

#[test]
fn warns_about_dirty_working_directory() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {
            }
        "#,
        )
        .build();

    p.expect_cmd("git init").run();
    p.expect_cmd("cargo-fix fix")
        .stderr(
            "warning: Working directory dirty\n\
            Make sure your working directory is clean so you can easily revert rustfix' changes.\n\
            ?? Cargo.toml\n\
            ?? src/\n\
            error: Aborting because of dirty working directory. Overwrite this behavior with `--allow-dirty`.\n\n",
        )
        .run();
    p.expect_cmd("cargo-fix fix --allow-dirty").status(0).run();
}

#[test]
fn does_not_warn_about_clean_working_directory() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {
            }
        "#,
        )
        .build();

    p.expect_cmd("git init").run();
    p.expect_cmd("git add .").run();
    p.expect_cmd("git config user.email rustfix@rustlang.org")
        .run();
    p.expect_cmd("git config user.name RustFix").run();
    p.expect_cmd("git commit -m Initial-commit").run();
    p.expect_cmd("cargo-fix fix").status(0).run();
}
