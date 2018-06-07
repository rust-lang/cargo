use super::project;

#[test]
fn specify_rustflags() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![allow(unused)]
                #![feature(rust_2018_preview)]

                mod foo {
                    pub const FOO: &str = "fooo";
                }

                fn main() {
                    let x = ::foo::FOO;
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FIXING] src/lib.rs (1 fix)
[FINISHED] dev [unoptimized + debuginfo]
";

    p.expect_cmd("cargo-fix fix --prepare-for 2018")
        .env("RUSTFLAGS", "-C target-cpu=native")
        .stdout("")
        .stderr(stderr)
        .run();
}
