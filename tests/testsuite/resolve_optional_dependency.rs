use crate::prelude::*;
use cargo_test_support::project;

#[cargo_test(ignore_windows = "test windows only dependency on unix systems")]
fn cargo_test_should_not_demand_not_required_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "sample"
version = "0.1.0"
edition = "2024"

[features]
default = ["feat"]
feat = ["dep:ipconfig"]

[target.'cfg(windows)'.dependencies]
ipconfig = { version = "0.3.2", optional = true }

[[example]]
name = "demo"
required-features = ["feat"]
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("examples/demo.rs", "fn main() {}")
        .build();

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        p.cargo("fetch --target=x86_64-unknown-linux-gnu").run();
        p.cargo("test --target=x86_64-unknown-linux-gnu --frozen")
            .run();
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        p.cargo("fetch --target=aarch64-unknown-linux-gnu").run();
        p.cargo("test --target=aarch64-unknown-linux-gnu --frozen")
            .run();
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        p.cargo("fetch --target=aarch64-apple-darwin").run();
        p.cargo("test --target=aarch64-apple-darwin --frozen").run();
    }
}
