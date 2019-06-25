use crate::support::{is_nightly, process, project};

#[cargo_test]
fn clippy() {
    if !is_nightly() {
        // --json-rendered is unstable
        eprintln!("skipping test: requires nightly");
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
fn fix_with_clippy() {
    if !is_nightly() {
        // fix --clippy is unstable
        eprintln!("skipping test: requires nightly");
        return;
    }

    if let Err(e) = process("clippy-driver").arg("-V").exec_with_output() {
        eprintln!("clippy-driver not available, skipping clippy test");
        eprintln!("{:?}", e);
        return;
    }

    let p = project()
        .file(
            "src/lib.rs",
            "
                pub fn foo() {
                    let mut v = Vec::<String>::new();
                    let _ = v.iter_mut().filter(|&ref a| a.is_empty());
                }
    ",
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.0.1 ([..])
[FIXING] src/lib.rs (1 fix)
[FINISHED] [..]
";

    p.cargo("fix -Zunstable-options --clippy --allow-no-vcs")
        .masquerade_as_nightly_cargo()
        .with_stderr(stderr)
        .with_stdout("")
        .run();
}
