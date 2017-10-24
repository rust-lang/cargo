extern crate cargotest;
extern crate hamcrest;
use cargotest::support::{project, execs, basic_bin_manifest};
use hamcrest::{assert_that};

#[test]
fn alias_incorrect_config_type() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
            fn main() {
        }"#)
        .file(".cargo/config",r#"
            [alias]
            b-cargo-test = 5
        "#)
        .build();

    assert_that(p.cargo("b-cargo-test").arg("-v"),
                execs().with_status(101).
                with_stderr_contains("[ERROR] invalid configuration \
for key `alias.b-cargo-test`
expected a list, but found a integer for [..]"));
}


#[test]
fn alias_default_config_overrides_config() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
            fn main() {
        }"#)
        .file(".cargo/config",r#"
            [alias]
            b = "not_build"
        "#)
        .build();

    assert_that(p.cargo("b").arg("-v"),
                execs().with_status(0).
                with_stderr_contains("[COMPILING] foo v0.5.0 [..]"));
}

#[test]
fn alias_config() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
            fn main() {
        }"#)
        .file(".cargo/config",r#"
            [alias]
            b-cargo-test = "build"
        "#)
        .build();

    assert_that(p.cargo("b-cargo-test").arg("-v"),
                execs().with_status(0).
                with_stderr_contains("[COMPILING] foo v0.5.0 [..]
[RUNNING] `rustc --crate-name foo [..]"));
}

#[test]
fn alias_list_test() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
            fn main() {
         }"#)
        .file(".cargo/config",r#"
            [alias]
            b-cargo-test = ["build", "--release"]
         "#)
        .build();

    assert_that(p.cargo("b-cargo-test").arg("-v"),
                execs().with_status(0).
                with_stderr_contains("[COMPILING] foo v0.5.0 [..]").
                with_stderr_contains("[RUNNING] `rustc --crate-name [..]")
                );
}

#[test]
fn alias_with_flags_config() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
            fn main() {
         }"#)
        .file(".cargo/config",r#"
            [alias]
            b-cargo-test = "build --release"
         "#)
        .build();

    assert_that(p.cargo("b-cargo-test").arg("-v"),
                execs().with_status(0).
                with_stderr_contains("[COMPILING] foo v0.5.0 [..]").
                with_stderr_contains("[RUNNING] `rustc --crate-name foo [..]")
                );
}

#[test]
fn cant_shadow_builtin() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"
            fn main() {
         }"#)
        .file(".cargo/config",r#"
            [alias]
            build = "fetch"
         "#)
        .build();

    assert_that(p.cargo("build"),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}
