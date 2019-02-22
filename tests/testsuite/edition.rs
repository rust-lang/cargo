use crate::support::{basic_lib_manifest, is_nightly, project};

#[test]
fn edition_works_for_build_script() {
    if !is_nightly() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = '2018'

                [build-dependencies]
                a = { path = 'a' }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    a::foo();
                }
            "#,
        )
        .file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("build -v").masquerade_as_nightly_cargo().run();
}
