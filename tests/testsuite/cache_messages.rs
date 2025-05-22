//! Tests for caching compiler diagnostics.

use cargo_test_support::prelude::*;
use cargo_test_support::str;
use cargo_test_support::tools;
use cargo_test_support::{basic_manifest, is_coarse_mtime, project, registry::Package, sleep_ms};

use super::messages::raw_rustc_output;

fn as_str(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("valid utf-8")
}

#[cargo_test]
fn simple() {
    // A simple example that generates two warnings (unused functions).
    let p = project()
        .file(
            "src/lib.rs",
            "
            fn a() {}
            fn b() {}
            ",
        )
        .build();

    // Capture what rustc actually emits. This is done to avoid relying on the
    // exact message formatting in rustc.
    let rustc_output = raw_rustc_output(&p, "src/lib.rs", &[]);

    // -q so the output is the same as rustc (no "Compiling" or "Finished").
    let cargo_output1 = p.cargo("check -q --color=never").run();
    assert_eq!(rustc_output, as_str(&cargo_output1.stderr));
    assert!(cargo_output1.stdout.is_empty());
    // Check that the cached version is exactly the same.
    let cargo_output2 = p.cargo("check -q").run();
    assert_eq!(rustc_output, as_str(&cargo_output2.stderr));
    assert!(cargo_output2.stdout.is_empty());
}

// same as `simple`, except everything is using the short format
#[cargo_test]
fn simple_short() {
    let p = project()
        .file(
            "src/lib.rs",
            "
                fn a() {}
                fn b() {}
            ",
        )
        .build();

    let rustc_output = raw_rustc_output(&p, "src/lib.rs", &["--error-format=short"]);

    let cargo_output1 = p
        .cargo("check -q --color=never --message-format=short")
        .run();
    assert_eq!(rustc_output, as_str(&cargo_output1.stderr));
    // assert!(cargo_output1.stdout.is_empty());
    let cargo_output2 = p.cargo("check -q --message-format=short").run();
    println!("{}", String::from_utf8_lossy(&cargo_output2.stdout));
    assert_eq!(rustc_output, as_str(&cargo_output2.stderr));
    assert!(cargo_output2.stdout.is_empty());
}

#[cargo_test]
fn color() {
    // Check enabling/disabling color.
    let p = project().file("src/lib.rs", "fn a() {}").build();

    // Hack for issue in fwdansi 1.1. It is squashing multiple resets
    // into a single reset.
    // https://github.com/kennytm/fwdansi/issues/2
    fn normalize(s: &str) -> String {
        #[cfg(windows)]
        return s.replace("\x1b[0m\x1b[0m", "\x1b[0m");
        #[cfg(not(windows))]
        return s.to_string();
    }

    let compare = |a, b| {
        assert_eq!(normalize(a), normalize(b));
    };

    // Capture the original color output.
    let rustc_color = raw_rustc_output(&p, "src/lib.rs", &["--color=always"]);
    assert!(rustc_color.contains("\x1b["));

    // Capture the original non-color output.
    let rustc_nocolor = raw_rustc_output(&p, "src/lib.rs", &[]);
    assert!(!rustc_nocolor.contains("\x1b["));

    // First pass, non-cached, with color, should be the same.
    let cargo_output1 = p.cargo("check -q --color=always").run();
    compare(&rustc_color, as_str(&cargo_output1.stderr));

    // Replay cached, with color.
    let cargo_output2 = p.cargo("check -q --color=always").run();
    compare(&rustc_color, as_str(&cargo_output2.stderr));

    // Replay cached, no color.
    let cargo_output_nocolor = p.cargo("check -q --color=never").run();
    compare(&rustc_nocolor, as_str(&cargo_output_nocolor.stderr));
}

