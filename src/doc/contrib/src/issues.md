# Issue Tracker

Cargo's issue tracker is located at
<https://github.com/rust-lang/cargo/issues/>. This is the primary spot where
we track bugs and small feature requests. See [Process] for more about our
process for proposing changes.

## Filing issues

We can't fix what we don't know about, so please report problems liberally.
This includes problems with understanding the documentation, unhelpful error
messages, and unexpected behavior.

**If you think that you have identified an issue with Cargo that might
compromise its users' security, please do not open a public issue on GitHub.
Instead, we ask you to refer to Rust's [security policy].**

Opening an issue is as easy as following [this link][new-issues]. There are
several templates for different issue kinds, but if none of them fit your
issue, don't hesitate to modify one of the templates, or click the [Open a
blank issue] link.

The Rust tools are spread across multiple repositories in the Rust
organization. It may not always be clear where to file an issue. No worries!
If you file in the wrong tracker, someone will either transfer it to the
correct one or ask you to move it. Some other repositories that may be
relevant are:

* [`rust-lang/rust`] — Home for the [`rustc`] compiler and [`rustdoc`].
* [`rust-lang/rustup`] — Home for the [`rustup`] toolchain installer.
* [`rust-lang/rustfmt`] — Home for the `rustfmt` tool, which also includes `cargo fmt`.
* [`rust-lang/rust-clippy`] — Home for the `clippy` tool, which also includes `cargo clippy`.
* [`rust-lang/crates.io`] — Home for the [crates.io] website.

Issues with [`cargo fix`] can be tricky to know where they should be filed,
since the fixes are driven by `rustc`, processed by [`rustfix`], and the
front-interface is implemented in Cargo. Feel free to file in the Cargo issue
tracker, and it will get moved to one of the other issue trackers if
necessary.

[Process]: process/index.md
[security policy]: https://www.rust-lang.org/security.html
[new-issues]: https://github.com/rust-lang/cargo/issues/new/choose
[Open a blank issue]: https://github.com/rust-lang/cargo/issues/new
[`rust-lang/rust`]: https://github.com/rust-lang/rust
[`rust-lang/rustup`]: https://github.com/rust-lang/rustup
[`rust-lang/rustfmt`]: https://github.com/rust-lang/rustfmt
[`rust-lang/rust-clippy`]: https://github.com/rust-lang/rust-clippy
[`rustc`]: https://doc.rust-lang.org/rustc/
[`rustdoc`]: https://doc.rust-lang.org/rustdoc/
[`rustup`]: https://rust-lang.github.io/rustup/
[`rust-lang/crates.io`]: https://github.com/rust-lang/crates.io
[crates.io]: https://crates.io/
[`rustfix`]: https://github.com/rust-lang/rustfix/
[`cargo fix`]: https://doc.rust-lang.org/cargo/commands/cargo-fix.html

## Issue labels

[Issue labels] are very helpful to identify the types of issues and which
category they are related to. The Cargo team typically manages assigning
labels. The labels use a naming convention with short prefixes and colors to
indicate the kind of label:

* Yellow, **A**-prefixed labels state which **area** of the project an issue
  relates to.

* Light purple, **C**-prefixed labels represent the **category** of an issue.
  In particular, **[C-feature-request]** marks *proposals* for new features. If
  an issue is **C-feature-request**, but is not **[Feature accepted]** or
  **[I-nominated]**, then it was not thoroughly discussed, and might need some
  additional design or perhaps should be implemented as an external subcommand
  first. Ping @rust-lang/cargo if you want to send a PR for such issue.

* Dark purple, **Command**-prefixed labels mean the issue has to do with a
  specific cargo command.

* Green, **E**-prefixed labels indicate the level of **experience** or
  **effort** necessary to fix the issue. **[E-mentor]** issues also
  have some instructions on how to get started. Generally, all of the
  **E**-prefixed labels are issues that are ready for someone to contribute
  to!

* Red, **I**-prefixed labels indicate the **importance** of the issue. The
  **[I-nominated]** label indicates that an issue has been nominated for
  prioritizing at the next triage meeting.

* Purple gray, **O**-prefixed labels are the **operating system** or platform
  that this issue is specific to.

* Orange, **P**-prefixed labels indicate a bug's **priority**.

* **S**-prefixed labels are "status" labels, typically used for PRs, but can
  also indicate an issue is **[S-blocked]**.

* The light orange **[relnotes]** label marks issues that should be highlighted
  in the [Rust release notes] of the next release.

* Dark blue, **Z**-prefixed labels are for unstable, [nightly features].

[Issue labels]: https://github.com/rust-lang/cargo/labels
[E-easy]: https://github.com/rust-lang/cargo/labels/E-easy
[E-mentor]: https://github.com/rust-lang/cargo/labels/E-mentor
[I-nominated]: https://github.com/rust-lang/cargo/labels/I-nominated
[C-feature-request]: https://github.com/rust-lang/cargo/labels/C-feature-request
[Feature accepted]: https://github.com/rust-lang/cargo/labels/Feature%20accepted
[S-blocked]: https://github.com/rust-lang/cargo/labels/S-blocked
[Rust release notes]: https://github.com/rust-lang/rust/blob/master/RELEASES.md
[nightly features]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html
[relnotes]: https://github.com/rust-lang/cargo/issues?q=label%3Arelnotes
