use crate::support::{is_nightly, process, project, registry::Package};
use std::path::Path;

fn as_str(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("valid utf-8")
}

#[cargo_test]
fn simple() {
    if !is_nightly() {
        // --json-rendered is unstable
        return;
    }
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

    let agnostic_path = Path::new("src").join("lib.rs");
    let agnostic_path_s = agnostic_path.to_str().unwrap();

    // Capture what rustc actually emits. This is done to avoid relying on the
    // exact message formatting in rustc.
    let rustc_output = process("rustc")
        .cwd(p.root())
        .args(&["--crate-type=lib", agnostic_path_s])
        .exec_with_output()
        .expect("rustc to run");

    assert!(rustc_output.stdout.is_empty());
    assert!(rustc_output.status.success());

    // -q so the output is the same as rustc (no "Compiling" or "Finished").
    let cargo_output1 = p
        .cargo("check -Zcache-messages -q --color=never")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("cargo to run");
    assert_eq!(as_str(&rustc_output.stderr), as_str(&cargo_output1.stderr));
    assert!(cargo_output1.stdout.is_empty());
    // Check that the cached version is exactly the same.
    let cargo_output2 = p
        .cargo("check -Zcache-messages -q")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("cargo to run");
    assert_eq!(as_str(&rustc_output.stderr), as_str(&cargo_output2.stderr));
    assert!(cargo_output2.stdout.is_empty());
}

#[cargo_test]
fn color() {
    if !is_nightly() {
        // --json-rendered is unstable
        return;
    }
    // Check enabling/disabling color.
    let p = project().file("src/lib.rs", "fn a() {}").build();

    let agnostic_path = Path::new("src").join("lib.rs");
    let agnostic_path_s = agnostic_path.to_str().unwrap();
    // Capture the original color output.
    let rustc_output = process("rustc")
        .cwd(p.root())
        .args(&["--crate-type=lib", agnostic_path_s, "--color=always"])
        .exec_with_output()
        .expect("rustc to run");
    assert!(rustc_output.status.success());
    let rustc_color = as_str(&rustc_output.stderr);
    assert!(rustc_color.contains("\x1b["));

    // Capture the original non-color output.
    let rustc_output = process("rustc")
        .cwd(p.root())
        .args(&["--crate-type=lib", agnostic_path_s])
        .exec_with_output()
        .expect("rustc to run");
    let rustc_nocolor = as_str(&rustc_output.stderr);
    assert!(!rustc_nocolor.contains("\x1b["));

    // First pass, non-cached, with color, should be the same.
    let cargo_output1 = p
        .cargo("check -Zcache-messages -q --color=always")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("cargo to run");
    assert_eq!(rustc_color, as_str(&cargo_output1.stderr));

    // Replay cached, with color.
    let cargo_output2 = p
        .cargo("check -Zcache-messages -q --color=always")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("cargo to run");
    assert_eq!(rustc_color, as_str(&cargo_output2.stderr));

    // Replay cached, no color.
    let cargo_output_nocolor = p
        .cargo("check -Zcache-messages -q --color=never")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("cargo to run");
    assert_eq!(rustc_nocolor, as_str(&cargo_output_nocolor.stderr));
}

#[cargo_test]
fn cached_as_json() {
    if !is_nightly() {
        // --json-rendered is unstable
        return;
    }
    // Check that cached JSON output is the same.
    let p = project().file("src/lib.rs", "fn a() {}").build();

    // Grab the non-cached output, feature disabled.
    // NOTE: When stabilizing, this will need to be redone.
    let cargo_output = p
        .cargo("check --message-format=json")
        .exec_with_output()
        .expect("cargo to run");
    assert!(cargo_output.status.success());
    let orig_cargo_out = as_str(&cargo_output.stdout);
    assert!(orig_cargo_out.contains("compiler-message"));
    p.cargo("clean").run();

    // Check JSON output, not fresh.
    let cargo_output1 = p
        .cargo("check -Zcache-messages --message-format=json")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("cargo to run");
    assert_eq!(as_str(&cargo_output1.stdout), orig_cargo_out);

    // Check JSON output, fresh.
    let cargo_output2 = p
        .cargo("check -Zcache-messages --message-format=json")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("cargo to run");
    // The only difference should be this field.
    let fix_fresh = as_str(&cargo_output2.stdout).replace("\"fresh\":true", "\"fresh\":false");
    assert_eq!(fix_fresh, orig_cargo_out);
}

