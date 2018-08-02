use support::{basic_manifest, execs, project};
use support::paths::CargoPathExt;
use support::hamcrest::assert_that;
use std::env;

#[test]
fn rustc_info_cache() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    let miss = "[..] rustc info cache miss[..]";
    let hit = "[..]rustc info cache hit[..]";
    let update = "[..]updated rustc info cache[..]";

    assert_that(
        p.cargo("build").env("RUST_LOG", "cargo::util::rustc=info"),
        execs()
            .with_stderr_contains("[..]failed to read rustc info cache[..]")
            .with_stderr_contains(miss)
            .with_stderr_does_not_contain(hit)
            .with_stderr_contains(update),
    );

    assert_that(
        p.cargo("build").env("RUST_LOG", "cargo::util::rustc=info"),
        execs()
            .with_stderr_contains("[..]reusing existing rustc info cache[..]")
            .with_stderr_contains(hit)
            .with_stderr_does_not_contain(miss)
            .with_stderr_does_not_contain(update),
    );

    assert_that(
        p.cargo("build")
            .env("RUST_LOG", "cargo::util::rustc=info")
            .env("CARGO_CACHE_RUSTC_INFO", "0"),
        execs()
            .with_stderr_contains("[..]rustc info cache disabled[..]")
            .with_stderr_does_not_contain(update),
    );

    let other_rustc = {
        let p = project().at("compiler")
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
        assert_that(p.cargo("build"), execs());

        p.root()
            .join("target/debug/compiler")
            .with_extension(env::consts::EXE_EXTENSION)
    };

    assert_that(
        p.cargo("build")
            .env("RUST_LOG", "cargo::util::rustc=info")
            .env("RUSTC", other_rustc.display().to_string()),
        execs()
            .with_stderr_contains("[..]different compiler, creating new rustc info cache[..]")
            .with_stderr_contains(miss)
            .with_stderr_does_not_contain(hit)
            .with_stderr_contains(update),
    );

    assert_that(
        p.cargo("build")
            .env("RUST_LOG", "cargo::util::rustc=info")
            .env("RUSTC", other_rustc.display().to_string()),
        execs()
            .with_stderr_contains("[..]reusing existing rustc info cache[..]")
            .with_stderr_contains(hit)
            .with_stderr_does_not_contain(miss)
            .with_stderr_does_not_contain(update),
    );

    other_rustc.move_into_the_future();

    assert_that(
        p.cargo("build")
            .env("RUST_LOG", "cargo::util::rustc=info")
            .env("RUSTC", other_rustc.display().to_string()),
        execs()
            .with_stderr_contains("[..]different compiler, creating new rustc info cache[..]")
            .with_stderr_contains(miss)
            .with_stderr_does_not_contain(hit)
            .with_stderr_contains(update),
    );

    assert_that(
        p.cargo("build")
            .env("RUST_LOG", "cargo::util::rustc=info")
            .env("RUSTC", other_rustc.display().to_string()),
        execs()
            .with_stderr_contains("[..]reusing existing rustc info cache[..]")
            .with_stderr_contains(hit)
            .with_stderr_does_not_contain(miss)
            .with_stderr_does_not_contain(update),
    );
}
