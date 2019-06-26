use crate::support::{clippy_is_available, is_nightly, project};

#[cargo_test]
fn clippy() {
    if !is_nightly() {
        // --json-rendered is unstable
        eprintln!("skipping test: requires nightly");
        return;
    }

    if !clippy_is_available() {
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