#[cargo_test]
fn cached_as_json() {
    // Check that cached JSON output is the same.
    let p = project().file("src/lib.rs", "fn a() {}").build();

    // Grab the non-cached output, feature disabled.
    // NOTE: When stabilizing, this will need to be redone.
    let cargo_output = p.cargo("check --message-format=json").run();
    let orig_cargo_out = as_str(&cargo_output.stdout);
    assert!(orig_cargo_out.contains("compiler-message"));
    p.cargo("clean").run();

    // Check JSON output, not fresh.
    let cargo_output1 = p.cargo("check --message-format=json").run();
    assert_eq!(as_str(&cargo_output1.stdout), orig_cargo_out);

    // Check JSON output, fresh.
    let cargo_output2 = p.cargo("check --message-format=json").run();
    // The only difference should be this field.
    let fix_fresh = as_str(&cargo_output2.stdout).replace("\"fresh\":true", "\"fresh\":false");
    assert_eq!(fix_fresh, orig_cargo_out);
}

#[cargo_test]
fn clears_cache_after_fix() {
    // Make sure the cache is invalidated when there is no output.
    let p = project().file("src/lib.rs", "fn asdf() {}").build();
    // Fill the cache.
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] function `asdf` is never used
...
[WARNING] `foo` (lib) generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    let cpath = p
        .glob("target/debug/.fingerprint/foo-*/output-*")
        .next()
        .unwrap()
        .unwrap();
    assert!(std::fs::read_to_string(cpath).unwrap().contains("asdf"));

    // Fix it.
    if is_coarse_mtime() {
        sleep_ms(1000);
    }
    p.change_file("src/lib.rs", "");

    p.cargo("check")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_eq!(
        p.glob("target/debug/.fingerprint/foo-*/output-*").count(),
        0
    );

    // And again, check the cache is correct.
    p.cargo("check")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn rustdoc() {
    // Create a warning in rustdoc.
    let p = project()
        .file(
            "src/lib.rs",
            "
            #![warn(missing_docs)]
            pub fn f() {}
            ",
        )
        .build();

    let rustdoc_output = p.cargo("doc -q --color=always").run();
    let rustdoc_stderr = as_str(&rustdoc_output.stderr);
    assert!(rustdoc_stderr.contains("missing"));
    assert!(rustdoc_stderr.contains("\x1b["));
    assert_eq!(
        p.glob("target/debug/.fingerprint/foo-*/output-*").count(),
        1
    );

    // Check the cached output.
    let rustdoc_output = p.cargo("doc -q --color=always").run();
    assert_eq!(as_str(&rustdoc_output.stderr), rustdoc_stderr);
}

#[cargo_test]
fn fix() {
    // Make sure `fix` is not broken by caching.
    let p = project().file("src/lib.rs", "pub fn try() {}").build();

    p.cargo("fix --edition --allow-no-vcs").run();

    assert_eq!(p.read_file("src/lib.rs"), "pub fn r#try() {}");
}

#[cargo_test]
fn very_verbose() {
    // Handle cap-lints in dependencies.
    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "fn not_used() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -vv")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[RUNNING] [..]
