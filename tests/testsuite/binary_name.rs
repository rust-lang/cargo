//! Tests for `cargo-features = ["different-binary-name"]`.

use crate::prelude::*;
use cargo_test_support::install::assert_has_installed_exe;
use cargo_test_support::install::assert_has_not_installed_exe;
use cargo_test_support::is_nightly;
use cargo_test_support::paths;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name =  "foo"
                version = "0.0.1"
                edition = "2015"

                [[bin]]
                name = "foo"
                filename = "007bar"
                path = "src/main.rs"
            "#,
        )
        .file("src/main.rs", "fn main() { assert!(true) }")
        .build();

    // Run cargo build.
    p.cargo("build")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `different-binary-name` is required

  The package requires the Cargo feature called `different-binary-name`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["different-binary-name"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#different-binary-name for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
// This test checks if:
// 1. The correct binary is produced
// 2. The deps file has the correct content
// 3. Fingerprinting works
// 4. `cargo clean` command works
fn binary_name1() {
    // Create the project.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["different-binary-name"]

                [package]
                name =  "foo"
                version = "0.0.1"
                edition = "2015"

                [[bin]]
                name = "foo"
                filename = "007bar"
                path = "src/main.rs"
            "#,
        )
        .file("src/main.rs", "fn main() { assert!(true) }")
        .build();

    // Run cargo build.
    p.cargo("build")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .run();

    // Check the name of the binary that cargo has generated.
    // A binary with the name of the crate should NOT be created.
    let foo_path = p.bin("foo");
    assert!(!foo_path.is_file());
    // A binary with the name provided in `filename` parameter should be created.
    let bar_path = p.bin("007bar");
    assert!(bar_path.is_file());

    // Check if deps file exists.
    let deps_path = p.bin("007bar").with_extension("d");
    assert!(deps_path.is_file(), "{:?}", bar_path);

    let depinfo = p.read_file(&deps_path);

    // Prepare what content we expect to be present in deps file.
    let deps_exp = format!(
        "{}: {}",
        p.bin("007bar").to_str().unwrap(),
        p.root().join("src").join("main.rs").to_str().unwrap()
    );

    // Compare actual deps content with expected deps content.
    assert!(
        depinfo.lines().any(|line| line == deps_exp),
        "Content of `{}` is incorrect",
        deps_path.to_string_lossy()
    );

    // Run cargo second time, to verify fingerprint.
    p.cargo("build -p foo -v")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Run cargo clean.
    p.cargo("clean -p foo")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .run();

    // Check if the appropriate file was removed.
    assert!(
        !bar_path.is_file(),
        "`cargo clean` did not remove the correct files"
    );
}

