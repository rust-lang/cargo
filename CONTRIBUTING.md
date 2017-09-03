# Contributing to Cargo

Thank you for your interest in contributing to Cargo! Good places to
start are this document, [ARCHITECTURE.md](ARCHITECTURE.md), which
describes high-level structure of Cargo and [E-easy] bugs on the
issue tracker.

As a reminder, all contributors are expected to follow our [Code of Conduct].

[E-easy]: https://github.com/rust-lang/cargo/labels/E-easy
[Code of Conduct]: https://www.rust-lang.org/conduct.html


## Running the tests

To run Cargo's tests, use `cargo test`. If you do not have the cross-compilers
installed locally, ignore the cross-compile test failures, or disable them by
using `CFG_DISABLE_CROSS_TESTS=1 cargo test`. Note that some tests are enabled
only on nightly toolchain.


## Contributing to the Docs

To contribute to the docs, all you need to do is change the markdown files in
the `src/doc` directory. To view the rendered version of changes you have
made locally, run:

```sh
sh src/ci/dox.sh
open target/doc/index.html
```


## Getting help

If you need some pointers about Cargo's internals, feel free to ask questions
on the relevant issue or on the [IRC].

[IRC]: https://kiwiirc.com/client/irc.mozilla.org/cargo
