Canonical home for these tests is https://github.com/rust-lang/rust/tree/master/tests/ui/frontmatter

To update
1. Sync changes to this directory
2. Run `SNAPSHOTS=overwrite cargo test --test testsuite -- script::rustc` to register new test cases
2. Run `SNAPSHOTS=overwrite cargo test --test testsuite -- script::rustc` to update snapshots for new test cases

Note:
- A `.stderr` file is assumed that the test fill fail
- A `.stdout` file is assumed that the test fill succeed