#[cargo_test]
fn clears_cache_after_fix() {
    if !is_nightly() {
        // --json-rendered is unstable
        return;
    }
    // Make sure the cache is invalidated when there is no output.
    let p = project().file("src/lib.rs", "fn asdf() {}").build();
    // Fill the cache.
    p.cargo("check -Zcache-messages")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]asdf[..]")
        .run();
    let cpath = p
        .glob("target/debug/.fingerprint/foo-*/output")
        .next()
        .unwrap()
        .unwrap();
    assert!(std::fs::read_to_string(cpath).unwrap().contains("asdf"));

    // Fix it.
    p.change_file("src/lib.rs", "");

    p.cargo("check -Zcache-messages")
        .masquerade_as_nightly_cargo()
        .with_stdout("")
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] [..]
",
        )
        .run();
    assert_eq!(p.glob("target/debug/.fingerprint/foo-*/output").count(), 0);

    // And again, check the cache is correct.
    p.cargo("check -Zcache-messages")
        .masquerade_as_nightly_cargo()
        .with_stdout("")
        .with_stderr(
            "\
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn rustdoc() {
    if !is_nightly() {
        // --json-rendered is unstable
        return;
    }
    // Create a warning in rustdoc.
    let p = project()
        .file(
            "src/lib.rs",
            "
            #![warn(private_doc_tests)]
            /// asdf
            /// ```
            /// let x = 1;
            /// ```
            fn f() {}
            ",
        )
        .build();

    // At this time, rustdoc does not support --json-rendered=termcolor. So it
    // will always be uncolored with -Zcache-messages.
    let rustdoc_output = p
        .cargo("doc -Zcache-messages -q")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("rustdoc to run");
    assert!(rustdoc_output.status.success());
    let rustdoc_stderr = as_str(&rustdoc_output.stderr);
    assert!(rustdoc_stderr.contains("private"));
    // Invert this when --json-rendered is added.
    assert!(!rustdoc_stderr.contains("\x1b["));
    assert_eq!(p.glob("target/debug/.fingerprint/foo-*/output").count(), 1);

    // Check the cached output.
    let rustdoc_output = p
        .cargo("doc -Zcache-messages -q")
        .masquerade_as_nightly_cargo()
        .exec_with_output()
        .expect("rustdoc to run");
    assert_eq!(as_str(&rustdoc_output.stderr), rustdoc_stderr);
}

#[cargo_test]
fn clippy() {
    if !is_nightly() {
        // --json-rendered is unstable
        return;
    }
    if let Err(e) = process("clippy-driver").arg("-V").exec_with_output() {
        eprintln!("clippy-driver not available, skipping clippy test");
        eprintln!("{:?}", e);
        return;
    }

    // Caching clippy output.
    // This is just a random clippy lint (assertions_on_constants) that
    // hopefully won't change much in the future.
    let p = project()
        .file("src/lib.rs", "pub fn f() { assert!(true); }")
        .build();

    p.cargo("clippy-preview -Zunstable-options -Zcache-messages")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]assert!(true)[..]")
        .run();

    // Again, reading from the cache.
    p.cargo("clippy-preview -Zunstable-options -Zcache-messages")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]assert!(true)[..]")
        .run();

    // FIXME: Unfortunately clippy is sharing the same hash with check. This
    // causes the cache to be reused when it shouldn't.
    p.cargo("check -Zcache-messages")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]assert!(true)[..]") // This should not be here.
        .run();
}

#[cargo_test]
fn fix() {
    if !is_nightly() {
        // --json-rendered is unstable
        return;
    }
    // Make sure `fix` is not broken by caching.
    let p = project().file("src/lib.rs", "pub fn try() {}").build();

    p.cargo("fix --edition --allow-no-vcs -Zcache-messages")
        .masquerade_as_nightly_cargo()
        .run();

    assert_eq!(p.read_file("src/lib.rs"), "pub fn r#try() {}");
}

#[cargo_test]
fn very_verbose() {
    if !is_nightly() {
        // --json-rendered is unstable
        return;
    }
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

            [dependencies]
            bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcache-messages -vv")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]not_used[..]")
        .run();

    p.cargo("check -Zcache-messages")
        .masquerade_as_nightly_cargo()
        .with_stderr("[FINISHED] [..]")
        .run();

    p.cargo("check -Zcache-messages -vv")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]not_used[..]")
        .run();
}

#[cargo_test]
fn short_incompatible() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("check -Zcache-messages --message-format=short")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "[ERROR] currently `--message-format short` is incompatible with cached output",
        )
        .with_status(101)
        .run();
}