[WARNING] function `not_used` is never used
...
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -vv")
        .with_stderr_data(str![[r#"
[FRESH] bar v1.0.0
[WARNING] function `not_used` is never used
...
[WARNING] `bar` (lib) generated 1 warning
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn doesnt_create_extra_files() {
    // Ensure it doesn't create `output` files when not needed.
    Package::new("dep", "1.0.0")
        .file("src/lib.rs", "fn unused() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                dep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check").run();

    assert_eq!(
        p.glob("target/debug/.fingerprint/foo-*/output-*").count(),
        0
    );
    assert_eq!(
        p.glob("target/debug/.fingerprint/dep-*/output-*").count(),
        0
    );
    if is_coarse_mtime() {
        sleep_ms(1000);
    }
    p.change_file("src/lib.rs", "fn unused() {}");
    p.cargo("check").run();
    assert_eq!(
        p.glob("target/debug/.fingerprint/foo-*/output-*").count(),
        1
    );
}

#[cargo_test]
fn replay_non_json() {
    // Handles non-json output.
    let rustc = project()
        .at("rustc")
        .file("Cargo.toml", &basic_manifest("rustc_alt", "1.0.0"))
        .file(
            "src/main.rs",
            r#"
            fn main() {
                eprintln!("line 1");
                eprintln!("line 2");
                let r = std::process::Command::new("rustc")
                    .args(std::env::args_os().skip(1))
                    .status();
                std::process::exit(r.unwrap().code().unwrap_or(2));
            }
            "#,
        )
        .build();
    rustc.cargo("build").run();
    let p = project().file("src/lib.rs", "").build();
    p.cargo("check")
        .env("RUSTC", rustc.bin("rustc_alt"))
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
line 1
line 2
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .env("RUSTC", rustc.bin("rustc_alt"))
        .with_stderr_data(str![[r#"
line 1
line 2
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn caching_large_output() {
    // Handles large number of messages.
    // This is an arbitrary amount that is greater than the 100 used in
    // job_queue. This is here to check for deadlocks or any other problems.
    const COUNT: usize = 250;
    let rustc = project()
        .at("rustc")
        .file("Cargo.toml", &basic_manifest("rustc_alt", "1.0.0"))
        .file(
            "src/main.rs",
            &format!(
                r#"
                fn main() {{
                    for i in 0..{} {{
                        eprintln!("{{{{\"message\": \"test message {{}}\", \"level\": \"warning\", \
                            \"spans\": [], \"children\": [], \"rendered\": \"test message {{}}\"}}}}",
                            i, i);
                    }}
                    let r = std::process::Command::new("rustc")
                        .args(std::env::args_os().skip(1))
                        .status();
                    std::process::exit(r.unwrap().code().unwrap_or(2));
                }}
                "#,
                COUNT
            ),
        )
        .build();

    let mut expected = String::new();
    for i in 0..COUNT {
        expected.push_str(&format!("test message {}\n", i));
    }

    rustc.cargo("build").run();
    let p = project().file("src/lib.rs", "").build();
    p.cargo("check")
        .env("RUSTC", rustc.bin("rustc_alt"))
        .with_stderr_data(&format!(
            "\
[CHECKING] foo v0.0.1 ([ROOT]/foo)
{}[WARNING] `foo` (lib) generated 250 warnings
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
",
            expected
        ))
        .run();

    p.cargo("check")
        .env("RUSTC", rustc.bin("rustc_alt"))
        .with_stderr_data(&format!(
            "\
{}[WARNING] `foo` (lib) generated 250 warnings
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
",
            expected
        ))
        .run();
}

#[cargo_test]
fn rustc_workspace_wrapper() {
    let p = project()
        .file(
            "src/lib.rs",
            "pub fn f() { assert!(true); }\n\
             fn unused_func() {}",
        )
        .build();

    p.cargo("check -v")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] [..]/rustc-echo-wrapper[EXE] rustc --crate-name foo [..]
WRAPPER CALLED: rustc --crate-name foo --edition=2015 src/lib.rs [..]
[WARNING] function `unused_func` is never used
...
[WARNING] `foo` (lib) generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Check without a wrapper should rebuild
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc[..]`
[WARNING] function `unused_func` is never used
...
[WARNING] `foo` (lib) generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
        .run();

    // Again, reading from the cache.
    p.cargo("check -v")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
WRAPPER CALLED: rustc [..]
...
[WARNING] `foo` (lib) generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
        .run();

    // And `check` should also be fresh, reading from cache.
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[WARNING] function `unused_func` is never used
...
[WARNING] `foo` (lib) generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
        .run();
}

#[cargo_test]
fn wacky_hashless_fingerprint() {
    // On Windows, executables don't have hashes. This checks for a bad
    // assumption that caused bad caching.
    let p = project()
        .file("src/bin/a.rs", "fn main() { let unused = 1; }")
        .file("src/bin/b.rs", "fn main() {}")
        .build();
    p.cargo("check --bin b")
        .with_stderr_does_not_contain("[..]unused[..]")
        .run();
    p.cargo("check --bin a")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] unused variable: `unused`
...
[WARNING] `foo` (bin "a") generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // This should not pick up the cache from `a`.
    p.cargo("check --bin b")
        .with_stderr_does_not_contain("[..]unused[..]")
        .run();
}
