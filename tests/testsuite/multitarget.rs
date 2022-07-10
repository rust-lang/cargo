//! Tests for multiple `--target` flags to subcommands

use cargo_test_support::{basic_manifest, cross_compile, project, rustc_host};

#[cargo_test]
fn double_target_rejected() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --target a --target b")
        .with_stderr("[ERROR] specifying multiple `--target` flags requires `-Zmultitarget`")
        .with_status(101)
        .run();
}

#[cargo_test]
fn array_of_target_rejected_with_config() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                target = ["a", "b"]
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr(
            "[ERROR] specifying an array in `build.target` config value requires `-Zmultitarget`",
        )
        .with_status(101)
        .run();

    p.change_file(
        ".cargo/config.toml",
        r#"
            [build]
            target = ["a"]
        "#,
    );

    p.cargo("build")
        .with_stderr(
            "[ERROR] specifying an array in `build.target` config value requires `-Zmultitarget`",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn simple_build() {
    if cross_compile::disabled() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Z multitarget")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .masquerade_as_nightly_cargo()
        .run();

    assert!(p.target_bin(t1, "foo").is_file());
    assert!(p.target_bin(t2, "foo").is_file());
}

#[cargo_test]
fn simple_build_with_config() {
    if cross_compile::disabled() {
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
                    [unstable]
                    multitarget = true
                    [build]
                    target = ["{t1}", "{t2}"]
                "#
            ),
        )
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().run();

    assert!(p.target_bin(t1, "foo").is_file());
    assert!(p.target_bin(t2, "foo").is_file());
}

#[cargo_test]
fn simple_test() {
    if !cross_compile::can_run_on_host() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "fn main() {}")
        .build();

    p.cargo("test -Z multitarget")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(&format!("[RUNNING] [..]{}[..]", t1))
        .with_stderr_contains(&format!("[RUNNING] [..]{}[..]", t2))
        .run();
}

#[cargo_test]
fn simple_run() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("run -Z multitarget --target a --target b")
        .with_stderr("[ERROR] only one `--target` argument is supported")
        .with_status(101)
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn simple_doc() {
    if cross_compile::disabled() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "//! empty lib")
        .build();

    p.cargo("doc -Z multitarget")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .masquerade_as_nightly_cargo()
        .run();

    assert!(p.build_dir().join(&t1).join("doc/foo/index.html").is_file());
    assert!(p.build_dir().join(&t2).join("doc/foo/index.html").is_file());
}

#[cargo_test]
fn simple_check() {
    if cross_compile::disabled() {
        return;
    }
    let t1 = cross_compile::alternate();
    let t2 = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Z multitarget")
        .arg("--target")
        .arg(&t1)
        .arg("--target")
        .arg(&t2)
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn same_value_twice() {
    if cross_compile::disabled() {
        return;
    }
    let t = rustc_host();
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Z multitarget")
        .arg("--target")
        .arg(&t)
        .arg("--target")
        .arg(&t)
        .masquerade_as_nightly_cargo()
        .run();

    assert!(p.target_bin(t, "foo").is_file());
}

#[cargo_test]
fn same_value_twice_with_config() {
    if cross_compile::disabled() {
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
                    [unstable]
                    multitarget = true
                    [build]
                    target = ["{t}", "{t}"]
                "#
            ),
        )
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().run();

    assert!(p.target_bin(t, "foo").is_file());
}

#[cargo_test]
fn works_with_config_in_both_string_or_list() {
    if cross_compile::disabled() {
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
                    [unstable]
                    multitarget = true
                    [build]
                    target = "{t}"
                "#
            ),
        )
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().run();

    assert!(p.target_bin(t, "foo").is_file());

    p.cargo("clean").run();

    p.change_file(
        ".cargo/config.toml",
        &format!(
            r#"
                [unstable]
                multitarget = true
                [build]
                target = ["{t}"]
            "#
        ),
    );

    p.cargo("build").masquerade_as_nightly_cargo().run();

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
