# Process

This chapter gives an overview of how Cargo comes together, and how you can be
a part of that process.

See the [Working on Cargo] chapter for an overview of the contribution
process.

[Working on Cargo]: working-on-cargo.md

## Cargo team

Cargo is managed by a [team] of volunteers. The Cargo Team reviews all
changes, and sets the direction for the project.

The team meets on a weekly basis on a video chat. If you are interested in
participating, feel free to contact us on [Zulip].

## Roadmap

The Cargo team typically establishes a roadmap each year that sets which areas
they will be focusing on. This is usually posted on the Inside Rust Blog (such
as [the 2020 roadmap]).

The [Roadmap Project Board] is used for tracking major initiatives. This gives
an overview of the things the team is interested in and thinking about.

The [RFC Project Board] is used for tracking [RFCs].

[the 2020 roadmap]: https://blog.rust-lang.org/inside-rust/2020/01/10/cargo-in-2020.html
[Roadmap Project Board]: https://github.com/rust-lang/cargo/projects/1
[RFC Project Board]: https://github.com/rust-lang/cargo/projects/2
[RFCs]: https://github.com/rust-lang/rfcs/

## Working on small bugs

Issues labeled with the [E-help-wanted], [E-easy], or [E-mentor] [labels] are
typically issues that the Cargo team wants to see addressed, and are
relatively easy to get started with. If you are interested in one of those,
and it has not already been assigned to someone, leave a comment. See [Issue
assignment](#issue-assignment) below for assigning yourself.

If there is a specific issue that you are interested in, but it doesn't have
one of the `E-` labels, leave a comment on the issue. If a Cargo team member
has the time to help out, they will respond to help with the next steps.

[E-help-wanted]: https://github.com/rust-lang/cargo/labels/E-help-wanted
[E-easy]: https://github.com/rust-lang/cargo/labels/E-easy
[E-mentor]: https://github.com/rust-lang/cargo/labels/E-mentor
[labels]: ../issues.md#issue-labels

## Working on large bugs

Some issues may be difficult to fix. They may require significant code
changes, or major design decisions. The [E-medium] and [E-hard] [labels] can
be used to tag such issues. These will typically involve some discussion with
the Cargo team on how to tackle it.

[E-medium]: https://github.com/rust-lang/cargo/labels/E-medium
[E-hard]: https://github.com/rust-lang/cargo/labels/E-hard

## Working on small features

Small feature requests are typically managed on the [issue
tracker][issue-feature-request]. Features that the Cargo team have approved
will have the [Feature accepted] label or the [E-mentor] label. If there is a
feature request that you are interested in, feel free to leave a comment
expressing your interest. If a Cargo team member has the time to help out,
they will respond to help with the next steps. Keep in mind that the Cargo
team has limited time, and may not be able to help with every feature request.
Most of them require some design work, which can be difficult. Check out the
[design principles chapter] for some guidance.

## Working on large features

Cargo follows the Rust model of evolution. Major features usually go through
an [RFC process]. Therefore, before opening a feature request issue create a
Pre-RFC thread on the [internals][irlo] forum to get preliminary feedback.
Implementing a feature as a [custom subcommand][subcommands] is encouraged as
it helps demonstrate the demand for the functionality and is a great way to
deliver a working solution faster as it can iterate outside of Cargo's release
cadence.

See the [unstable chapter] for how new major features are typically
implemented.

[unstable chapter]: unstable.md

## Bots and infrastructure

The Cargo project uses several bots:

* [GitHub Actions] are used to automatically run all tests for each PR.
* [rust-highfive] automatically assigns reviewers for PRs.
* [bors] is used to merge PRs. See [The merging process].
* [triagebot] is used for assigning issues to non-members, see [Issue
  assignment](#issue-assignment).
* [rfcbot] is used for making asynchronous decisions by team members.

[rust-highfive]: https://github.com/rust-highfive
[bors]: https://buildbot2.rust-lang.org/homu/
[The merging process]: working-on-cargo.md#the-merging-process
[GitHub Actions]: https://github.com/features/actions
[triagebot]: https://github.com/rust-lang/triagebot/wiki
[rfcbot]: https://github.com/rust-lang/rfcbot-rs

## Issue assignment

Normally, if you plan to work on an issue that has been marked with one of the
`E-` tags or [Feature accepted], it is sufficient just to leave a comment that
you are working on it. We also have a bot that allows you to formally "claim"
an issue by entering the text `@rustbot claim` in a comment. See the
[Assignment] docs on how this works.


[Assignment]: https://github.com/rust-lang/triagebot/wiki/Assignment
[team]: https://www.rust-lang.org/governance/teams/dev-tools#cargo
[Zulip]: https://rust-lang.zulipchat.com/#narrow/stream/246057-t-cargo
[issue-feature-request]: https://github.com/rust-lang/cargo/labels/C-feature-request
[Feature accepted]: https://github.com/rust-lang/cargo/labels/Feature%20accepted
[design principles chapter]: ../design.md
[RFC process]: https://github.com/rust-lang/rfcs/
[irlo]: https://internals.rust-lang.org/
[subcommands]: https://doc.rust-lang.org/cargo/reference/external-tools.html#custom-subcommands
