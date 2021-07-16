//! Tests for multiple `--target` flags to subcommands

use cargo_test_support::{basic_manifest, cross_compile, project, rustc_host};

#[cargo_test]
fn double_target_rejected() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --target a --target b")
        .with_stderr("error: specifying multiple `--target` flags requires `-Zmultitarget`")
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
        .with_stderr("error: only one `--target` argument is supported")
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

    assert!(p
        .root()
        .join("target")
        .join(&t1)
        .join("doc/foo/index.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(&t2)
        .join("doc/foo/index.html")
        .is_file());
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