#[cargo_test]
// This test checks if:
// 1. Check `cargo run`
// 2. Check `cargo test`
// 3. Check `cargo install/uninstall`
fn binary_name2() {
    // Create the project.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["different-binary-name"]

                [package]
                name =  "foo"
                version = "0.0.1"
                edition = "2015"

                [[bin]]
                name = "foo"
                filename = "007bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn hello(name: &str) -> String {
                    format!("Hello, {}!", name)
                }

                fn main() {
                    println!("{}", hello("crabs"));
                }

                #[cfg(test)]
                mod tests {
                    use super::*;

                    #[test]
                    fn check_crabs() {
                        assert_eq!(hello("crabs"), "Hello, crabs!");
                    }
                }
            "#,
        )
        .build();

    // Run cargo build.
    p.cargo("build")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .run();

    // Check the name of the binary that cargo has generated.
    // A binary with the name of the crate should NOT be created.
    let foo_path = p.bin("foo");
    assert!(!foo_path.is_file());
    // A binary with the name provided in `filename` parameter should be created.
    let bar_path = p.bin("007bar");
    assert!(bar_path.is_file());

    // Check if `cargo test` works
    p.cargo("test")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/foo-[..][EXE])

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test tests::check_crabs ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();

    // Check if `cargo run` is able to execute the binary
    p.cargo("run")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .with_stdout_data(str![[r#"
Hello, crabs!

"#]])
        .run();

    p.cargo("install")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .run();

    assert_has_installed_exe(paths::cargo_home(), "007bar");

    p.cargo("uninstall")
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/bin/007bar[EXE]

"#]])
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .run();

    assert_has_not_installed_exe(paths::cargo_home(), "007bar");
}

#[cargo_test]
fn check_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["different-binary-name"]

                [package]
                name =  "foo"
                version = "0.0.1"
                edition = "2015"

                [[bin]]
                name = "foo"
                filename = "007bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    println!("{}", option_env!("CARGO_BIN_NAME").unwrap());
                }
            "#,
        )
        .file(
            "tests/integration.rs",
            r#"
                #[test]
                fn check_env_vars2() {
                    let value = option_env!("CARGO_BIN_EXE_007bar").expect("Could not find environment variable.");
                    assert!(value.contains("007bar"));
                }
            "#
        )
        .build();

    // Run cargo build.
    p.cargo("build")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .run();
    p.cargo("run")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .with_stdout_data(str![[r#"
007bar

"#]])
        .run();
    p.cargo("test")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .with_status(0)
        .run();
}

#[cargo_test]
fn check_msg_format_json() {
    // Create the project.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["different-binary-name"]

                [package]
                name =  "foo"
                version = "0.0.1"
                edition = "2015"

                [[bin]]
                name = "foo"
                filename = "007bar"
                path = "src/main.rs"
            "#,
        )
        .file("src/main.rs", "fn main() { assert!(true) }")
        .build();

    // Run cargo build.
    p.cargo("build --message-format=json")
        .masquerade_as_nightly_cargo(&["different-binary-name"])
        .with_stdout_data(
            str![[r#"
[
  {
    "executable": "[ROOT]/foo/target/debug/007bar[EXE]",
    "features": [],
    "filenames": "{...}",
    "fresh": false,
    "manifest_path": "[ROOT]/foo/Cargo.toml",
    "package_id": "path+[ROOTURL]/foo#0.0.1",
    "profile": "{...}",
    "reason": "compiler-artifact",
    "target": "{...}"
  },
  {
    "reason": "build-finished",
    "success": true
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .run();
}

#[cargo_test]
fn targets_with_relative_path_in_workspace_members() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["relative-bar"]
            resolver = "2"
        "#,
        )
        .file(
            "relative-bar/Cargo.toml",
            r#"
                [package]
                name = "relative-bar"
                version = "0.1.0"
                edition = "2021"

                build = "./build.rs"

                [[bin]]
                name = "bar"
                path = "./src/main.rs"

                [lib]
                name = "lib"
                path = "./src/lib.rs"

                [[example]]
                name = "example"
                path = "./example.rs"

                [[test]]
                name = "test"
                path = "./test.rs"

                [[bench]]
                name = "bench"
                path = "./bench.rs"
            "#,
        )
        .file("relative-bar/build.rs", "fn main() { let a = 1; }")
        .file("relative-bar/src/main.rs", "fn main() { let a = 1; }")
        .file("relative-bar/src/lib.rs", "fn a() {}")
        .file("relative-bar/example.rs", "fn main() { let a = 1; }")
        .file(
            "relative-bar/test.rs",
            r#"
                fn main() {}

                #[test]
                fn test_a() { let a = 1; } 
            "#,
        )
        .file(
            "relative-bar/bench.rs",
            r#"  
                #![feature(test)]
                #[cfg(test)]
                extern crate test;

                #[bench]
                fn bench_a(_b: &mut test::Bencher) { let a = 1; }
            "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
...
 --> relative-bar/build.rs:1:17
...
 --> relative-bar/src/lib.rs:1:4
...
 --> relative-bar/src/main.rs:1:17
...
"#]])
        .run();

    p.cargo("check --example example")
        .with_stderr_data(str![[r#"
...
 --> relative-bar/example.rs:1:17
...
"#]])
        .run();

    p.cargo("check --test test")
        .with_stderr_data(str![[r#"
...
 --> relative-bar/test.rs:5:35
...
"#]])
        .run();

    if is_nightly() {
        p.cargo("check --bench bench")
            .with_stderr_data(str![[r#"
...
 --> relative-bar/bench.rs:7:58
...
"#]])
            .run();
    }

    // Disable Cargo target auto-discovery.
    p.change_file(
        "relative-bar/Cargo.toml",
        r#"
            [package]
            name = "relative-bar"
            version = "0.1.0"
            edition = "2021"

            autolib = false
            autobins = false
            autoexamples = false
            autotests = false
            autobenches = false

            build = "./build.rs"

            [[bin]]
            name = "bar"
            path = "./src/main.rs"

            [lib]
            name = "lib"
            path = "./src/lib.rs"

            [[example]]
            name = "example"
            path = "./example.rs"

            [[test]]
            name = "test"
            path = "./test.rs"

            [[bench]]
            name = "bench"
            path = "./bench.rs"
        "#,
    );

    p.cargo("check")
        .with_stderr_data(str![[r#"
...
 --> relative-bar/build.rs:1:17
...
 --> relative-bar/src/lib.rs:1:4
...
 --> relative-bar/src/main.rs:1:17
...
"#]])
        .run();

    p.cargo("check --example example")
        .with_stderr_data(str![[r#"
...
 --> relative-bar/example.rs:1:17
...
"#]])
        .run();

    p.cargo("check --test test")
        .with_stderr_data(str![[r#"
...
 --> relative-bar/test.rs:5:35
...
"#]])
        .run();

    if is_nightly() {
        p.cargo("check --bench bench")
            .with_stderr_data(str![[r#"
...
 --> relative-bar/bench.rs:7:58
...
"#]])
            .run();
    }
}
