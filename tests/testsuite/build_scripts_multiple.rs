//! Tests for multiple build scripts feature.

use crate::prelude::*;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::git;
use cargo_test_support::publish::validate_crate_contents;
use cargo_test_support::str;
use cargo_test_support::{Project, project};
use std::fs::File;

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
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `multiple-build-scripts` is required

  The package requires the Cargo feature called `multiple-build-scripts`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["multiple-build-scripts"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#multiple-build-scripts for more information about the status of this feature.

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
        .with_status(0)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn multiple_build_scripts_metadata() {
    let p = basic_empty_project();
    p.cargo("metadata --format-version=1")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(0)
        .with_stderr_data("")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2024",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.1.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2024",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/main.rs",
          "test": true
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": false,
          "doctest": false,
          "edition": "2024",
          "kind": [
            "custom-build"
          ],
          "name": "build-script-build1",
          "src_path": "[ROOT]/foo/build1.rs",
          "test": false
        },
        {
          "crate_types": [
            "bin"
          ],
          "doc": false,
          "doctest": false,
          "edition": "2024",
          "kind": [
            "custom-build"
          ],
          "name": "build-script-build2",
          "src_path": "[ROOT]/foo/build2.rs",
          "test": false
        }
      ],
      "version": "0.1.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.1.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.1.0"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.1.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
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
        .with_status(0)
        .with_stderr_data(str![[r#"
[PACKAGING] foo v0.1.0 ([ROOT]/foo)
[WARNING] ignoring `package.build` entry `build2.rs` as it is not included in the published package
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.1.0 ([ROOT]/foo)
[COMPILING] foo v0.1.0 ([ROOT]/foo/target/package/foo-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let f = File::open(&p.root().join("target/package/foo-0.1.0.crate")).unwrap();
    validate_crate_contents(
        f,
        "foo-0.1.0.crate",
        &[
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "build1.rs",
            "Cargo.lock",
        ],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

cargo-features = ["multiple-build-scripts"]

[package]
edition = "2024"
name = "foo"
version = "0.1.0"
authors = []
build = "build1.rs"
include = [
    "src/main.rs",
    "build1.rs",
]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
documentation = "docs.rs/foo"
readme = false
license = "MIT"

[[bin]]
name = "foo"
path = "src/main.rs"

"##]],
        )],
    );
}

fn add_git_vendor_config(p: &Project, git_project: &Project) {
    p.change_file(
        ".cargo/config.toml",
        &format!(
            r#"
            [source."git+{url}"]
            git = "{url}"
            replace-with = 'vendor'

            [source.vendor]
            directory = 'vendor'
        "#,
            url = git_project.url()
        ),
    );
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
        .with_status(0)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep`
[LOCKING] 1 package to latest [..] compatible version
   Vendoring dep v0.1.0 ([ROOTURL]/dep#[..]) ([ROOT]/home/.cargo/git/checkouts/dep-[HASH]/[..]) to vendor/dep
[WARNING] ignoring `package.build` entry `build2.rs` as it is not included in the published package
To use vendored sources, add this to your .cargo/config.toml for this project:


"#]])
        .run();
    add_git_vendor_config(&p, &git_project);

    assert_e2e().eq(
        p.read_file("vendor/dep/Cargo.toml"),
        str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

cargo-features = ["multiple-build-scripts"]

[package]
edition = "2024"
name = "dep"
version = "0.1.0"
authors = []
build = "build1.rs"
include = [
    "src/main.rs",
    "build1.rs",
]
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "dependency of foo"
documentation = "docs.rs/dep"
readme = false
license = "MIT"

[[bin]]
name = "dep"
path = "src/main.rs"

"##]],
    );

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .run();
}

#[cargo_test]
fn custom_build_script_first_index_script_failed() {
    // In this, the script that is at first index in the build script array fails
    let p = project()
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
        .file("build1.rs", "fn main() { std::process::exit(101); }")
        .file("build2.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
...
[ERROR] failed to run custom build command for `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  process didn't exit successfully: `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build1` ([EXIT_STATUS]: 101)
...
"#]])
        .run();
}

#[cargo_test]
fn custom_build_script_second_index_script_failed() {
    // In this, the script that is at second index in the build script array fails
    // This test was necessary because earlier, the program failed only if first script failed.
    let p = project()
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
        .file("build2.rs", "fn main() { std::process::exit(101); }")
        .build();

    p.cargo("check -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
...
[ERROR] failed to run custom build command for `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  process didn't exit successfully: `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build2` ([EXIT_STATUS]: 101)
...
"#]])
        .run();
}

#[cargo_test]
fn build_script_with_conflicting_environment_variables() {
    // In this, multiple scripts set different values to same environment variables
    let p = project()
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
        .file(
            "src/main.rs",
            r#"
                const FOO: &'static str = env!("FOO");
                fn main() {
                    println!("{}", FOO);
                }
            "#,
        )
        .file(
            "build1.rs",
            r#"fn main() { println!("cargo::rustc-env=FOO=bar1"); }"#,
        )
        .file(
            "build2.rs",
            r#"fn main() { println!("cargo::rustc-env=FOO=bar2"); }"#,
        )
        .build();

    p.cargo("run -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(0)
        .with_stdout_data(str![[r#"
bar2

"#]])
        .run();
}

#[cargo_test]
fn build_script_with_conflicting_out_dirs() {
    // In this, multiple scripts create file with same name in their respective OUT_DIR.

    let p = project()
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
        // By default, OUT_DIR is set to that of the first build script in the array
        .file(
            "src/main.rs",
            r#"
                include!(concat!(env!("OUT_DIR"), "/foo.rs"));
                fn main() {
                    println!("{}", message());
                }
            "#,
        )
        .file(
            "build1.rs",
            r#"
            use std::env;
            use std::fs;
            use std::path::Path;

            fn main() {
                let out_dir = env::var_os("OUT_DIR").unwrap();
                let dest_path = Path::new(&out_dir).join("foo.rs");
                fs::write(
                    &dest_path,
                    "pub fn message() -> &'static str {
                        \"Hello, from Build Script 1!\"
                    }
                    "
                ).unwrap();
             }"#,
        )
        .file(
            "build2.rs",
            r#"
            use std::env;
            use std::fs;
            use std::path::Path;

            fn main() {
                let out_dir = env::var_os("OUT_DIR").unwrap();
                let dest_path = Path::new(&out_dir).join("foo.rs");
                fs::write(
                    &dest_path,
                    "pub fn message() -> &'static str {
                        \"Hello, from Build Script 2!\"
                    }
                    "
                ).unwrap();
             }"#,
        )
        .build();

    p.cargo("run -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(0)
        .with_stdout_data(str![[r#"
Hello, from Build Script 1!

"#]])
        .run();
}

#[cargo_test]
fn build_script_with_conflicts_reverse_sorted() {
    // In this, multiple scripts create file with same name in their respective OUT_DIR.
    // It is different from above because `package.build` is not sorted in this.

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["multiple-build-scripts"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"

                build = ["build2.rs", "build1.rs"]
            "#,
        )
        // By default, OUT_DIR is set to that of the first build script in the array
        .file(
            "src/main.rs",
            r#"
                include!(concat!(env!("OUT_DIR"), "/foo.rs"));
                fn main() {
                    println!("{}", message());
                }
            "#,
        )
        .file(
            "build1.rs",
            r#"
            use std::env;
            use std::fs;
            use std::path::Path;

            fn main() {
                let out_dir = env::var_os("OUT_DIR").unwrap();
                let dest_path = Path::new(&out_dir).join("foo.rs");
                fs::write(
                    &dest_path,
                    "pub fn message() -> &'static str {
                        \"Hello, from Build Script 1!\"
                    }
                    "
                ).unwrap();
             }"#,
        )
        .file(
            "build2.rs",
            r#"
            use std::env;
            use std::fs;
            use std::path::Path;

            fn main() {
                let out_dir = env::var_os("OUT_DIR").unwrap();
                let dest_path = Path::new(&out_dir).join("foo.rs");
                fs::write(
                    &dest_path,
                    "pub fn message() -> &'static str {
                        \"Hello, from Build Script 2!\"
                    }
                    "
                ).unwrap();
             }"#,
        )
        .build();

    p.cargo("run -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(0)
        .with_stdout_data(str![[r#"
Hello, from Build Script 2!

"#]])
        .run();
}

#[cargo_test]
fn rerun_untracks_other_files() {
    let p = project()
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
        .file(
            "build1.rs",
            r#"
fn main() {
    foo();
}
fn foo() {
    let _path = "assets/foo.txt";
}
"#,
        )
        .file(
            "build2.rs",
            r#"
fn main() {
    bar();
}

fn bar() {
    let path = "assets/bar.txt";
    println!("cargo::rerun-if-changed={path}");
}"#,
        )
        .file("assets/foo.txt", "foo")
        .file("assets/bar.txt", "bar")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .run();

    // Editing foo.txt will also recompile now since they are separate build scripts
    p.change_file("assets/foo.txt", "foo updated");
    p.cargo("check -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.1.0 ([ROOT]/foo): the [..]
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build1`
[RUNNING] `rustc --crate-name foo --edition=2024 src/main.rs [..] --crate-type bin [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Editing bar.txt will also recompile now since they are separate build scripts

    p.change_file("assets/bar.txt", "bar updated");
    p.cargo("check -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_stderr_data(str![[r#"
[DIRTY] foo v0.1.0 ([ROOT]/foo): the [..]
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build[..]`
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build[..]`
[RUNNING] `rustc --crate-name foo --edition=2024 src/main.rs [..] --crate-type bin [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn multiple_out_dirs() {
    // Test to verify access to the `OUT_DIR` of the respective build scripts.

    let p = project()
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
        .file(
            "src/main.rs",
            r#"
                include!(concat!(env!("build1_OUT_DIR"), "/foo.rs"));
                include!(concat!(env!("build2_OUT_DIR"), "/foo.rs"));
                fn main() {
                    println!("{}", message1());
                    println!("{}", message2());
                }
            "#,
        )
        .file(
            "build1.rs",
            r#"
            use std::env;
            use std::fs;
            use std::path::Path;

            fn main() {
                let out_dir = env::var_os("OUT_DIR").unwrap();
                let dest_path = Path::new(&out_dir).join("foo.rs");
                fs::write(
                    &dest_path,
                    "pub fn message1() -> &'static str {
                        \"Hello, from Build Script 1!\"
                    }
                    "
                ).unwrap();
             }"#,
        )
        .file(
            "build2.rs",
            r#"
            use std::env;
            use std::fs;
            use std::path::Path;

            fn main() {
                let out_dir = env::var_os("OUT_DIR").unwrap();
                let dest_path = Path::new(&out_dir).join("foo.rs");
                fs::write(
                    &dest_path,
                    "pub fn message2() -> &'static str {
                        \"Hello, from Build Script 2!\"
                    }
                    "
                ).unwrap();
             }"#,
        )
        .build();

    p.cargo("run -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(0)
        .with_stdout_data(str![[r#"
Hello, from Build Script 1!
Hello, from Build Script 2!

"#]])
        .run();
}

#[cargo_test]
fn duplicate_build_script_stems() {
    // Test to verify that duplicate build script file stems throws error.

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["multiple-build-scripts"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                build = ["build1.rs", "foo/build1.rs"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build1.rs", "fn main() {}")
        .file("foo/build1.rs", "fn main() {}")
        .build();

    p.cargo("check -v")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  found build scripts with duplicate file stems, but all build scripts must have a unique file stem
    for stem `build1`: build1.rs, foo/build1.rs

"#]])
        .run();
}
