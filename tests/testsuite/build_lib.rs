use support::{basic_manifest, basic_bin_manifest, execs, project, Project};
use support::hamcrest::assert_that;

fn verbose_output_for_lib(p: &Project) -> String {
    format!(
        "\
[COMPILING] {name} v{version} ({url})
[RUNNING] `rustc --crate-name {name} src/lib.rs --crate-type lib \
        --emit=dep-info,link -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        dir = p.root().display(),
        url = p.url(),
        name = "foo",
        version = "0.0.1"
    )
}

#[test]
fn build_lib_only() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    assert_that(
        p.cargo("build --lib -v"),
        execs()
            .with_stderr(verbose_output_for_lib(&p)),
    );
}

#[test]
fn build_with_no_lib() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("build --lib"),
        execs()
            .with_status(101)
            .with_stderr("[ERROR] no library targets found in package `foo`"),
    );
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
        .file("src/test_dependency/Cargo.toml", &basic_manifest("test-dependency", "0.0.1"))
        .build();

    assert_that(
        p.cargo("build").env("CARGO_HOME", "./cargo_home/"),
        execs(),
    );
}
