use cargo_test_support::project;

#[cargo_test]
fn carg_add_with_vendored_packages() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hi!"); }"#)
        .file(
            "Cargo.toml",
            r#"[package]
name = "example"
version = "0.1.0"
edition = "2021"

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
xml-rs = "0.8.4""#,
        )
        .build();

    p.cargo("vendor ./vendor")
        .with_stdout(
            r#"
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "./vendor""#,
        )
        .run();
    p.change_file(
        ".cargo/config.toml",
        r#"
    [source.crates-io]
    replace-with = "vendored-sources"
    
    [source.vendored-sources]
    directory = "./vendor""#,
    );
    p.cargo("add cbindgen")
        // current output
        //.with_stdout(r#"warning: translating `cbindgen` to `xml-rs`
        //Adding xml-rs v0.8.4 to dependencies."#)
        // correct output
        .with_stdout(
            r#"    Updating crates.io index
      Adding xml-rs v0.8.4 to dependencies."#,
        )
        .run();
}
