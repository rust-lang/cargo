# Unstable features

Most new features should go through the unstable process. This means that the
feature will only be usable on the nightly channel, and requires a specific
opt-in by the user. Small changes can skip this process, but please consult
with the Cargo team first.

## Unstable feature opt-in

For features that require behavior changes or new syntax in `Cargo.toml`, then
it will need a `cargo-features` value placed at the top of `Cargo.toml` to
enable it. The process for doing adding a new feature is described in the
[`features` module]. Code that implements the feature will need to manually
check that the feature is enabled for the current manifest.

For features that add new command-line flags, config options, or environment
variables, then the `-Z` flags will be needed to enable them. The [`features`
module] also describes how to add these. New flags should use the
`fail_if_stable_opt` method to check if the `-Z unstable-options` flag has
been passed.

## Unstable documentation

Every unstable feature should have a section added to the [unstable chapter]
describing how to use the feature.

[unstable chapter]: https://github.com/rust-lang/cargo/blob/master/src/doc/src/reference/unstable.md

## Tracking issues

Each unstable feature should get a [tracking issue]. These issues are
typically created when a PR is close to being merged, or soon after it is
merged. Use the [tracking issue template] when creating a tracking issue.

Larger features should also get a new label in the issue tracker so that when
issues are filed, they can be easily tied together. Typically this would be
one of the `Z-` prefixed labels for nightly features.

When opening a tracking issue, be sure to also add an `S-` status label to
indicate what needs to happen for it to move forward:

* [S-needs-mentor] --- The feature isn't yet implemented, and needs a Cargo
  team member to commit to helping guide and review the implementation.
* [S-accepted] --- The feature isn't yet implemented, and has a Cargo team
  member willing to help review the implementation.
* [S-waiting-on-feedback] --- After the feature has been implemented, this
  label indicates that it is waiting on community feedback for bugs or design
  concerns.

Tracking issues may have multiple status labels if necessary, for example if
something is only partially implemented, it may have both
[S-waiting-on-feedback] (for what is implemented) and [S-needs-mentor] or
[S-accepted] to finish the rest of the work.

[tracking issue]: https://github.com/rust-lang/cargo/labels/C-tracking-issue
[tracking issue template]: https://github.com/rust-lang/cargo/issues/new?labels=C-tracking-issue&template=tracking_issue.md
[S-needs-mentor]: https://github.com/rust-lang/cargo/labels/S-needs-mentor
[S-accepted]: https://github.com/rust-lang/cargo/labels/S-accepted
[S-waiting-on-feedback]: https://github.com/rust-lang/cargo/labels/S-waiting-on-feedback

## Pre-Stabilization 

Once an unstable feature is "complete", the search for users to test
and give feedback begins. Testing notes should be written up to give users an
idea of how to test the new feature. An example being the 
[workspace inheritance testing notes] for workspace inheritance. Once testing
notes have been written up you should make posts in various rust communities
([rust subreddit], [users], [internals], etc). Example posts made for workspace
inheritance: [reddit post], [users post], [internals post]. The unstable feature
should also be added to [This Week in Rust]. This should be done by adding the
label `call-for-testing` to the RFC for the feature and making a comment with a
link to the testing notes and the tracking issue (as needed). If there is not an
RFC, a pull request should be made to the [TWiR repo] adding the feature to the
`Call for Testing` section ([example]).

[workspace inheritance testing notes]: https://github.com/rust-lang/cargo/blob/6d6dd9d9be9c91390da620adf43581619c2fa90e/src/doc/src/reference/unstable.md#testing-notes
[rust subreddit]: https://www.reddit.com/r/rust/
[users]: https://users.rust-lang.org/
[internals]: https://internals.rust-lang.org/
[reddit post]: https://www.reddit.com/r/rust/comments/uo8zeh/help_test_workspace_inheritance_in_preparation/
[users post]: https://users.rust-lang.org/t/help-test-workspace-inheritance-in-preparation-for-stablization/75582
[internals post]: https://internals.rust-lang.org/t/help-test-workspace-inheritance-in-preparation-for-stablization/16618
[This Week in Rust]: https://this-week-in-rust.org/
[TWiR repo]: https://github.com/rust-lang/this-week-in-rust
[example]: https://github.com/rust-lang/this-week-in-rust/pull/3256

## Stabilization

After some period of time, typically measured in months, the feature can be
considered to be stabilized. The feature should not have any significant known
bugs or issues, and any design concerns should be resolved.

The stabilization process depends on the kind of feature. For smaller
features, you can leave a comment on the tracking issue expressing interest in
stabilizing it. It can usually help to indicate that the feature has received
some real-world testing, and has exhibited some demand for broad use.

For larger features that have not gone through the [RFC process], then an RFC
to call for stabilization might be warranted. This gives the community a final
chance to provide feedback about the proposed design.

For a small feature, or one that has already gone through the RFC process, a
Cargo Team member may decide to call for a "final comment period" using
[rfcbot]. This is a public signal that a major change is being made, and gives
the Cargo Team members an opportunity to confirm or block the change. This
process can take a few days or weeks, or longer if a concern is raised.

Once the stabilization has been approved, the person who called for
stabilization should prepare a PR to stabilize the feature. This PR should:

* Flip the feature to stable in the [`features` module].
* Remove any unstable checks that aren't automatically handled by the feature
  system.
* Move the documentation from the [unstable chapter] into the appropriate
  places in the Cargo book and man pages.
* Remove the `-Z` flags and help message if applicable.
* Update all tests to remove nightly checks.
* Tag the PR with [relnotes] label if it seems important enough to highlight
  in the [Rust release notes].

[`features` module]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/features.rs
[RFC process]: https://github.com/rust-lang/rfcs/
[rfcbot]: https://github.com/rust-lang/rfcbot-rs
[Rust release notes]: https://github.com/rust-lang/rust/blob/master/RELEASES.md
[relnotes]: https://github.com/rust-lang/cargo/issues?q=label%3Arelnotes
