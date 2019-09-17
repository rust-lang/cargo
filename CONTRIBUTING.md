# Contributing to Cargo

Thank you for your interest in contributing to Cargo! Good places to
start are this document, [ARCHITECTURE.md](ARCHITECTURE.md), which
describes the high-level structure of Cargo and [E-easy] bugs on the
issue tracker.

If you have a general question about Cargo or it's internals, feel free to ask
on [Discord].

## Code of Conduct

All contributors are expected to follow our [Code of Conduct].

## Bug reports

We can't fix what we don't know about, so please report problems liberally. This
includes problems with understanding the documentation, unhelpful error messages
and unexpected behavior.

**If you think that you have identified an issue with Cargo that might compromise
its users' security, please do not open a public issue on GitHub. Instead,
we ask you to refer to Rust's [security policy].**

Opening an issue is as easy as following [this link][new-issues] and filling out
the fields. Here's a template that you can use to file an issue, though it's not
necessary to use it exactly:

    <short summary of the problem>

    I tried this: <minimal example that causes the problem>

    I expected to see this happen: <explanation>

    Instead, this happened: <explanation>

    I'm using <output of `cargo --version`>

All three components are important: what you did, what you expected, what
happened instead. Please use https://gist.github.com/ if your examples run long.


## Feature requests

Cargo follows the general Rust model of evolution. All major features go through
an RFC process. Therefore, before opening a feature request issue create a
Pre-RFC thread on the [internals][irlo] forum to get preliminary feedback.
Implementing a feature as a [custom subcommand][subcommands] is encouraged as it
helps demonstrate the demand for the functionality and is a great way to deliver
a working solution faster as it can iterate outside of cargo's release cadence.

## Working on issues

If you're looking for somewhere to start, check out the [E-easy][E-Easy] and
[E-mentor][E-mentor] tags.

Feel free to ask for guidelines on how to tackle a problem on [Discord] or open a
[new issue][new-issues]. This is especially important if you want to add new
features to Cargo or make large changes to the already existing code-base.
Cargo's core developers will do their best to provide help.

If you start working on an already-filed issue, post a comment on this issue to
let people know that somebody is working it. Feel free to ask for comments if
you are unsure about the solution you would like to submit.

While Cargo does make use of some Rust-features available only through the
`nightly` toolchain, it must compile on stable Rust. Code added to Cargo
is encouraged to make use of the latest stable features of the language and
`stdlib`.

We use the "fork and pull" model [described here][development-models], where
contributors push changes to their personal fork and create pull requests to
bring those changes into the source repository. This process is partly
automated: Pull requests are made against Cargo's master-branch, tested and
reviewed. Once a change is approved to be merged, a friendly bot merges the
changes into an internal branch, runs the full test-suite on that branch
and only then merges into master. This ensures that Cargo's master branch
passes the test-suite at all times.

Your basic steps to get going:

* Fork Cargo and create a branch from master for the issue you are working on.
* Please adhere to the code style that you see around the location you are
working on.
* [Commit as you go][githelp].
* Include tests that cover all non-trivial code. The existing tests
in `test/` provide templates on how to test Cargo's behavior in a
sandbox-environment. The internal module `testsuite/support` provides a vast amount
of helpers to minimize boilerplate. See [`testsuite/support/mod.rs`] for an
introduction to writing tests.
* Make sure `cargo test` passes. If you do not have the cross-compilers
installed locally, install them using the instructions returned by
`cargo test cross_compile::cross_tests` (twice, with `--toolchain nightly`
added to get the nightly cross target too); alternatively just
ignore the cross-compile test failures or disable them by
using `CFG_DISABLE_CROSS_TESTS=1 cargo test`. Note that some tests are enabled
only on `nightly` toolchain. If you can, test both toolchains.
* All code changes are expected to comply with the formatting suggested by `rustfmt`.
You can use `rustup component add --toolchain nightly rustfmt` to install `rustfmt` and use
`rustfmt +nightly --unstable-features --skip-children` on the changed files to automatically format your code.
* Push your commits to GitHub and create a pull request against Cargo's
`master` branch.

## Pull requests

