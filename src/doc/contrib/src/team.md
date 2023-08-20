# Cargo Team

## Mission

The Cargo Team is a group of volunteers that support the Rust community in developing and maintaining Cargo, the Rust package manager and build tool.
The team is responsible for deciding how Cargo and its related libraries operate and evolve.
The team has a shared responsibility with the [crates.io team] for the design and usage of Cargo's index format and its registry API as it relates to the [crates.io] service.

The team is expected to keep Cargo in an operational state, to support Rust's 6-week release cycle, and to uphold the [Design Principles] of Cargo.

[crates.io team]: https://www.rust-lang.org/governance/teams/crates-io
[crates.io]: https://crates.io/
[Design Principles]: design.md

## Team membership

The Cargo Team consists of team members with one serving as a team leader.
The team leader is responsible for coordinating the team and providing a contact point with other teams.
The leader is selected by consensus of the existing members with no objections.

Membership is maintained in the [Rust team database].

[Rust team database]: https://github.com/rust-lang/team/blob/master/teams/cargo.toml

### Membership expectations

Team members are expected to participate in voting on RFCs and major change proposals

Team members are expected to regularly participate in at least some of the following membership-related activities.
Members are not expected to participate in all of these activities, but exhibit some interest and involvement in the project that covers some of these activities.

- Attending meetings
- Reviewing contributions (auto-assignment is managed in [triagebot.toml])
- Triaging and responding to issues
- Mentoring new contributors
- Shepherding major changes and RFCs
- Coordinating interaction with other Rust groups and outside interests
- Managing and updating the policies of the Cargo Team itself
- Keeping up with maintenance of the Cargo codebase, ensuring it stays functional and that infrastructure and team processes continue to run smoothly

Breaks and vacations are welcome and encouraged.
If a member is no longer participating after a few months, they may be asked to step down.

Members are required to always:

- Represent the Rust project in a way that upholds the [Rust code of conduct][coc] to a high standard.
- Represent the Cargo Team in a way that upholds the expectations of this charter, and be friendly, welcoming, and constructive with contributors and users.

Members are given privileges, such as:

- Merge permissions (bors rights)
- Issue and project management (GitHub permissions)
- Voting and decision making (RFCs, major changes)
- Access to private communications related to team management and security discussions

[coc]: https://www.rust-lang.org/policies/code-of-conduct
[triagebot.toml]: https://github.com/rust-lang/cargo/blob/master/triagebot.toml

### Meetings

The team meets on a weekly basis on a video chat.
If you are interested in participating, feel free to contact us on [Zulip].

### Becoming a member

A contributor can become a member of the Cargo Team by requesting a review or being nominated by one of the existing members.
They can be added by unanimous consent of the team.
The team lead or another member of the team will also confirm with the moderation team that there are no concerns involving the proposed team member.

