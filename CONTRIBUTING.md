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


## Issue Triage

Sometimes, an issue will stay open, even though the bug has been fixed. And
sometimes, the original bug may go stale because something has changed in the
meantime.

It can be helpful to go through older bug reports and make sure that they are
still valid. Load up an older issue, double check that it's still true, and
leave a comment letting us know if it is or is not. The [least recently
updated sort][lru] is good for finding issues like this.

Contributors with sufficient permissions on the Rust repo can help by adding
labels to triage issues:

* Yellow, **A**-prefixed labels state which **area** of the project an issue
  relates to.

* Magenta, **B**-prefixed labels identify bugs which are **blockers**.

* Light purple, **C**-prefixed labels represent the **category** of an issue.

* Dark purple, **Command**-prefixed labels mean the issue has to do with a
  specific cargo command.

* Green, **E**-prefixed labels explain the level of **experience** or
  **effort** necessary to fix the issue.

* Red, **I**-prefixed labels indicate the **importance** of the issue. The
  [I-nominated][inom] label indicates that an issue has been nominated for
  prioritizing at the next triage meeting.

* Purple gray, **O**-prefixed labels are the **operating system** or platform
  that this issue is specific to.

* Orange, **P**-prefixed labels indicate a bug's **priority**. These labels
  are only assigned during triage meetings, and replace the [I-nominated][inom]
  label.

* The light orange **relnotes** label marks issues that should be documented in
  the release notes of the next release.

If you're looking for somewhere to start, check out the [E-easy][eeasy] tag.

[eeasy]: https://github.com/rust-lang/cargo/issues?q=is%3Aopen+is%3Aissue+label%3AE-easy
[lru]: https://github.com/rust-lang/cargo/issues?q=is%3Aissue+is%3Aopen+sort%3Aupdated-asc

## Getting help

If you need some pointers about Cargo's internals, feel free to ask questions
on the relevant issue or on the [IRC].

[IRC]: https://kiwiirc.com/client/irc.mozilla.org/cargo
