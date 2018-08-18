use support::{execs, project};
use support::hamcrest::assert_that;

#[test]
fn parses_env() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("doc -v").env("RUSTDOCFLAGS", "--cfg=foo"),
        execs()
            .with_stderr_contains("[RUNNING] `rustdoc [..] --cfg=foo[..]`"),
    );
}

#[test]
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

    assert_that(
        p.cargo("doc -v"),
        execs()
            .with_stderr_contains("[RUNNING] `rustdoc [..] --cfg foo[..]`"),
    );
}

#[test]
fn bad_flags() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("doc").env("RUSTDOCFLAGS", "--bogus"),
        execs().with_status(101),
    );
}

#[test]
fn rerun() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=foo"),
        execs(),
    );
    assert_that(
        p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=foo"),
        execs()
            .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"),
    );
    assert_that(
        p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=bar"),
        execs().with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
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

    assert_that(
        p.cargo("test --doc")
            .env("RUSTDOCFLAGS", "--cfg do_not_choke"),
        execs(),
    );
}

#[test]
fn rustdocflags_passed_to_rustdoc_through_cargo_test_only_once() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("test --doc")
            .env("RUSTDOCFLAGS", "--markdown-no-toc"),
        execs(),
    );
}
