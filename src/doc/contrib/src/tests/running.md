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
