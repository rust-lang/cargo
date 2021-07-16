//! Tests for the cache file for the rustc version info.

use cargo_test_support::{basic_bin_manifest, paths::CargoPathExt, rustc_host};
use cargo_test_support::{basic_manifest, project};
use std::env;

const MISS: &str = "[..] rustc info cache miss[..]";
const HIT: &str = "[..]rustc info cache hit[..]";
const UPDATE: &str = "[..]updated rustc info cache[..]";

#[ignore]
#[cargo_test]
fn rustc_info_cache() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .with_stderr_contains("[..]failed to read rustc info cache[..]")
        .with_stderr_contains(MISS)
        .with_stderr_does_not_contain(HIT)
        .with_stderr_contains(UPDATE)
        .run();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .with_stderr_contains("[..]reusing existing rustc info cache[..]")
        .with_stderr_contains(HIT)
        .with_stderr_does_not_contain(MISS)
        .with_stderr_does_not_contain(UPDATE)
        .run();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("CARGO_CACHE_RUSTC_INFO", "0")
        .with_stderr_contains("[..]rustc info cache disabled[..]")
        .with_stderr_does_not_contain(UPDATE)
        .run();

    let other_rustc = {
        let p = project()
            .at("compiler")
            .file("Cargo.toml", &basic_manifest("compiler", "0.1.0"))
            .file(
                "src/main.rs",
                r#"
                    use std::process::Command;
                    use std::env;

                    fn main() {
                        let mut cmd = Command::new("rustc");
                        for arg in env::args_os().skip(1) {
                            cmd.arg(arg);
                        }
                        std::process::exit(cmd.status().unwrap().code().unwrap());
                    }
                "#,
            )
            .build();
        p.cargo("build").run();

        p.root()
            .join(&format!("target/{}/debug/compiler", rustc_host()))
            .with_extension(env::consts::EXE_EXTENSION)
    };

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC", other_rustc.display().to_string())
        .with_stderr_contains("[..]different compiler, creating new rustc info cache[..]")
        .with_stderr_contains(MISS)
        .with_stderr_does_not_contain(HIT)
        .with_stderr_contains(UPDATE)
        .run();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC", other_rustc.display().to_string())
        .with_stderr_contains("[..]reusing existing rustc info cache[..]")
        .with_stderr_contains(HIT)
        .with_stderr_does_not_contain(MISS)
        .with_stderr_does_not_contain(UPDATE)
        .run();

    other_rustc.move_into_the_future();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC", other_rustc.display().to_string())
        .with_stderr_contains("[..]different compiler, creating new rustc info cache[..]")
        .with_stderr_contains(MISS)
        .with_stderr_does_not_contain(HIT)
        .with_stderr_contains(UPDATE)
        .run();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC", other_rustc.display().to_string())
        .with_stderr_contains("[..]reusing existing rustc info cache[..]")
        .with_stderr_contains(HIT)
        .with_stderr_does_not_contain(MISS)
        .with_stderr_does_not_contain(UPDATE)
        .run();
}

#[ignore]
#[cargo_test]
fn rustc_info_cache_with_wrappers() {
    let wrapper_project = project()
        .at("wrapper")
        .file("Cargo.toml", &basic_bin_manifest("wrapper"))
        .file("src/main.rs", r#"fn main() { }"#)
        .build();
    let wrapper = wrapper_project.bin("wrapper");

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.0.0"
                authors = []
                [workspace]
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    for &wrapper_env in ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"].iter() {
        p.cargo("clean").with_status(0).run();
        wrapper_project.change_file(
            "src/main.rs",
            r#"
            fn main() {
                let mut args = std::env::args_os();
                let _me = args.next().unwrap();
                let rustc = args.next().unwrap();
                let status = std::process::Command::new(rustc).args(args).status().unwrap();
                std::process::exit(if status.success() { 0 } else { 1 })
            }
            "#,
        );
        wrapper_project.cargo("build").with_status(0).run();

        p.cargo("build")
            .env("CARGO_LOG", "cargo::util::rustc=debug")
            .env(wrapper_env, &wrapper)
            .with_stderr_contains("[..]failed to read rustc info cache[..]")
            .with_stderr_contains(MISS)
            .with_stderr_contains(UPDATE)
            .with_stderr_does_not_contain(HIT)
            .with_status(0)
            .run();
        p.cargo("build")
            .env("CARGO_LOG", "cargo::util::rustc=debug")
            .env(wrapper_env, &wrapper)
            .with_stderr_contains("[..]reusing existing rustc info cache[..]")
            .with_stderr_contains(HIT)
            .with_stderr_does_not_contain(UPDATE)
            .with_stderr_does_not_contain(MISS)
            .with_status(0)
            .run();

        wrapper_project.change_file("src/main.rs", r#"fn main() { panic!() }"#);
        wrapper_project.cargo("build").with_status(0).run();

        p.cargo("build")
            .env("CARGO_LOG", "cargo::util::rustc=debug")
            .env(wrapper_env, &wrapper)
            .with_stderr_contains("[..]different compiler, creating new rustc info cache[..]")
            .with_stderr_contains(MISS)
            .with_stderr_contains(UPDATE)
            .with_stderr_does_not_contain(HIT)
            .with_status(101)
            .run();
        p.cargo("build")
            .env("CARGO_LOG", "cargo::util::rustc=debug")
            .env(wrapper_env, &wrapper)
            .with_stderr_contains("[..]reusing existing rustc info cache[..]")
            .with_stderr_contains(HIT)
            .with_stderr_does_not_contain(UPDATE)
            .with_stderr_does_not_contain(MISS)
            .with_status(101)
            .run();
    }
}
