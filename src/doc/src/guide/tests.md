# Tests

Cargo can run your tests with the `cargo test` command. Cargo looks for tests
to run in two places: in each of your `src` files and any tests in `tests/`.
Tests in your `src` files should be unit tests and [documentation tests].
Tests in `tests/` should be integration-style tests. As such, you’ll need to
import your crates into the files in `tests`.

Here's an example of running `cargo test` in our [package][def-package], which
currently has no tests:

```console
$ cargo test
   Compiling regex v1.5.0 (https://github.com/rust-lang/regex.git#9f9f693)
   Compiling hello_world v0.1.0 (file:///path/to/package/hello_world)
     Running target/test/hello_world-9c2b65bbb79eabce

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

If our package had tests, we would see more output with the correct number of
tests.

You can also run a specific test by passing a filter:

```console
$ cargo test foo
```

This will run any test with `foo` in its name.

`cargo test` runs additional checks as well. It will compile any examples
you’ve included to ensure they still compile. It also runs documentation
tests to ensure your code samples from documentation comments compile.
Please see the [testing guide][testing] in the Rust documentation for a general
view of writing and organizing tests. See [Cargo Targets: Tests] to learn more
about different styles of tests in Cargo.

[documentation tests]: ../../rustdoc/write-documentation/documentation-tests.html
[def-package]:  ../appendix/glossary.md#package  '"package" (glossary entry)'
[testing]: ../../book/ch11-00-testing.html
[Cargo Targets: Tests]: ../reference/cargo-targets.html#tests
