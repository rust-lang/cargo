# Writing Tests

The following focuses on writing an integration test. However, writing unit
tests is also encouraged!

## Testsuite

Cargo has a wide variety of integration tests that execute the `cargo` binary
and verify its behavior, located in the [`testsuite`] directory.  The
[`support`] crate and [`snapbox`] contain many helpers to make this process easy.

There are two styles of tests that can roughly be categorized as
- functional tests
  - The fixture is programmatically defined
  - The assertions are regular string comparisons
  - Easier to share in an issue as a code block is completely self-contained
  - More resilient to insignificant changes though ui tests are easy to update when a change does occur
- ui tests
  - The fixture is file-based
  - The assertions use file-backed snapshots that can be updated with an env variable
  - Easier to review the expected behavior of the command as more details are included
  - Easier to get up and running from an existing project
  - Easier to reason about as everything is just files in the repo

These tests typically work by creating a temporary "project" with a
`Cargo.toml` file, executing the `cargo` binary process, and checking the
stdout and stderr output against the expected output.

### Functional Tests

Generally, a functional test will be placed in `tests/testsuite/<command>.rs` and will look roughly like:
```rust,ignore
#[cargo_test]
fn <description>() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hi!"); }"#)
        .build();

    p.cargo("run --bin foo")
        .with_stderr(
            "\
    [COMPILING] foo [..]
    [FINISHED] [..]
    [RUNNING] `target/debug/foo`
    ",
        )
        .with_stdout("hi!")
        .run();
    }
}
```

