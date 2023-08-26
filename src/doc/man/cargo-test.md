# cargo-test(1)
{{~*set command="test"}}
{{~*set actionverb="Test"}}
{{~*set nouns="tests"}}
{{~*set multitarget=true}}

## NAME

cargo-test --- Execute unit and integration tests of a package

## SYNOPSIS

`cargo test` [_options_] [_testname_] [`--` _test-options_]

## DESCRIPTION

Compile and execute unit, integration, and documentation tests.

The test filtering argument `TESTNAME` and all the arguments following the two
dashes (`--`) are passed to the test binaries and thus to _libtest_ (rustc's
built in unit-test and micro-benchmarking framework).  If you're passing
arguments to both Cargo and the binary, the ones after `--` go to the binary,
the ones before go to Cargo.  For details about libtest's arguments see the
output of `cargo test -- --help` and check out the rustc book's chapter on
how tests work at <https://doc.rust-lang.org/rustc/tests/index.html>.

As an example, this will filter for tests with `foo` in their name and run them
on 3 threads in parallel:

    cargo test foo -- --test-threads 3

Tests are built with the `--test` option to `rustc` which creates a special
executable by linking your code with libtest. The executable automatically
runs all functions annotated with the `#[test]` attribute in multiple threads.
`#[bench]` annotated functions will also be run with one iteration to verify
that they are functional.

If the package contains multiple test targets, each target compiles to a
special executable as aforementioned, and then is run serially.

The libtest harness may be disabled by setting `harness = false` in the target
manifest settings, in which case your code will need to provide its own `main`
function to handle running tests.

### Documentation tests

Documentation tests are also run by default, which is handled by `rustdoc`. It
extracts code samples from documentation comments of the library target, and
then executes them.

Different from normal test targets, each code block compiles to a doctest
executable on the fly with `rustc`. These executables run in parallel in
separate processes. The compilation of a code block is in fact a part of test
function controlled by libtest, so some options such as `--jobs` might not
take effect. Note that this execution model of doctests is not guaranteed
and may change in the future; beware of depending on it.

See the [rustdoc book](https://doc.rust-lang.org/rustdoc/) for more information
on writing doc tests.

### Working directory of tests

The working directory when running each unit and integration test is set to the
root directory of the package the test belongs to.
Setting the working directory of tests to the package's root directory makes it
possible for tests to reliably access the package's files using relative paths,
regardless from where `cargo test` was executed from.

For documentation tests, the working directory when invoking `rustdoc` is set to
the workspace root directory, and is also the directory `rustdoc` uses as the
compilation directory of each documentation test.
The working directory when running each documentation test is set to the root
directory of the package the test belongs to, and is controlled via `rustdoc`'s
`--test-run-directory` option.

## OPTIONS

### Test Options

{{> options-test }}

{{> section-package-selection }}

### Target Selection

When no target selection options are given, `cargo test` will build the
following targets of the selected packages:

- lib --- used to link with binaries, examples, integration tests, and doc tests
- bins (only if integration tests are built and required features are
  available)
- examples --- to ensure they compile
- lib as a unit test
- bins as unit tests
- integration tests
- doc tests for the lib target

The default behavior can be changed by setting the `test` flag for the target
in the manifest settings. Setting examples to `test = true` will build and run
the example as a test, replacing the example's `main` function with the
libtest harness. If you don't want the `main` function replaced, also include
`harness = false`, in which case the example will be built and executed as-is.

Setting targets to `test = false` will stop them from being tested by default.
Target selection options that take a target by name (such as `--example foo`)
ignore the `test` flag and will always test the given target.

Doc tests for libraries may be disabled by setting `doctest = false` for the
library in the manifest.

See [Configuring a target](../reference/cargo-targets.html#configuring-a-target)
for more information on per-target settings.

{{> options-targets-bin-auto-built }}

{{> options-targets }}

{{#options}}

{{#option "`--doc`" }}
Test only the library's documentation. This cannot be mixed with other
target options.
{{/option}}

{{/options}}

{{> section-features }}

### Compilation Options

{{#options}}

{{> options-target-triple }}

{{> options-release }}

{{> options-profile }}

{{> options-ignore-rust-version }}

{{> options-timings }}

{{/options}}

### Output Options

{{#options}}
{{> options-target-dir }}
{{/options}}

### Display Options

By default the Rust test harness hides output from test execution to keep
results readable. Test output can be recovered (e.g., for debugging) by passing
`--nocapture` to the test binaries:

    cargo test -- --nocapture

{{#options}}

{{> options-display }}

{{> options-message-format }}

{{/options}}

### Manifest Options

{{#options}}

{{> options-manifest-path }}

{{> options-locked }}

{{/options}}

{{> section-options-common }}

### Miscellaneous Options

The `--jobs` argument affects the building of the test executable but does not
affect how many threads are used when running the tests. The Rust test harness
includes an option to control the number of threads used:

    cargo test -j 2 -- --test-threads=2

{{#options}}

{{> options-jobs }}
{{> options-future-incompat }}

{{/options}}

While `cargo test` involves compilation, it does not provide a `--keep-going`
flag. Use `--no-fail-fast` to run as many tests as possible without stopping at
the first failure. To "compile" as many tests as possible, use `--tests` to
build test binaries separately. For example:

    cargo build --tests --keep-going
    cargo test --tests --no-fail-fast

{{> section-environment }}

{{> section-exit-status }}

## EXAMPLES

1. Execute all the unit and integration tests of the current package:

       cargo test

2. Run only tests whose names match against a filter string:

       cargo test name_filter

3. Run only a specific test within a specific integration test:

       cargo test --test int_test_name -- modname::test_name

## SEE ALSO
{{man "cargo" 1}}, {{man "cargo-bench" 1}}, [types of tests](../reference/cargo-targets.html#tests), [how to write tests](https://doc.rust-lang.org/rustc/tests/index.html)
