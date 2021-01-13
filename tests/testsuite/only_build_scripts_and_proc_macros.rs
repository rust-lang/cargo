//! Tests for --only-build-scripts-and-proc-macros feature.

use cargo_test_support::{basic_bin_manifest, project};

#[cargo_test]
fn simple_build_script() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo:rustc-cfg=my_feature"); }"#,
        )
        .build();

    p.cargo("check -Zunstable-options --only-build-scripts-and-proc-macros --message-format json")
        .masquerade_as_nightly_cargo()
        .with_stdout_contains("[..]my_feature[..]")
        .run();
}

#[cargo_test]
fn main_code_not_checked() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "some invalid code")
        .build();

    p.cargo("check -Zunstable-options --only-build-scripts-and-proc-macros")
        .masquerade_as_nightly_cargo()
        .run();
    assert!(!p.bin("foo").is_file());
}
