//! Tests for -Zextra-link-arg.

use cargo_test_support::{basic_bin_manifest, basic_lib_manifest, project};

#[cargo_test]
fn build_script_extra_link_arg_bins() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-arg-bins=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -Zextra-link-arg -v")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo [..]-C link-arg=--this-is-a-bogus-flag[..]",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_tests() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file("tests/test_foo.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-arg-tests=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("test -Zextra-link-arg -v")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name test_foo [..]-C link-arg=--this-is-a-bogus-flag[..]",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_benches() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file("benches/bench_foo.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-arg-benches=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("bench -Zextra-link-arg -v")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bench_foo [..]-C link-arg=--this-is-a-bogus-flag[..]",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_examples() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file("examples/example_foo.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-arg-examples=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -Zextra-link-arg -v --examples")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name example_foo [..]-C link-arg=--this-is-a-bogus-flag[..]",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-arg=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -Zextra-link-arg -v")
        .masquerade_as_nightly_cargo()
        .without_status()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo [..]-C link-arg=--this-is-a-bogus-flag[..]",
        )
        .run();
}

#[cargo_test]
fn build_script_extra_link_arg_warn_without_flag() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-link-arg=--this-is-a-bogus-flag");
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .with_status(0)
        .with_stderr_contains("warning: cargo:rustc-link-arg requires -Zextra-link-arg flag")
        .run();
}
