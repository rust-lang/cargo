use support::{basic_lib_manifest, execs, project};
use support::{rustc_host, ChannelChanger};
use support::hamcrest::assert_that;

#[test]
fn metabuild_gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            metabuild = ["mb"]
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr_contains(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `metabuild` is required

consider adding `cargo-features = [\"metabuild\"]` to the manifest
",
        ),
    );
}

#[test]
fn metabuild_basic() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "foo"
            version = "0.0.1"
            metabuild = ["mb", "mb-other"]

            [build-dependencies]
            mb = {path="mb"}
            mb-other = {path="mb-other"}
        "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .file(
            "mb-other/Cargo.toml",
            r#"
            [package]
            name = "mb-other"
            version = "0.0.1"
        "#,
        )
        .file(
            "mb-other/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb-other"); }"#,
        )
        .build();

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs()
            .with_status(0)
            .with_stdout_contains("Hello mb")
            .with_stdout_contains("Hello mb-other"),
    );
}

#[test]
fn metabuild_error_both() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "foo"
            version = "0.0.1"
            metabuild = "mb"

            [build-dependencies]
            mb = {path="mb"}
        "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"fn main() {}"#)
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .build();

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr_contains(
            "\
error: failed to parse manifest at [..]

Caused by:
  cannot specify both `metabuild` and `build`
",
        ),
    );
}

#[test]
fn metabuild_missing_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "foo"
            version = "0.0.1"
            metabuild = "mb"
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr_contains(
            "\
error: failed to parse manifest at [..]

Caused by:
  metabuild package `mb` must be specified in `build-dependencies`",
        ),
    );
}

#[test]
fn metabuild_optional_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "foo"
            version = "0.0.1"
            metabuild = "mb"

            [build-dependencies]
            mb = {path="mb", optional=true}
        "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .build();

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs()
            .with_status(0)
            .with_stdout_does_not_contain("Hello mb"),
    );

    assert_that(
        p.cargo("build -vv --features mb")
            .masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stdout_contains("Hello mb"),
    );
}

#[test]
fn metabuild_lib_name() {
    // Test when setting `name` on [lib].
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "foo"
            version = "0.0.1"
            metabuild = "mb"

            [build-dependencies]
            mb = {path="mb"}
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "mb/Cargo.toml",
            r#"
            [package]
            name = "mb"
            version = "0.0.1"
            [lib]
            name = "other"
        "#,
        )
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .build();

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stdout_contains("Hello mb"),
    );
}

#[test]
fn metabuild_fresh() {
    // Check that rebuild is fresh.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "foo"
            version = "0.0.1"
            metabuild = "mb"

            [build-dependencies]
            mb = {path="mb"}
        "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb"); }"#,
        )
        .build();

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stdout_contains("Hello mb"),
    );

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs()
            .with_status(0)
            .with_stdout_does_not_contain("Hello mb")
            .with_stderr(
                "\
[FRESH] mb [..]
[FRESH] foo [..]
[FINISHED] dev [..]
",
            ),
    );
}

#[test]
fn metabuild_links() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "foo"
            version = "0.0.1"
            links = "cat"
            metabuild = "mb"

            [build-dependencies]
            mb = {path="mb"}
        "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() {
                assert_eq!(std::env::var("CARGO_MANIFEST_LINKS"),
                    Ok("cat".to_string()));
                println!("Hello mb");
            }"#,
        )
        .build();

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs().with_status(0).with_stdout_contains("Hello mb"),
    );
}

#[test]
fn metabuild_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "foo"
            version = "0.0.1"
            links = "cat"
            metabuild = "mb"

            [build-dependencies]
            mb = {path="mb"}
        "#,
        )
        .file("src/lib.rs", "")
        .file("mb/Cargo.toml", &basic_lib_manifest("mb"))
        .file(
            "mb/src/lib.rs",
            r#"pub fn metabuild() { panic!("should not run"); }"#,
        )
        .file(
            ".cargo/config",
            &format!(
                r#"
            [target.{}.cat]
            rustc-link-lib = ["a"]
        "#,
                rustc_host()
            ),
        )
        .build();

    assert_that(
        p.cargo("build -vv").masquerade_as_nightly_cargo(),
        execs().with_status(0),
    );
}

#[test]
fn metabuild_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["member1", "member2"]
        "#,
        )
        .file(
            "member1/Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "member1"
            version = "0.0.1"
            metabuild = ["mb1", "mb2"]

            [build-dependencies]
            mb1 = {path="../../mb1"}
            mb2 = {path="../../mb2"}
        "#,
        )
        .file("member1/src/lib.rs", "")
        .file(
            "member2/Cargo.toml",
            r#"
            cargo-features = ["metabuild"]
            [package]
            name = "member2"
            version = "0.0.1"
            metabuild = ["mb1"]

            [build-dependencies]
            mb1 = {path="../../mb1"}
        "#,
        )
        .file("member2/src/lib.rs", "")
        .build();

    project()
        .at("mb1")
        .file("Cargo.toml", &basic_lib_manifest("mb1"))
        .file(
            "src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb1 {}", std::env::var("CARGO_MANIFEST_DIR").unwrap()); }"#,
        )
        .build();

    project()
        .at("mb2")
        .file("Cargo.toml", &basic_lib_manifest("mb2"))
        .file(
            "src/lib.rs",
            r#"pub fn metabuild() { println!("Hello mb2 {}", std::env::var("CARGO_MANIFEST_DIR").unwrap()); }"#,
        )
        .build();

    assert_that(
        p.cargo("build -vv --all").masquerade_as_nightly_cargo(),
        execs()
            .with_status(0)
            .with_stdout_contains("Hello mb1 [..]member1")
            .with_stdout_contains("Hello mb2 [..]member1")
            .with_stdout_contains("Hello mb1 [..]member2")
            .with_stdout_does_not_contain("Hello mb2 [..]member2"),
    );
}
