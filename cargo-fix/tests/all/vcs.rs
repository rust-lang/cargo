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
        .stderr("no VCS found, aborting. overwrite this behavior with --allow-no-vcs")
        .status(1)
        .run();
    p.expect_cmd("cargo-fix fix --allow-no-vcs").status(0).run();
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
            "?? Cargo.toml\n\
             ?? src/\n\
             working directory is dirty, aborting. overwrite this behavior with --allow-dirty",
        )
        .status(1)
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
    p.expect_cmd("git config user.email rustfix@rustlang.org").run();
    p.expect_cmd("git config user.name RustFix").run();
    p.expect_cmd("git commit -m Initial-commit").run();
    p.expect_cmd("cargo-fix fix").status(0).run();
}
