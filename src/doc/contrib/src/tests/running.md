# Running Tests

Using `cargo test` is usually sufficient for running the full test suite. This
can take a few minutes, so you may want to use more targeted flags to pick the
specific test you want to run, such as `cargo test --test testsuite
-- check::check_success`.

## Running nightly tests

Some tests only run on the nightly toolchain, and will be ignored on other
channels. It is recommended that you run tests with both nightly and stable to
ensure everything is working as expected.

Some of the nightly tests require the `rustc-dev` and `llvm-tools-preview`
rustup components installed. These components include the compiler as a
library. This may already be installed with your nightly toolchain, but if it
isn't, run `rustup component add rustc-dev llvm-tools-preview
--toolchain=nightly`.

## Running cross tests

Some tests exercise cross compiling to a different target. This will require
you to install the appropriate target. This typically is the 32-bit target of
your host platform. For example, if your host is a 64-bit
`x86_64-unknown-linux-gnu`, then you should install the 32-bit target with
`rustup target add i686-unknown-linux-gnu`. If you don't have the alternate
target installed, there should be an error message telling you what to do. You
may also need to install additional tools for the target. For example, on Ubuntu
you should install the `gcc-multilib` package.

If you can't install an alternate target, you can set the
`CFG_DISABLE_CROSS_TESTS=1` environment variable to disable these tests. The
Windows cross tests only support the MSVC toolchain.

## Running build-std tests

The `build-std` tests are disabled by default, but you can run them by setting
the `CARGO_RUN_BUILD_STD_TESTS=1` environment variable and running `cargo test
--test build-std`. This requires the nightly channel, and also requires the
`rust-src` component installed with `rustup component add rust-src
--toolchain=nightly`.

## Running with `gitoxide` as default git backend in tests

By default, the `git2` backend is used for most git operations. As tests need to explicitly
opt-in to use nightly features and feature flags, adjusting all tests to run with nightly
and `-Zgitoxide` is unfeasible.

This is why the private environment variable named `__CARGO_USE_GITOXIDE_INSTEAD_OF_GIT2` can be
set while running tests to automatically enable the `-Zgitoxide` flag implicitly, allowing to
test `gitoxide` for the entire cargo test suite.

## Running public network tests

Some (very rare) tests involve connecting to the public internet.
These tests are disabled by default,
but you can run them by setting the `CARGO_PUBLIC_NETWORK_TESTS=1` environment variable.
Additionally our CI suite has a smoke test for fetching dependencies.
For most contributors, you will never need to bother with this.

## Running container tests

Tests marked with `container_test` involve running Docker to test more complex configurations.
These tests are disabled by default,
but you can run them by setting the `CARGO_CONTAINER_TESTS=1` environment variable.
You will need to have Docker installed and running to use these.

> Note: Container tests mostly do not work on Windows.
> * The SSH tests require ssh-agent, but the two versions of ssh-agent
> on Windows are not suitable for testing.
>     * The Microsoft version of ssh-agent runs as a global service, and can't be isolated per test.
>     * The mingw/cygwin one can't be accessed from a Windows executable like cargo.
>     * Pageant similarly does not seem to have a way to isolate it (and I'm not certain it can be driven completely from the command-line).
>
> The tests also can't run on Windows CI because the Docker that is preinstalled doesn't support Linux containers, and setting up Windows containers is a pain.
>
> macOS should work with Docker installed and running,
> but unfortunately the tests are not run on CI because Docker is not available.
