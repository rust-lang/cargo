# Working on Cargo

This chapter gives an overview of how to build Cargo, make a change, and
submit a Pull Request.

0. [Before hacking on Cargo.](#before-hacking-on-cargo)
1. [Check out the Cargo source.](#checkout-out-the-source)
2. [Building Cargo.](#building-cargo)
3. [Making a change.](#making-a-change)
4. [Writing and running tests.](../tests/index.md)
5. [Submitting a Pull Request.](#submitting-a-pull-request)
6. [The merging process.](#the-merging-process)

## Before hacking on Cargo

We encourage people to discuss their design before hacking on code. This gives
the Cargo team a chance to know your idea more. Sometimes after a discussion,
we even find a way to solve the problem without coding! Typically, you
[file an issue] or start a thread on the [internals forum] before submitting a
pull request.

Please read [the process] of how features and bugs are managed in Cargo.
**Only issues that have been explicitly marked as [accepted] will be reviewed.**

## Checkout the source

We use the "fork and pull" model [described here][development-models], where
contributors push changes to their personal fork and [create pull requests] to
bring those changes into the source repository. Cargo uses [git] and [GitHub]
for all development.

1. Fork the [`rust-lang/cargo`] repository on GitHub to your personal account
   (see [GitHub docs][how-to-fork]).
2. Clone your fork to your local machine using `git clone` (see [GitHub
   docs][how-to-clone])
3. It is recommended to start a new branch for the change you want to make.
   All Pull Requests are made against the master branch.

## Building Cargo

Cargo is built by...running `cargo`! There are a few prerequisites that you
need to have installed:

* `rustc` and `cargo` need to be installed. Cargo is expected to build and
  test with the current stable, beta, and nightly releases. It is your choice
  which to use. Nightly is recommended, since some nightly-specific tests are
  disabled when using the stable release. But using stable is fine if you
  aren't working on those.
* A C compiler (typically gcc, clang, or MSVC).
* [git]
* Unix:
    * pkg-config
    * OpenSSL (`libssl-dev` on Ubuntu, `openssl-devel` on Fedora)
* macOS:
    * OpenSSL ([homebrew] is recommended to install the `openssl` package)

If you can successfully run `cargo build`, you should be good to go!

[homebrew]: https://brew.sh/

## Running Cargo

You can use `cargo run` to run cargo itself, or you can use the path directly
to the cargo binary, such as `target/debug/cargo`.

If you are using [`rustup`], beware that running the binary directly can cause
issues with rustup overrides. Usually, when `cargo` is executed as part of
rustup, the toolchain becomes sticky (via an environment variable), and all
calls to `rustc` will use the same toolchain. But when `cargo` is not run via
rustup, the toolchain may change based on the directory. Since Cargo changes
the directory for each compilation, this can cause different calls to `rustc`
to use different versions. There are a few workarounds:

* Don't use rustup overrides.
* Use `rustup run <toolchain> target/debug/cargo` to specify the toolchain(rustc) to use.
  For example, `rustup run nightly target/debug/cargo`.
* Set the `RUSTC` environment variable to a specific `rustc` executable (not
  the rustup wrapper).
* Create a [custom toolchain]. This is a bit of a hack, but you can create a
  directory in the rustup `toolchains` directory, and create symlinks for all
  the files and directories in there to your toolchain of choice (such as
  nightly), except for the `cargo` binary, which you can symlink to your
  `target/debug/cargo` binary in your project directory.

*Normally*, all development is done by running Cargo's test suite, so running
it directly usually isn't required. But it can be useful for testing Cargo on
more complex projects.

[`rustup`]: https://rust-lang.github.io/rustup/
[custom toolchain]: https://rust-lang.github.io/rustup/concepts/toolchains.html#custom-toolchains

## Making a change

Some guidelines on working on a change:

* All code changes are expected to comply with the formatting suggested by
  `rustfmt`. You can use `rustup component add rustfmt` to install `rustfmt`
  and use `cargo fmt` to automatically format your code.
* Include tests that cover all non-trivial code. See the [Testing chapter] for
  more about writing and running tests.
* All code should be warning-free. This is checked during tests.

## Submitting a Pull Request

After you have committed your work, and pushed it to GitHub, you can
open a Pull Request

* Push your commits to GitHub and create a pull request against Cargo's
  `master` branch.
* Include a clear description of what the change is and why it is being made.
* Use [GitHub's keywords] in the description to automatically link to an issue
  if the PR resolves the issue. For example `Closes #1234` will link issue
  #1234 to the PR. When the PR is merged, GitHub will automatically close the
  issue.

[`@rustbot`] will automatically assign a reviewer for the PR. It
may take at least a few days for someone to respond. If you don't get a
response in over a week, feel free to ping the assigned reviewer.

When your PR is submitted, GitHub automatically runs all tests. The GitHub
interface will show a green checkmark if it passes, or a red X if it fails.
There are links to the logs on the PR page to diagnose any issues. The tests
typically finish in under 30 minutes.

The reviewer might point out changes deemed necessary. Large or tricky changes
may require several passes of review and changes.

> **tip:** Prefer atomic commits where each commit is a single, complete, and coherent unit of work.
> For example, if your feature work leads to renaming a module, make the rename its own commit.
> However, adding an internal function that is unused is not complete or coherent.
>
> As part of your atomic commits, prefer adding tests as their own commit *before* any functionality changes.
> The tests should pass in each commit, demonstrating the behavior before your
> change and how each commit affects behavior.
> This makes it easier for reviewers and community members to understand the
> precise details of the side effects of your change and gives you confidence
> that your tests are verifying the right behavior.
>
> Examples:
> - [#13910: fix: remove symlink dir on Windows](https://github.com/rust-lang/cargo/pull/13910)
> - [#14006: fix(add): Avoid escaping double-quotes by using string literals](https://github.com/rust-lang/cargo/pull/14006)

### Status labeling

PRs will get marked with [labels] like [`S-waiting-on-review`] or [`S-waiting-on-author`] to indicate their status.
The [`@rustbot`] bot can be used by anyone to adjust the labels.
If a PR gets marked as `S-waiting-on-author`, and you have pushed new changes that you would like to be reviewed, you can write a comment on the PR with the text `@rustbot ready`.
The bot will switch the labels on the PR.

More information about these commands can be found at the [shortcuts documentation].

[labels]: https://github.com/rust-lang/cargo/labels
[`S-waiting-on-review`]: https://github.com/rust-lang/cargo/labels/S-waiting-on-review
[`S-waiting-on-author`]: https://github.com/rust-lang/cargo/labels/S-waiting-on-author
[`@rustbot`]: https://github.com/rustbot
[shortcuts documentation]: https://forge.rust-lang.org/triagebot/shortcuts.html

## The merging process

After a reviewer has approved your PR,
they will add the PR to [GitHub merge queue].
The merge queue will create a temporary branch with your PR,
and run all required jobs.
If it fails, it will be removed from the queue.
The merge queue ensures that the master branch is always in a good state,
and that merges are processed one at a time.
The [merge queue dashboard] shows the current queued pull requests.

Assuming everything works, congratulations! It may take at least a week for
the changes to arrive on the nightly channel. See the [release chapter] for
more information on how Cargo releases are made.

[development-models]: https://help.github.com/articles/about-collaborative-development-models/
[create pull requests]: https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/creating-a-pull-request
[how-to-fork]: https://docs.github.com/en/github/getting-started-with-github/fork-a-repo
[`rust-lang/cargo`]: https://github.com/rust-lang/cargo/
[git]: https://git-scm.com/
[GitHub]: https://github.com/
[how-to-clone]: https://docs.github.com/en/github/creating-cloning-and-archiving-repositories/cloning-a-repository
[Testing chapter]: ../tests/index.md
[GitHub's keywords]: https://docs.github.com/en/github/managing-your-work-on-github/linking-a-pull-request-to-an-issue
[GitHub merge queue]: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
[merge queue dashboard]: https://github.com/rust-lang/cargo/queue/master
[release chapter]: release.md
[internals forum]: https://internals.rust-lang.org/c/tools-and-infrastructure/cargo
[file an issue]: https://github.com/rust-lang/cargo/issues
[the process]: index.md
[accepted]: https://github.com/rust-lang/cargo/issues?q=is%3Aissue+is%3Aopen+label%3AS-accepted
