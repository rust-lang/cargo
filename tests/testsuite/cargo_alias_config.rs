use support::{basic_bin_manifest, project};

#[test]
fn alias_incorrect_config_type() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [alias]
            b-cargo-test = 5
        "#,
        ).build();

    p.cargo("b-cargo-test -v")
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] invalid configuration for key `alias.b-cargo-test`
expected a list, but found a integer for [..]",
        ).run();
}

#[test]
fn alias_default_config_overrides_config() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [alias]
            b = "not_build"
        "#,
        ).build();

    p.cargo("b -v")
        .with_stderr_contains("[COMPILING] foo v0.5.0 [..]")
        .run();
}

#[test]
fn alias_config() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [alias]
            b-cargo-test = "build"
        "#,
        ).build();

    p.cargo("b-cargo-test -v")
        .with_stderr_contains(
            "\
[COMPILING] foo v0.5.0 [..]
[RUNNING] `rustc --crate-name foo [..]",
        ).run();
}

#[test]
fn recursive_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r"fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [alias]
            b-cargo-test = "build"
            a-cargo-test = ["b-cargo-test", "-v"]
        "#,
        ).build();

    p.cargo("a-cargo-test")
        .with_stderr_contains(
            "\
[COMPILING] foo v0.5.0 [..]
[RUNNING] `rustc --crate-name foo [..]",
        ).run();
}

#[test]
fn alias_list_test() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [alias]
            b-cargo-test = ["build", "--release"]
         "#,
        ).build();

    p.cargo("b-cargo-test -v")
        .with_stderr_contains("[COMPILING] foo v0.5.0 [..]")
        .with_stderr_contains("[RUNNING] `rustc --crate-name [..]")
        .run();
}

#[test]
fn alias_with_flags_config() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [alias]
            b-cargo-test = "build --release"
         "#,
        ).build();

    p.cargo("b-cargo-test -v")
        .with_stderr_contains("[COMPILING] foo v0.5.0 [..]")
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..]")
        .run();
}

#[test]
fn cant_shadow_builtin() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [alias]
            build = "fetch"
         "#,
        ).build();

    p.cargo("build")
        .with_stderr(
            "\
[WARNING] alias `build` is ignored, because it is shadowed by a built-in command
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
}

#[test]
fn cant_shadow_builtin_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [alias]
            b = "fetch"
         "#,
        ).build();

    p.cargo("b")
        .with_stderr(
            "\
[WARNING] alias `b` is ignored, because it is shadowed by a built-in alias for `build`
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ).run();
}