Contributors who wish to join the team should exhibit an interest in carrying the design principles of Cargo and participate in some of the activities listed above in [Membership Expectations](#membership-expectations).

Members may leave at any time, preferably by letting the team know ahead of time.

## Decision process

The team uses a consensus-driven process for making decisions ranging from new features and major changes to management of the team itself.
The degree of process is correlated with the degree of change being proposed:

- Bug fixes, refactorings, documentation updates, and other small changes are usually delegated to a single team member (who is not the author) to approve based on their judgement.
  Team members may also solicit feedback from other members or the whole team for any change should they want to gather other perspectives from the team.

  Some examples of what this might cover are:
  - Bug fixes that do not introduce backwards-incompatible changes, and adhere to Cargo's expected behavior.
  - Addition of new warnings, or other diagnostic changes.
  - New or updated documentation.
  - Localized refactorings (that is, those that do not have a significant, wide-ranging impact to the usage and maintenance of the codebase).
  - Minor or planned changes to Cargo's console output.
  - Beta backports that clearly address a regression, and are expected to be low-risk.
  - Development of a previously approved unstable feature that matches the expected development of that feature.

- Small features or changes, large refactorings, or major changes to Cargo's codebase or process require an approval by the team via consensus.
  These decisions can be done via the FCP process of [rfcbot], or in an ad-hoc manner such as during a team meeting.
  rfcbot FCP requests do not require waiting for the 10-day feedback window if there is a complete team consensus, as this process is mostly aimed at polling the team rather than publicly soliciting feedback.
  Though, public feedback is welcome at any time.

  Some examples of what this might cover are:
  - Addition of a new, minor command-line argument, or an addition of an option to an existing one.
  - Addition of new fields and values to JSON outputs.
  - A bug fix or change that may technically involve a backwards-incompatible change.
    See the [Backwards compatibility] section for some examples.
  - Documentation changes that may substantially change the expected usage of Rust and Cargo.
    For example, the [SemVer chapter] contains subjective prescriptions for how users should develop their code.
  - A significant change in Cargo's console output.
  - A significant change to Cargo's code structure, or how maintenance or usage of the Cargo codebase is handled.
  - Beta backports that are risky or have any uncertainty about their necessity.
  - [Stable backports].
    These usually also require involvement with the Release team.
  - A significant change to the management of the Cargo team itself or the processes it uses, such as significant updates to this document.
  - Addition of new members to the Cargo team, or other actions involving the team membership.
    These decisions are usually processed via private channels by the entirety of the team.
  - A change that is a "one-way door".
    That is, something that is difficult to reverse without breaking backwards compatibility.

- Larger features should usually go through the [RFC process].
  This usually involves first soliciting feedback from the Cargo team and the rest of the community, often via the [Rust Internals] discussion board, [Cargo's issue tracker], and the [Zulip] channel.
  If there is positive feedback to the idea, the next step is to formally post an RFC on the RFC repo.
  The community and the Cargo team will have an opportunity to provide feedback on the proposal.
  After some period of time, the Cargo team may decide to either accept, postpone, or close a proposal based on the interest in the proposal and the team's availability.

  Some examples of what this might cover are:
  - Major changes or new features or options in `Cargo.toml` or the config files.
  - Changes to the registry index or API.
  - New or changed CLI options that are expected to have a significant impact on how Cargo is used.
  - New `cargo` commands that are not trivial.
    In some cases, the team may decide to adopt a pre-existing external command without an RFC if the command has already been broadly adopted.

- Stabilization of [Unstable] features requires an approval via the FCP process of [rfcbot].
  This provides a final opportunity to solicit feedback from the public, and for the Cargo team to agree via consensus.

- The team may decide to experiment with larger features without starting the RFC process if it is an initiative that the team has consensus that it is something they want to pursue.
  This is usually reserved for something that has an unclear path that the RFC process is not expected to provide feedback that would substantially move the process forward.
  Such experiments are expected to be nightly-only (see the [Unstable] chapter), and involve efforts to shape the final result via exploration, testing, and public involvement.
  Any such features *must* ultimately have an RFC approved before they can be stabilized.

[rfcbot]: https://github.com/rust-lang/rfcbot-rs
[RFC process]: https://github.com/rust-lang/rfcs/
[Rust Internals]: https://internals.rust-lang.org/
[Unstable]: process/unstable.md
[Backwards compatibility]: design.md#backwards-compatibility
[Stable backports]: process/release.md#stable-backports
[SemVer chapter]: https://doc.rust-lang.org/cargo/reference/semver.html

## Contacting the team

The team may be contacted through several channels:

- If you have a **security concern**, please refer to Rust's [security policy] for the correct contact method.
- Issues and feature requests can be submitted to [Cargo's issue tracker].
  Please see the [Issues chapter] for more detail.
- The [`t-cargo` Zulip channel][Zulip] stream is the chat platform the Cargo Team uses to coordinate on.
- The <cargo@rust-lang.org> email address can be used to contact the team.
  However, using one of the other channels is strongly encouraged.

[Zulip]: https://rust-lang.zulipchat.com/#narrow/stream/246057-t-cargo
[security policy]: https://www.rust-lang.org/security.html
[Cargo's issue tracker]: https://github.com/rust-lang/cargo/issues/
[Issues chapter]: issues.md
