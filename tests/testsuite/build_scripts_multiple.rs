//! Tests for multiple build scripts feature.

use cargo_test_support::git;
use cargo_test_support::prelude::*;
use cargo_test_support::str;
use cargo_test_support::{project, Project};

#[cargo_test]
fn build_without_feature_enabled_aborts_with_error() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                build = ["build1.rs", "build2.rs"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build1.rs", "fn main() {}")
        .file("build2.rs", "fn main() {}")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: sequence, expected a boolean or string
 --> Cargo.toml:6:25
  |
6 |                 build = ["build1.rs", "build2.rs"]
  |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^
  |

"#]])
        .run();
}

fn basic_empty_project() -> Project {
    project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["multiple-build-scripts"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                build = ["build1.rs", "build2.rs"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build1.rs", "fn main() {}")
        .file("build2.rs", "fn main() {}")
        .build()
}

#[cargo_test]
fn empty_multiple_build_script_project() {
    let p = basic_empty_project();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: sequence, expected a boolean or string
 --> Cargo.toml:8:25
  |
8 |                 build = ["build1.rs", "build2.rs"]
  |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^
  |

"#]])
        .run();
}

#[cargo_test]
fn multiple_build_scripts_metadata() {
    let p = basic_empty_project();
    p.cargo("metadata --format-version=1")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: sequence, expected a boolean or string
 --> Cargo.toml:8:25
  |
8 |                 build = ["build1.rs", "build2.rs"]
  |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^
  |

"#]])
        .run();
}

#[cargo_test]
fn verify_package_multiple_build_scripts() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["multiple-build-scripts"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                license = "MIT"
                description = "foo"
                documentation = "docs.rs/foo"
                authors = []

                build = ["build1.rs", "build2.rs"]
                include = [ "src/main.rs", "build1.rs" ]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build1.rs", "fn main() {}")
        .file("build2.rs", "fn main() {}")
        .build();

    p.cargo("package")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: sequence, expected a boolean or string
  --> Cargo.toml:13:25
   |
13 |                 build = ["build1.rs", "build2.rs"]
   |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^
   |

"#]])
        .run();
}

#[cargo_test]
fn verify_vendor_multiple_build_scripts() {
    let git_project = git::new("dep", |project| {
        project
            .file(
                "Cargo.toml",
                r#"
                cargo-features = ["multiple-build-scripts"]

                [package]
                name = "dep"
                version = "0.1.0"
                edition = "2024"
                license = "MIT"
                description = "dependency of foo"
                documentation = "docs.rs/dep"
                authors = []

                build = ["build1.rs", "build2.rs"]
                include = [ "src/main.rs", "build1.rs" ]
            "#,
            )
            .file("src/main.rs", "fn main() {}")
            .file("build1.rs", "fn main() {}")
            .file("build2.rs", "fn main() {}")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    cargo-features = ["multiple-build-scripts"]

                    [package]
                    name = "foo"
                    version = "0.1.0"
                    edition = "2024"

                    [dependencies.dep]
                    git = '{}'
                "#,
                git_project.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("vendor --respect-source-config")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep`
[ERROR] invalid type: sequence, expected a boolean or string
  --> ../home/.cargo/git/checkouts/dep-[HASH]/[..]/Cargo.toml:13:25
   |
13 |                 build = ["build1.rs", "build2.rs"]
   |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
[ERROR] failed to sync

Caused by:
  failed to load lockfile for [ROOT]/foo

Caused by:
  failed to get `dep` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep`

Caused by:
  Unable to update [ROOTURL]/dep

"#]])
        .run();
}

#[cargo_test]
fn rerun_untracks_other_files() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
fn main() {
    foo();
    bar();
}
fn foo() {
    let _path = "assets/foo.txt";
}
fn bar() {
    let path = "assets/bar.txt";
    println!("cargo::rerun-if-changed={path}");
}"#,
        )
        .file("assets/foo.txt", "foo")
        .file("assets/bar.txt", "bar")
        .build();
    p.cargo("build").run();

    // Editing foo.txt won't recompile, leading to unnoticed changes

    p.change_file("assets/foo.txt", "foo updated");
    p.cargo("build -v")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Editing bar.txt will recompile

    p.change_file("assets/bar.txt", "bar updated");
    p.cargo("build -v")      
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.1.0 ([ROOT]/foo): the file `assets/bar.txt` has changed ([TIME_DIFF_AFTER_LAST_BUILD])
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name foo --edition=2024 src/main.rs [..] --crate-type bin [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