The [`#[cargo_test]` attribute](#cargo_test-attribute) is used in place of `#[test]` to inject some setup code.

[`ProjectBuilder`] via `project()`:
- Each project is in a separate directory in the sandbox
- If you do not specify a `Cargo.toml` manifest using `file()`, one is
  automatically created with a project name of `foo` using `basic_manifest()`.

[`Execs`] via `p.cargo(...)`:
- This executes the command and evaluates different assertions
  - See [`support::compare`] for an explanation of the string pattern matching.
    Patterns are used to make it easier to match against the expected output.

#### `#[cargo_test]` attribute

The `#[cargo_test]` attribute injects code which does some setup before starting the test.
It will create a filesystem "sandbox" under the "cargo integration test" directory for each test, such as `/path/to/cargo/target/tmp/cit/t123/`.
The sandbox will contain a `home` directory that will be used instead of your normal home directory.

The `#[cargo_test]` attribute takes several options that will affect how the test is generated.
They are listed in parentheses separated with commas, such as:

```rust,ignore
#[cargo_test(nightly, reason = "-Zfoo is unstable")]
```

The options it supports are:

* `nightly` --- This will cause the test to be ignored if not running on the nightly toolchain.
  This is useful for tests that use unstable options in `rustc` or `rustdoc`.
  These tests are run in Cargo's CI, but are disabled in rust-lang/rust's CI due to the difficulty of updating both repos simultaneously.
  A `reason` field is required to explain why it is nightly-only.
* `build_std_real` --- This is a "real" `-Zbuild-std` test (in the `build_std` integration test).
  This only runs on nightly, and only if the environment variable `CARGO_RUN_BUILD_STD_TESTS` is set (these tests on run on Linux).
* `build_std_mock` --- This is a "mock" `-Zbuild-std` test (which uses a mock standard library).
  This only runs on nightly, and is disabled for windows-gnu.
* `requires_` --- This indicates a command that is required to be installed to be run.
  For example, `requires_rustfmt` means the test will only run if the executable `rustfmt` is installed.
  These tests are *always* run on CI.
  This is mainly used to avoid requiring contributors from having every dependency installed.
* `>=1.64` --- This indicates that the test will only run with the given version of `rustc` or newer.
  This can be used when a new `rustc` feature has been stabilized that the test depends on.
  If this is specified, a `reason` is required to explain why it is being checked.
* `public_network_test` --- This tests contacts the public internet.
  These tests are disabled unless the `CARGO_PUBLIC_NETWORK_TESTS` environment variable is set.
  Use of this should be *extremely rare*, please avoid using it if possible.
  The hosts it contacts should have a relatively high confidence that they are reliable and stable (such as github.com), especially in CI.
  The tests should be carefully considered for developer security and privacy as well.
* `container_test` --- This indicates that it is a test that uses Docker.
  These tests are disabled unless the `CARGO_CONTAINER_TESTS` environment variable is set.
  This requires that you have Docker installed.
  The SSH tests also assume that you have OpenSSH installed.
  These should work on Linux, macOS, and Windows where possible.
  Unfortunately these tests are not run in CI for macOS or Windows (no Docker on macOS, and Windows does not support Linux images).
  See [`crates/cargo-test-support/src/containers.rs`](https://github.com/rust-lang/cargo/blob/master/crates/cargo-test-support/src/containers.rs) for more on writing these tests.
* `ignore_windows="reason"` --- Indicates that the test should be ignored on windows for the given reason.

#### Testing Nightly Features

If you are testing a Cargo feature that only works on "nightly" Cargo, then
you need to call `masquerade_as_nightly_cargo` on the process builder and pass 
the name of the feature as the reason, like this:

```rust,ignore
p.cargo("build").masquerade_as_nightly_cargo(&["print-im-a-teapot"])
```

If you are testing a feature that only works on *nightly rustc* (such as
benchmarks), then you should use the `nightly` option of the `cargo_test`
attribute, like this:

```rust,ignore
#[cargo_test(nightly, reason = "-Zfoo is unstable")]
```

This will cause the test to be ignored if not running on the nightly toolchain.

#### Specifying Dependencies

You should not write any tests that use the network such as contacting
crates.io. Typically, simple path dependencies are the easiest way to add a
dependency. Example:

```rust,ignore
let p = project()
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "1.0.0"

        [dependencies]
        bar = {path = "bar"}
    "#)
    .file("src/lib.rs", "extern crate bar;")
    .file("bar/Cargo.toml", &basic_manifest("bar", "1.0.0"))
    .file("bar/src/lib.rs", "")
    .build();
```

If you need to test with registry dependencies, see
[`support::registry::Package`] for creating packages you can depend on.

If you need to test git dependencies, see [`support::git`] to create a git
dependency.

#### Cross compilation

There are some utilities to help support tests that need to work against a
target other than the host. See [Running cross
tests](running.md#running-cross-tests) for more an introduction on cross
compilation tests.

Tests that need to do cross-compilation should include this at the top of the
test to disable it in scenarios where cross compilation isn't available:

```rust,ignore
if cargo_test_support::cross_compile::disabled() {
    return;
}
```

The name of the target can be fetched with the [`cross_compile::alternate()`]
function. The name of the host target can be fetched with
[`cargo_test_support::rustc_host()`].

The cross-tests need to distinguish between targets which can *build* versus
those which can actually *run* the resulting executable. Unfortunately, macOS is
currently unable to run an alternate target (Apple removed 32-bit support a
long time ago). For building, `x86_64-apple-darwin` will target
`x86_64-apple-ios` as its alternate. However, the iOS target can only execute
binaries if the iOS simulator is installed and configured. The simulator is
not available in CI, so all tests that need to run cross-compiled binaries are
disabled on CI. If you are running on macOS locally, and have the simulator
installed, then it should be able to run them.

If the test needs to run the cross-compiled binary, then it should have
something like this to exit the test before doing so:

```rust,ignore
if cargo_test_support::cross_compile::can_run_on_host() {
    return;
}
```

[`cross_compile::alternate()`]: https://github.com/rust-lang/cargo/blob/d58902e22e148426193cf3b8c4449fd3c05c0afd/crates/cargo-test-support/src/cross_compile.rs#L208-L225
[`cargo_test_support::rustc_host()`]: https://github.com/rust-lang/cargo/blob/d58902e22e148426193cf3b8c4449fd3c05c0afd/crates/cargo-test-support/src/lib.rs#L1137-L1140

### UI Tests

UI Tests are a bit more spread out and generally look like:

`tests/testsuite/<command>/mod.rs`:
```rust,ignore
mod <case>;
```

`tests/testsuite/<command>/<case>/mod.rs`:
```rust,ignore
use cargo_test_support::prelude::*;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::Project;
use cargo_test_support::curr_dir;

#[cargo_test]
fn case() {
    let project = Project::from_template(curr_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("run")
        .arg_line("--bin foo")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert_ui().subset_matches(curr_dir!().join("out"), &project_root);
}
```

Then populate
- `tests/testsuite/<command>/<case>/in` with the project's directory structure
- `tests/testsuite/<command>/<case>/out` with the files you want verified
- `tests/testsuite/<command>/<case>/stdout.log` with nothing
- `tests/testsuite/<command>/<case>/stderr.log` with nothing

`#[cargo_test]`:
- This is used in place of `#[test]`
- This attribute injects code which does some setup before starting the
  test, creating a filesystem "sandbox" under the "cargo integration test"
  directory for each test such as
  `/path/to/cargo/target/cit/t123/`
- The sandbox will contain a `home` directory that will be used instead of your normal home directory

`Project`:
- The project is copied from a directory in the repo
- Each project is in a separate directory in the sandbox

[`Command`] via `Command::cargo_ui()`:
- Set up and run a command.

[`OutputAssert`] via `Command::assert()`:
- Perform assertions on the result of the [`Command`]

[`Assert`] via `assert_ui()`:
- Verify the command modified the file system as expected

#### Updating Snapshots

The project, stdout, and stderr snapshots can be updated by running with the
`SNAPSHOTS=overwrite` environment variable, like:
```console
$ SNAPSHOTS=overwrite cargo test
```

Be sure to check the snapshots to make sure they make sense.

#### Testing Nightly Features

If you are testing a Cargo feature that only works on "nightly" Cargo, then
you need to call `masquerade_as_nightly_cargo` on the process builder and pass
the name of the feature as the reason, like this:

```rust,ignore
    snapbox::cmd::Command::cargo()
        .masquerade_as_nightly_cargo(&["print-im-a-teapot"])
```

If you are testing a feature that only works on *nightly rustc* (such as
benchmarks), then you should use the `nightly` option of the `cargo_test`
attribute, like this:

```rust,ignore
#[cargo_test(nightly, reason = "-Zfoo is unstable")]
```

This will cause the test to be ignored if not running on the nightly toolchain.

### Platform-specific Notes

When checking output, use `/` for paths even on Windows: the actual output
of `\` on Windows will be replaced with `/`.

Be careful when executing binaries on Windows. You should not rename, delete,
or overwrite a binary immediately after running it. Under some conditions
Windows will fail with errors like "directory not empty" or "failed to remove"
or "access is denied".

## Debugging tests

In some cases, you may need to dig into a test that is not working as you
expect, or you just generally want to experiment within the sandbox
environment. The general process is:

1. Build the sandbox for the test you want to investigate. For example:

   `cargo test --test testsuite -- features2::inactivate_targets`.
2. In another terminal, head into the sandbox directory to inspect the files and run `cargo` directly.
    1. The sandbox directories start with `t0` for the first test.

       `cd target/tmp/cit/t0`
    2. Set up the environment so that the sandbox configuration takes effect:

       `export CARGO_HOME=$(pwd)/home/.cargo`
    3. Most tests create a `foo` project, so head into that:

       `cd foo`
3. Run whatever cargo command you want. See [Running Cargo] for more details
   on running the correct `cargo` process. Some examples:

   * `/path/to/my/cargo/target/debug/cargo check`
   * Using a debugger like `lldb` or `gdb`:
        1. `lldb /path/to/my/cargo/target/debug/cargo`
        2. Set a breakpoint, for example: `b generate_root_units`
        3. Run with arguments: `r check`

[`testsuite`]: https://github.com/rust-lang/cargo/tree/master/tests/testsuite/
[`ProjectBuilder`]: https://github.com/rust-lang/cargo/blob/d847468768446168b596f721844193afaaf9d3f2/crates/cargo-test-support/src/lib.rs#L196-L202
[`Execs`]: https://github.com/rust-lang/cargo/blob/d847468768446168b596f721844193afaaf9d3f2/crates/cargo-test-support/src/lib.rs#L531-L550
[`support`]: https://github.com/rust-lang/cargo/blob/master/crates/cargo-test-support/src/lib.rs
[`support::compare`]: https://github.com/rust-lang/cargo/blob/master/crates/cargo-test-support/src/compare.rs
[`support::registry::Package`]: https://github.com/rust-lang/cargo/blob/d847468768446168b596f721844193afaaf9d3f2/crates/cargo-test-support/src/registry.rs#L311-L389
[`support::git`]: https://github.com/rust-lang/cargo/blob/master/crates/cargo-test-support/src/git.rs
[Running Cargo]: ../process/working-on-cargo.md#running-cargo
[`snapbox`]: https://docs.rs/snapbox/latest/snapbox/
[`Command`]: https://docs.rs/snapbox/latest/snapbox/cmd/struct.Command.html
[`OutputAssert`]: https://docs.rs/snapbox/latest/snapbox/cmd/struct.OutputAssert.html
[`Assert`]: https://docs.rs/snapbox/latest/snapbox/struct.Assert.html
