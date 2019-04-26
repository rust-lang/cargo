use crate::support::{basic_bin_manifest, basic_manifest, project};

#[test]
fn build_lib_only() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("build --lib -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs --color never --crate-type lib \
        --emit=[..]link -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
}

#[test]
fn build_with_no_lib() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --lib")
        .with_status(101)
        .with_stderr("[ERROR] no library targets found in package `foo`")
        .run();
}

#[test]
fn build_with_relative_cargo_home_path() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]

            name = "foo"
            version = "0.0.1"
            authors = ["wycats@example.com"]

            [dependencies]

            "test-dependency" = { path = "src/test_dependency" }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/test_dependency/src/lib.rs", r#" "#)
        .file(
            "src/test_dependency/Cargo.toml",
            &basic_manifest("test-dependency", "0.0.1"),
        )
        .build();

    p.cargo("build").env("CARGO_HOME", "./cargo_home/").run();
}