After the pull request is made, a friendly bot will automatically assign a
reviewer; the review-process will make sure that the proposed changes are
sound. Please give the assigned reviewer sufficient time, especially during
weekends. If you don't get a reply, you may poke the core developers on [Discord].

A merge of Cargo's master-branch and your changes is immediately queued
to be tested after the pull request is made. In case unforeseen
problems are discovered during this step (e.g., a failure on a platform you
originally did not develop on), you may ask for guidance. Push additional
commits to your branch to tackle these problems.

The reviewer might point out changes deemed necessary. Please add them as
extra commits; this ensures that the reviewer can see what has changed since
the code was previously reviewed. Large or tricky changes may require several
passes of review and changes.

Once the reviewer approves your pull request, a friendly bot picks it up
and [merges][mergequeue] it into Cargo's `master` branch.

## Contributing to the documentation

To contribute to the documentation, all you need to do is change the markdown
files in the `src/doc` directory. To view the rendered version of changes you
have made locally, make sure you have `mdbook` installed and run:

```sh
cd src/doc
mdbook build
open book/index.html
```

To install `mdbook` run `cargo install mdbook`.


## Issue Triage

Sometimes an issue will stay open, even though the bug has been fixed. And
sometimes, the original bug may go stale because something has changed in the
meantime.

It can be helpful to go through older bug reports and make sure that they are
still valid. Load up an older issue, double check that it's still true, and
leave a comment letting us know if it is or is not. The [least recently
updated sort][lru] is good for finding issues like this.

Contributors with sufficient permissions on the Rust-repository can help by
adding labels to triage issues:

* Yellow, **A**-prefixed labels state which **area** of the project an issue
  relates to.

* Magenta, **B**-prefixed labels identify bugs which are **blockers**.

* Light purple, **C**-prefixed labels represent the **category** of an issue.
  In particular, **C-feature-request** marks *proposals* for new features. If
  an issue is **C-feature-request**, but is not **Feature accepted** or **I-nominated**,
  then it was not thoroughly discussed, and might need some additional design
  or perhaps should be implemented as an external subcommand first. Ping
  @rust-lang/cargo if you want to send a PR for such issue.

* Dark purple, **Command**-prefixed labels mean the issue has to do with a
  specific cargo command.

* Green, **E**-prefixed labels explain the level of **experience** or
  **effort** necessary to fix the issue. [**E-mentor**][E-mentor] issues also
  have some instructions on how to get started.

* Red, **I**-prefixed labels indicate the **importance** of the issue. The
  **[I-nominated][]** label indicates that an issue has been nominated for
  prioritizing at the next triage meeting.

* Purple gray, **O**-prefixed labels are the **operating system** or platform
  that this issue is specific to.

* Orange, **P**-prefixed labels indicate a bug's **priority**. These labels
  are only assigned during triage meetings and replace the **[I-nominated][]**
  label.

* The light orange **relnotes** label marks issues that should be documented in
  the release notes of the next release.


[githelp]: https://dont-be-afraid-to-commit.readthedocs.io/en/latest/git/commandlinegit.html
[development-models]: https://help.github.com/articles/about-collaborative-development-models/
[gist]: https://gist.github.com/
[new-issues]: https://github.com/rust-lang/cargo/issues/new
[mergequeue]: https://buildbot2.rust-lang.org/homu/queue/cargo
[security policy]: https://www.rust-lang.org/security.html
[lru]: https://github.com/rust-lang/cargo/issues?q=is%3Aissue+is%3Aopen+sort%3Aupdated-asc
[E-easy]: https://github.com/rust-lang/cargo/labels/E-easy
[E-mentor]: https://github.com/rust-lang/cargo/labels/E-mentor
[I-nominated]: https://github.com/rust-lang/cargo/labels/I-nominated
[Code of Conduct]: https://www.rust-lang.org/conduct.html
[Discord]: https://discordapp.com/invite/rust-lang
[`testsuite/support/mod.rs`]: https://github.com/rust-lang/cargo/blob/master/tests/testsuite/support/mod.rs
[irlo]: https://internals.rust-lang.org/
[subcommands]: https://doc.rust-lang.org/cargo/reference/external-tools.html#custom-subcommands
