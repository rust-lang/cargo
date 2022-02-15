# Writing Tests

The following focuses on writing an integration test. However, writing unit
tests is also encouraged!

## Testsuite

Cargo has a wide variety of integration tests that execute the `cargo` binary
and verify its behavior, located in the [`testsuite`] directory. The
[`support`] crate contains many helpers to make this process easy.

These tests typically work by creating a temporary "project" with a
`Cargo.toml` file, executing the `cargo` binary process, and checking the
stdout and stderr output against the expected output.

### `cargo_test` attribute

Cargo's tests use the `#[cargo_test]` attribute instead of `#[test]`. This
attribute injects some code which does some setup before starting the test,
creating the little "sandbox" described below.

### Basic test structure

The general form of a test involves creating a "project", running `cargo`, and
checking the result. Projects are created with the [`ProjectBuilder`] where
you specify some files to create. The general form looks like this:

```rust,ignore
let p = project()
    .file("src/main.rs", r#"fn main() { println!("hi!"); }"#)
    .build();
```

The project creates a mini sandbox under the "cargo integration test"
directory with each test getting a separate directory such as
`/path/to/cargo/target/cit/t123/`. Each project appears as a separate
directory. There is also an empty `home` directory created that will be used
as a home directory instead of your normal home directory.

If you do not specify a `Cargo.toml` manifest using `file()`, one is
automatically created with a project name of `foo` using `basic_manifest()`.

To run Cargo, call the `cargo` method and make assertions on the execution:

```rust,ignore
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
```

This uses the [`Execs`] struct to build up a command to execute, along with
the expected output.

See [`support::compare`] for an explanation of the string pattern matching.
Patterns are used to make it easier to match against the expected output.

Browse the `pub` functions and modules in the [`support`] crate for a variety
of other helpful utilities.

### Testing Nightly Features

If you are testing a Cargo feature that only works on "nightly" Cargo, then
you need to call `masquerade_as_nightly_cargo` on the process builder like
this:

```rust,ignore
p.cargo("build").masquerade_as_nightly_cargo()
```

If you are testing a feature that only works on *nightly rustc* (such as
benchmarks), then you should exit the test if it is not running with nightly
rust, like this:

```rust,ignore
if !is_nightly() {
    // Add a comment here explaining why this is necessary.
    return;
}
```

### Platform-specific Notes

When checking output, use `/` for paths even on Windows: the actual output
of `\` on Windows will be replaced with `/`.

Be careful when executing binaries on Windows. You should not rename, delete,
or overwrite a binary immediately after running it. Under some conditions
Windows will fail with errors like "directory not empty" or "failed to remove"
or "access is denied".

### Specifying Dependencies

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
        2. Set a breakpoint, for example: `b generate_targets`
        3. Run with arguments: `r check`

[`testsuite`]: https://github.com/rust-lang/cargo/tree/master/tests/testsuite/
[`ProjectBuilder`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/crates/cargo-test-support/src/lib.rs#L225-L231
[`Execs`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/crates/cargo-test-support/src/lib.rs#L558-L579
[`support`]: https://github.com/rust-lang/cargo/blob/master/crates/cargo-test-support/src/lib.rs
[`support::compare`]: https://github.com/rust-lang/cargo/blob/master/crates/cargo-test-support/src/compare.rs
[`support::registry::Package`]: https://github.com/rust-lang/cargo/blob/e4b65bdc80f2a293447f2f6a808fa7c84bf9a357/crates/cargo-test-support/src/registry.rs#L73-L149
[`support::git`]: https://github.com/rust-lang/cargo/blob/master/crates/cargo-test-support/src/git.rs
[Running Cargo]: ../process/working-on-cargo.md#running-cargo
