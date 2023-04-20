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

* [`rust-lang/rust`] --- Home for the [`rustc`] compiler and [`rustdoc`].
* [`rust-lang/rustup`] --- Home for the [`rustup`] toolchain installer.
* [`rust-lang/rustfmt`] --- Home for the `rustfmt` tool, which also includes `cargo fmt`.
* [`rust-lang/rust-clippy`] --- Home for the `clippy` tool, which also includes `cargo clippy`.
* [`rust-lang/crates.io`] --- Home for the [crates.io] website.

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
category they are related to.

Anyone can apply most labels by posting comments with a form such as:

```text
@rustbot label: +A-doctests, -A-dependency-resolution
```

This example will add the [`A-doctests`] label and remove the
[`A-dependency-resolution`] label.

[Issue labels]: https://github.com/rust-lang/cargo/labels
[`A-doctests`]: https://github.com/rust-lang/cargo/labels/A-doctests
[`A-dependency-resolution`]: https://github.com/rust-lang/cargo/labels/A-dependency-resolution

The labels use a naming convention with short prefixes and colors to indicate
the kind of label:

<style>
.label-color {
    border-radius:0.5em;
}
table td:nth-child(2) {
    white-space: nowrap;
}

</style>

| Labels | Color | Description |
|--------|-------|-------------|
| [A-]   | <span class="label-color" style="background-color:#fbca04;">&#x2003;</span>&nbsp;Yellow | The **area** of the project an issue relates to. |
| [beta-] | <span class="label-color" style="background-color:#1e76d9;">&#x2003;</span>&nbsp;Dark Blue | Tracks changes which need to be [backported to beta][beta-backport] |
| [C-] | <span class="label-color" style="background-color:#f5f1fd;">&#x2003;</span>&nbsp;Light Purple | The **category** of an issue. |
| [Command-] | <span class="label-color" style="background-color:#5319e7;">&#x2003;</span>&nbsp;Dark Purple | The `cargo` command it is related to. |
| [E-] | <span class="label-color" style="background-color:#02e10c;">&#x2003;</span>&nbsp;Green | The **experience** level necessary to fix an issue. |
| [I-] | <span class="label-color" style="background-color:#fc2929;">&#x2003;</span>&nbsp;Red | The **importance** of the issue. |
| [O-] | <span class="label-color" style="background-color:#7e7ec8;">&#x2003;</span>&nbsp;Purple Grey | The **operating system** or platform that the issue is specific to. |
| [P-] | <span class="label-color" style="background-color:#eb6420;">&#x2003;</span>&nbsp;Orange | The issue **priority**. |
| [regression-] | <span class="label-color" style="background-color:#e4008a;">&#x2003;</span>&nbsp;Pink | Tracks regressions from a stable release. |
| [relnotes] | <span class="label-color" style="background-color:#fad8c7;">&#x2003;</span>&nbsp;Light Orange | Marks issues or PRs that should be highlighted in the [Rust release notes] of the next release. |
| [S-] | Varies | Tracks the **status** of issues and pull requests (see [Issue status labels](#issue-status-labels)) |
| [Z-] | <span class="label-color" style="background-color:#453574;">&#x2003;</span>&nbsp;Dark Blue | Unstable, [nightly features]. |


[A-]: https://github.com/rust-lang/cargo/labels?q=A
[beta-]: https://github.com/rust-lang/cargo/labels?q=beta
[beta-backport]: https://forge.rust-lang.org/release/backporting.html#beta-backporting-in-rust-langcargo
[C-]: https://github.com/rust-lang/cargo/labels?q=C
[Command-]: https://github.com/rust-lang/cargo/labels?q=Command
[E-]: https://github.com/rust-lang/cargo/labels?q=E
[I-]: https://github.com/rust-lang/cargo/labels?q=I
[nightly features]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html
[O-]: https://github.com/rust-lang/cargo/labels?q=O
[P-]: https://github.com/rust-lang/cargo/labels?q=P
[regression-]: https://github.com/rust-lang/cargo/labels?q=regression
[relnotes]: https://github.com/rust-lang/cargo/issues?q=label%3Arelnotes
[Rust release notes]: https://github.com/rust-lang/rust/blob/master/RELEASES.md
[S-]: https://github.com/rust-lang/cargo/labels?q=S
[Z-]: https://github.com/rust-lang/cargo/labels?q=nightly

### Issue status labels

The `S-` prefixed *status* labels are the primary mechanism we use to track
what is happening with an issue and what it is waiting on. The following is a
list of the status labels and what they mean. This is listed roughly in the
order that an issue might go through, though issues will often jump to
different steps, or in rare cases have multiple statuses.

* **[S-triage]** --- New issues get this label automatically assigned to them
  to indicate that nobody has yet looked at them, and they need someone to
  assign other labels and decide what the next step is.

* **[S-needs-info]** --- Needs more info, such as a reproduction or more
  background for a feature request.

  Anyone is welcome to help with providing additional info to help reproduce
  or provide more detail on use cases and such. But usually this is a request
  to the initial author.

  When adding this label, there should also usually be a comment that goes
  along with it stating the information requested.

* **[S-needs-team-input]** --- Needs input from team on whether/how to
  proceed.

  Here it is essentially blocked waiting for a team member to move it to the
  next stage.

* **[S-needs-design]** --- Needs someone to work further on the design for the
  feature or fix.

  Anyone is welcome to help at this stage, but it should be clear that it is
  not yet accepted. It is expected that people should contribute comments and
  ideas to the issue which furthers the process of fleshing out what is
  needed, or alternate ideas. This may also require reaching out to the wider
  community via forums and such.

* **[S-needs-rfc]** --- Needs an [RFC] before this can make more progress.

  Anyone is welcome to help at this stage, but it should be clear that it is
  not yet accepted. However, this should only be tagged for changes that are
  somewhat likely to be accepted.

* **[S-needs-mentor]** --- Needs a Cargo team member to commit to helping and
  reviewing.

  This is for something that is accepted, such as after an RFC or a team
  discussion, or an obvious issue that just needs fixing, but no team member
  is available to help or review.

* **[S-accepted]** --- Issue or feature is accepted, and has a team member
  available to help mentor or review.

* **[S-waiting-on-feedback]** --- An implemented feature is waiting on
  community feedback for bugs or design concerns.

  This is typically used on a [tracking issue] after it has been implemented
  to indicate what it is waiting on.


[S-triage]: https://github.com/rust-lang/cargo/labels/S-triage
[S-needs-info]: https://github.com/rust-lang/cargo/labels/S-needs-info
[S-needs-team-input]: https://github.com/rust-lang/cargo/labels/S-needs-team-input
[S-needs-design]: https://github.com/rust-lang/cargo/labels/S-needs-design
[S-needs-rfc]: https://github.com/rust-lang/cargo/labels/S-needs-rfc
[S-needs-mentor]: https://github.com/rust-lang/cargo/labels/S-needs-mentor
[S-accepted]: https://github.com/rust-lang/cargo/labels/S-accepted
[S-waiting-on-feedback]: https://github.com/rust-lang/cargo/labels/S-waiting-on-feedback
[RFC]: https://github.com/rust-lang/rfcs/
[tracking issue]: https://github.com/rust-lang/cargo/labels/C-tracking-issue

## Triaging issues

Triaging issues involves processing issues to assign appropriate labels, make
sure the issue has sufficient information, and to decide the next steps.
When new issues are filed, they should automatically get the [S-triage] label
assuming the author uses one of the templates. This helps identify which
issues have not yet been triaged.

There are several things to consider when triaging an issue:

* Is this a duplicate? Search the issue tracker (including closed issues) to
  see if there is something similar or identical to what is reported. If it is
  obviously a duplicate, write a comment that it is a duplicate of the other
  issue, and close the issue. If it isn't obvious that it is a duplicate,
  leave a comment asking the author if the other issue covers what they reported.

* For a bug, check if the report contains enough information to reproduce it.
  If you can't reproduce it, solicit more information from the author to
  better understand the issue.
  Change the label from [S-triage] to [S-needs-info] if this is the case.

* Add labels that describe what the issue is related to.

    * Add the appropriate [A-], [Command-], [O-], and [Z-] prefixed labels.
    * If this is a regression from stable, add one of the [regression-]
      prefixed labels (depending on if it is a regression in an already
      released stable release, or it is in nightly).

* Assuming the issue looks valid, remove the [S-triage] label and move it onto
  a new status:

  * [S-needs-rfc] --- This is a large feature request that will require a
    public design process.
  * [S-needs-design] --- The resolution of the issue or small feature request
    will need more work to come up with the appropriate design.
  * [S-needs-team-input] --- The next steps are not clear, and the Cargo team
    needs to discuss whether or not to proceed and what needs to be done to
    address the issue.
  * [S-needs-mentor] --- This is something the Cargo team wants to address,
    but does not currently have the capacity to help with reviewing.
  * [S-accepted] --- This is something that clearly needs to be addressed, and
    a Cargo team member has volunteered to help review.

Anyone is welcome to help with the triaging process. You can help with
reproducing issues, checking for duplicates, gathering more information from
the reporter, assigning labels using [`@rustbot` comments](#issue-labels), and
creating a test using [Cargo's testsuite] ([example][cargotest-example]).

[Cargo's testsuite]: tests/writing.md
[cargotest-example]: https://github.com/rust-lang/cargo/issues/11628#issuecomment-1411088951
