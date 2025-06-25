//! Test if PGO works.

use std::path::PathBuf;
use std::process::Command;

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

fn llvm_profdata() -> Option<PathBuf> {
    let output = Command::new("rustc")
        .arg("--print=target-libdir")
        .output()
        .expect("rustc to run");
    assert!(output.status.success());
    let mut libdir = PathBuf::from(String::from_utf8(output.stdout).unwrap());
    assert!(libdir.pop());
    let mut bin = libdir.join("bin").join("llvm-profdata");
    bin.exists().then(|| bin.clone()).or_else(|| {
        bin.set_extension("exe");
        bin.exists().then_some(bin)
    })
}

// Rustc build may be without profiling support.
// Mark it as nightly so it won't run on rust-lang/rust CI.
#[cfg_attr(
    target_os = "linux",
    cargo_test(nightly, reason = "rust-lang/rust#133675")
)]
// macOS may emit different LLVM PGO warnings.
// Windows LLVM has different requirements.
#[cfg_attr(not(target_os = "linux"), cargo_test, ignore = "linux only")]
fn pgo_works() {
    let Some(llvm_profdata) = llvm_profdata() else {
        return;
    };

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2021"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn fibonacci(n: u64) -> u64 {
                    match n {
                        0 => 0,
                        1 => 1,
                        _ => fibonacci(n - 1) + fibonacci(n - 2),
                    }
                }

                fn main() {
                    for i in [15, 20, 25] {
                        let _ = fibonacci(i);
                    }
                }
            "#,
        )
        .build();

    let target_dir = p.build_dir();
    let release_bin = target_dir.join("release").join("foo");
    let pgo_data_dir = target_dir.join("pgo-data");
    let profdata_path = target_dir.join("merged.profdata");

    // Build the instrumented binary
    p.cargo("build --release")
        .env(
            "RUSTFLAGS",
            format!("-Cprofile-generate={}", pgo_data_dir.display()),
        )
        .run();
    // Run the instrumented binary
    cargo_test_support::execs()
        .with_process_builder(cargo_test_support::process(release_bin))
        .run();

    cargo_test_support::process(llvm_profdata)
        .arg("merge")
        .arg("-o")
        .arg(&profdata_path)
        .arg(pgo_data_dir)
        .status()
        .unwrap();

    // Use merged profdata during optimization.
    //
    // -Cllvm-args=-pgo-warn-missing-function is essential.
    // If there are LLVM warnings, there might be something wrong.
    p.cargo("build --release -v")
        .env(
            "RUSTFLAGS",
            format!(
                "-Cprofile-use={} -Cllvm-args=-pgo-warn-missing-function",
                profdata_path.display()
            ),
        )
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-Cprofile-use=[ROOT]/foo/target/merged.profdata -Cllvm-args=-pgo-warn-missing-function`
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();
}
