//! Tests for multiple `--target` flags to subcommands

use crate::prelude::*;
use crate::utils::cross_compile::{
    can_run_on_host as cross_compile_can_run_on_host, disabled as cross_compile_disabled,
};
use cargo_test_support::{basic_manifest, cross_compile, project, rustc_host, str};

#[cargo_test]
fn simple_build() {
    if cross_compile_disabled() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .run();

    assert!(p.target_bin(t1, "foo").is_file());
    assert!(p.target_bin(t2, "foo").is_file());
}

#[cargo_test]
fn simple_build_with_config() {
    if cross_compile_disabled() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [build]
                    target = ["{t1}", "{t2}"]
                "#
            ),
        )
        .build();

    p.cargo("build").run();

    assert!(p.target_bin(t1, "foo").is_file());
    assert!(p.target_bin(t2, "foo").is_file());
}

#[cargo_test]
fn simple_test() {
    if !cross_compile_can_run_on_host() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "fn main() {}")
        .build();

    p.cargo("test")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .with_stderr_data(
            str![[r#"
[RUNNING] unittests src/lib.rs (target/[ALT_TARGET]/debug/deps/foo-[HASH][EXE])
[RUNNING] unittests src/lib.rs (target/[HOST_TARGET]/debug/deps/foo-[HASH][EXE])
...

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn simple_run() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("run --target a --target b")
        .with_stderr_data(str![[r#"
[ERROR] only one `--target` argument is supported

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn simple_doc() {
    if cross_compile_disabled() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "//! empty lib")
        .build();

    p.cargo("doc")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .run();

    assert!(p.build_dir().join(&t1).join("doc/foo/index.html").is_file());
    assert!(p.build_dir().join(&t2).join("doc/foo/index.html").is_file());
}

#[cargo_test]
fn simple_doc_open() {
    if cross_compile_disabled() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "//! empty lib")
        .build();

    p.cargo("doc")
        .arg("--open")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v1.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[ERROR] only one `--target` argument is supported

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn simple_check() {
    if cross_compile_disabled() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .run();
}

#[cargo_test]
fn same_value_twice() {
    if cross_compile_disabled() {
        return;
    }
    let t = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .arg("--target")
        .arg(&t)
        .arg("--target")
        .arg(&t)
        .run();

    assert!(p.target_bin(t, "foo").is_file());
}

#[cargo_test]
fn same_value_twice_with_config() {
    if cross_compile_disabled() {
        return;
    }
    let t = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [build]
                    target = ["{t}", "{t}"]
                "#
            ),
        )
        .build();

    p.cargo("build").run();

    assert!(p.target_bin(t, "foo").is_file());
}

#[cargo_test]
fn works_with_config_in_both_string_or_list() {
    if cross_compile_disabled() {
        return;
    }
    let t = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [build]
                    target = "{t}"
                "#
            ),
        )
        .build();

    p.cargo("build").run();

    assert!(p.target_bin(t, "foo").is_file());

    p.cargo("clean").run();

    p.change_file(
        ".cargo/config.toml",
        &format!(
            r#"
                [build]
                target = ["{t}"]
            "#
        ),
    );

    p.cargo("build").run();

    assert!(p.target_bin(t, "foo").is_file());
}

#[cargo_test]
fn works_with_env() {
    let t = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").env("CARGO_BUILD_TARGET", t).run();

    assert!(p.target_bin(t, "foo").is_file());
}
