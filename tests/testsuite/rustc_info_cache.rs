//! Tests for the cache file for the rustc version info.

use cargo_test_support::{basic_bin_manifest, paths::CargoPathExt};
use cargo_test_support::{basic_manifest, project, str};
use std::env;

#[cargo_test]
fn rustc_info_cache() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .with_stderr_data(str![[r#"
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: failed to read rustc info cache: failed to read `[ROOT]/foo/target/.rustc_info.json`
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `rustc -vV`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `rustc - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --check-cfg 'cfg()'`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `rustc - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --print=sysroot --print=split-debuginfo --print=crate-name --print=cfg`
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: failed to read rustc info cache: failed to read `[ROOT]/foo/target/.rustc_info.json`
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `rustc -vV`
   [..]s  WARN cargo::util::rustc: failed to update rustc info cache: failed to write `[ROOT]/foo/target/.rustc_info.json`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
   [..]s  INFO cargo::util::rustc: updated rustc info cache

"#]])

        .run();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .with_stderr_data(str![[r#"
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
   [..]s DEBUG cargo::util::rustc: rustc info cache hit
   [..]s DEBUG cargo::util::rustc: rustc info cache hit
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("CARGO_CACHE_RUSTC_INFO", "0")
        .with_stderr_data(str![[r#"
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=None}: cargo::util::rustc: rustc info cache disabled
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=None}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=None}: cargo::util::rustc: running `rustc -vV`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `rustc - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --check-cfg 'cfg()'`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `rustc - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --print=sysroot --print=split-debuginfo --print=crate-name --print=cfg`
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=None}: cargo::util::rustc: rustc info cache disabled
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=None}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="rustc" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=None}: cargo::util::rustc: running `rustc -vV`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            .join("target/debug/compiler")
            .with_extension(env::consts::EXE_EXTENSION)
    };

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC", other_rustc.display().to_string())
        .with_stderr_data(str![[r#"
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: different compiler, creating new rustc info cache
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `[ROOT]/compiler/target/debug/compiler -vV`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `[ROOT]/compiler/target/debug/compiler - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --check-cfg 'cfg()'`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `[ROOT]/compiler/target/debug/compiler - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --print=sysroot --print=split-debuginfo --print=crate-name --print=cfg`
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: different compiler, creating new rustc info cache
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `[ROOT]/compiler/target/debug/compiler -vV`
   [..]s  INFO cargo::util::rustc: updated rustc info cache
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
   [..]s  INFO cargo::util::rustc: updated rustc info cache

"#]])
        .run();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC", other_rustc.display().to_string())
        .with_stderr_data(str![[r#"
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
   [..]s DEBUG cargo::util::rustc: rustc info cache hit
   [..]s DEBUG cargo::util::rustc: rustc info cache hit
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    other_rustc.move_into_the_future();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC", other_rustc.display().to_string())
        .with_stderr_data(str![[r#"
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: different compiler, creating new rustc info cache
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `[ROOT]/compiler/target/debug/compiler -vV`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `[ROOT]/compiler/target/debug/compiler - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --check-cfg 'cfg()'`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `[ROOT]/compiler/target/debug/compiler - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --print=sysroot --print=split-debuginfo --print=crate-name --print=cfg`
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: different compiler, creating new rustc info cache
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `[ROOT]/compiler/target/debug/compiler -vV`
   [..]s  INFO cargo::util::rustc: updated rustc info cache
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
   [..]s  INFO cargo::util::rustc: updated rustc info cache

"#]])
        .run();

    p.cargo("build")
        .env("CARGO_LOG", "cargo::util::rustc=debug")
        .env("RUSTC", other_rustc.display().to_string())
        .with_stderr_data(str![[r#"
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
   [..]s DEBUG cargo::util::rustc: rustc info cache hit
   [..]s DEBUG cargo::util::rustc: rustc info cache hit
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="[ROOT]/compiler/target/debug/compiler" wrapper=None workspace_wrapper=None rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

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
            .with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: no edition set: defaulting to the 2015 edition while the latest is 2021
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: failed to read rustc info cache: failed to read `[ROOT]/foo/target/.rustc_info.json`
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `[ROOT]/wrapper/target/debug/wrapper rustc -vV`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `[ROOT]/wrapper/target/debug/wrapper rustc - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --check-cfg 'cfg()'`
   [..]s DEBUG cargo::util::rustc: rustc info cache miss
   [..]s DEBUG cargo::util::rustc: running `[ROOT]/wrapper/target/debug/wrapper rustc - --crate-name ___ --print=file-names --crate-type bin --crate-type rlib --crate-type dylib --crate-type cdylib --crate-type staticlib --crate-type proc-macro --print=sysroot --print=split-debuginfo --print=crate-name --print=cfg`
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: failed to read rustc info cache: failed to read `[ROOT]/foo/target/.rustc_info.json`
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `[ROOT]/wrapper/target/debug/wrapper rustc -vV`
   [..]s  WARN cargo::util::rustc: failed to update rustc info cache: failed to write `[ROOT]/foo/target/.rustc_info.json`
[COMPILING] test v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
   [..]s  INFO cargo::util::rustc: updated rustc info cache

"#]])
            .with_status(0)
            .run();
        p.cargo("build")
            .env("CARGO_LOG", "cargo::util::rustc=debug")
            .env(wrapper_env, &wrapper)
            .with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: no edition set: defaulting to the 2015 edition while the latest is 2021
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
   [..]s DEBUG cargo::util::rustc: rustc info cache hit
   [..]s DEBUG cargo::util::rustc: rustc info cache hit
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
            .with_status(0)
            .run();

        wrapper_project.change_file("src/main.rs", r#"fn main() { panic!() }"#);
        wrapper_project.cargo("build").with_status(0).run();

        p.cargo("build")
            .env("CARGO_LOG", "cargo::util::rustc=debug")
            .env(wrapper_env, &wrapper)
            .with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: no edition set: defaulting to the 2015 edition while the latest is 2021
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: different compiler, creating new rustc info cache
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache miss
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: running `[ROOT]/wrapper/target/debug/wrapper rustc -vV`
   [..]s  INFO new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: updated rustc info cache
[ERROR] process didn't exit successfully: `[ROOT]/wrapper/target/debug/wrapper rustc -vV` ([EXIT_STATUS]: 101)
--- stderr
thread 'main' panicked at src/main.rs:1:13:
explicit panic
[NOTE] run with `RUST_BACKTRACE=1` environment variable to display a backtrace


"#]])
            .with_status(101)
            .run();
        p.cargo("build")
            .env("CARGO_LOG", "cargo::util::rustc=debug")
            .env(wrapper_env, &wrapper)
            .with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: no edition set: defaulting to the 2015 edition while the latest is 2021
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: reusing existing rustc info cache
   [..]s DEBUG new{path="rustc" wrapper=[..] workspace_wrapper=[..] rustup_rustc="[ROOT]/home/.cargo/bin/rustc" cache_location=Some("[ROOT]/foo/target/.rustc_info.json")}: cargo::util::rustc: rustc info cache hit
[ERROR] process didn't exit successfully: `[ROOT]/wrapper/target/debug/wrapper rustc -vV` ([EXIT_STATUS]: 101)
--- stderr
thread 'main' panicked at src/main.rs:1:13:
explicit panic
[NOTE] run with `RUST_BACKTRACE=1` environment variable to display a backtrace


"#]])
            .with_status(101)
            .run();
    }
}
