//! Tests for setting custom rustdoc flags.

use cargo_test_support::project;

#[cargo_test]
fn parses_env() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("doc -v")
        .env("RUSTDOCFLAGS", "--cfg=foo")
        .with_stderr_contains("[RUNNING] `rustdoc [..] --cfg=foo[..]`")
        .run();
}

#[cargo_test]
fn parses_config() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
                [build]
                rustdocflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p.cargo("doc -v")
        .with_stderr_contains("[RUNNING] `rustdoc [..] --cfg foo[..]`")
        .run();
}

#[cargo_test]
fn bad_flags() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--bogus")
        .with_status(101)
        .with_stderr_contains("[..]bogus[..]")
        .run();
}

#[cargo_test]
fn rerun() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=foo").run();
    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--cfg=foo")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--cfg=bar")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn rustdocflags_passed_to_rustdoc_through_cargo_test() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                //! ```
                //! assert!(cfg!(do_not_choke));
                //! ```
            "#,
        )
        .build();

    p.cargo("test --doc")
        .env("RUSTDOCFLAGS", "--cfg do_not_choke")
        .run();
}

#[cargo_test]
fn rustdocflags_passed_to_rustdoc_through_cargo_test_only_once() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("test --doc")
        .env("RUSTDOCFLAGS", "--markdown-no-toc")
        .run();
}

#[cargo_test]
fn rustdocflags_misspelled() {
    let p = project().file("src/main.rs", "fn main() { }").build();

    p.cargo("doc")
        .env("RUSTDOC_FLAGS", "foo")
        .with_stderr_contains("[WARNING] Cargo does not read `RUSTDOC_FLAGS` environment variable. Did you mean `RUSTDOCFLAGS`?")
        .run();
}

#[cargo_test]
fn whitespace() {
    // Checks behavior of different whitespace characters.
    let p = project().file("src/lib.rs", "").build();

    // "too many operands"
    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--crate-version this has spaces")
        .with_stderr_contains("[ERROR] could not document `foo`")
        .with_status(101)
        .run();

    p.cargo("doc")
        .env_remove("__CARGO_TEST_FORCE_ARGFILE") // Not applicable for argfile.
        .env(
            "RUSTDOCFLAGS",
            "--crate-version 1111\n2222\t3333\u{00a0}4444",
        )
        .run();

    let contents = p.read_file("target/doc/foo/index.html");
    assert!(contents.contains("1111"));
    assert!(contents.contains("2222"));
    assert!(contents.contains("3333"));
    assert!(contents.contains("4444"));
}

#[cargo_test]
fn not_affected_by_target_rustflags() {
    let cfg = if cfg!(windows) { "windows" } else { "unix" };
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.'cfg({cfg})']
                    rustflags = ["-D", "missing-docs"]

                    [build]
                    rustdocflags = ["--cfg", "foo"]
                "#,
            ),
        )
        .build();

    // `cargo build` should fail due to missing docs.
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains("[RUNNING] `rustc [..] -D missing-docs[..]`")
        .run();

    // `cargo doc` shouldn't fail.
    p.cargo("doc -v")
        .with_stderr_contains("[RUNNING] `rustdoc [..] --cfg foo[..]`")
        .run();
}
