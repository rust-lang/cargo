# Security issues

Issues involving reporting a security vulnerability in cargo usually start by following the [Rust security policy].
The Security Response Working Group ("the WG") is responsible for running the process of handling the response to a security issue.
Their process is documented at [Handling Reports].
This document gives an overview of the process from a Cargo team member's perspective.

The general order of events happens as follows:

1. The "reporter" (even if it is a Cargo team member) reports an issue to <security@rust-lang.org>.
1. The WG will evaluate if the report is credible, and manages responses to the reporter.
1. The WG will start a private Zulip stream to coordinate discussion and plans for a fix.
1. The WG will pull in one or more team members into the Zulip stream ("responders").
    - Security vulnerabilities are **embargoed** until they are released publicly.
      People who are brought into these discussions should **not** discuss the issue with *anyone* outside of the group, including your employer, without first consulting The WG.
1. A discussion then starts to evaluate the severity of the issue and what possible solutions should be considered.
   This includes figuring out who will volunteer to actually develop the patches to resolve the issue, and who will review it.
1. The WG will create a temporary private fork of the `rust-lang/cargo` repo using GitHub's [repository security advisory][github-advisory] system.
   This provides a space where changes can be securely posted, and the security advisory can be drafted.
   See ["Collaborating in a temporary private fork"][private-fork] for some screenshots of what this looks like.
   GitHub includes instructions on how to work with the fork.

   Beware that the private fork has some limitations, such as not supporting CI, or (for some weird reason) not supporting syntax highlighting.
1. Someone will need to review the patches and make sure everyone agrees on the solution.
   This may also involve the WG conferring with the reporter to validate the fix.
1. Create a rollout plan.
   This includes deciding if there will be a new patch release of Rust, or if it should wait for the next stable release, or whether to remove the embargo on the fix.
1. The WG will handle drafting a Security Advisory using GitHub's Security Advisory ("GHSA") system.
   [GHSA-r5w3-xm58-jv6j] is an example of what this looks like.
   This process also involves reserving a [CVE](https://www.cve.org/) number, where the report will eventually be posted.

   The responders should carefully review the report to make sure it is correct.

   This process may also involve deciding on the CVSS score.
   There are a bunch of calculators on the web where you can see how this works (such as the [FIRST CVSS Calculator][calc], or you can view GitHub's calculator by drafting a security advisory in one of your personal repos).
   FIRST has a [user guide][first-guide] for deciding how to score each characteristic.
1. If it is decided to do a patch release of Rust, the general overview of steps is:
    1. Finalizing the patches.
       This includes all the little details like updating changelogs, version numbers, and such.
    1. Preparing PRs in the private fork against the stable, beta, and nightly (master) branches.
    1. The WG handles creating a private fork of `rust-lang/rust` to prepare the point release.
       This usually includes changes for stable, beta, and nightly.
    1. The WG handles posting patches in various places (such as mailing lists), possibly several days in advance.
    1. The WG handles posting public PRs to `rust-lang/rust` to incorporate the fix and prepare a new release.
    1. The WG handles announcing everything, including publishing the GHSA, publishing a blog post, and several other places.

## External dependency patches

Sometimes it may be necessary to make changes to external dependencies to support a fix.
This can make things complicated.
If the change is by itself benign and not directly related to the security issue,
then it may be safe to publicly propose the change (but not giving context) and try to get a new release of the dependency made (though confer with the WG first!).
However, if the issue is directly related to the dependency, then it becomes significantly more awkward.

The general process for [GHSA-r5w3-xm58-jv6j] which involved a fix in `git2-rs` was handled by the responders because it is a dependency owned by the rust-lang org.
The general outline of how we managed this is:

- Pre-release:
    1. Created a private fork of `rust-lang/git2-rs` just like we did for `rust-lang/cargo`.
       git2-rs also had its own Security Advisory just like cargo did.
    1. Created and reviewed PRs in the private fork for the fixes.
        - The PRs in the `rust-lang/cargo` private fork had to have a temporary `[patch]` git dependency on the `git2-rs` private fork.
    1. Before the release, the PRs were changed to remove the `[patch]`, and pretend as-if git2-rs had already been published.
- Showtime:
    1. The git2-rs changes were publicly merged, and a new release was published to crates.io.
    1. The cargo PR was merged to cargo's stable branch.
    1. The private rust-lang/rust PR updated the cargo submodule and updated `Cargo.lock` to pick up the new git2 dependencies.
    1. Release proceeds as normal (publish both GHSA, create release, etc.).
- Post-release:
    1. Various forward ports were created in git2-rs, and new releases were made.

If the change is in a crate not managed by any responder, then confer with the WG on a strategy.
One option is to create a temporary fork used for the security response that will be removed as soon as the security advisory is released and a new public release of the dependency is made with the fix.

## Checklist

There are a lot of details to handle, and it can be a bit of a challenge under time pressure.
The following is a checklist of some items to pay attention to during the process.

Pre-release:
- [ ] Check for any SemVer-incompatible changes in the public API of any crates that are modified.
  - Try to avoid these if at all possible.
    Although not a severe problem, making Cargo's version number drift farther from Rust's can contribute to confusion.
  - If a SemVer-breaking release is made to a dependency, make sure this is coordinated correctly between the stable, beta, and master branches.
- [ ] With a checkout of the proposed fixes, run as much of cargo's CI testsuite locally as you can.
  Since private forks don't support CI, the responders will be responsible for making sure all tests pass.
  Enlist other responders if you don't have the necessary systems like Windows.
- [ ] Manually exercise the fix locally.
  Since we will essentially have *no* nightly testing, the responders are responsible for making sure things work.
  Try to consider all the different environments users may be using.
- [ ] Make sure any comments or docs that need updating get updated.
- [ ] Review the git commit messages of the patch.
  Make sure they clearly and accurately reflect what is being changed and why.
  Clean up the commit history if it goes through several revisions during review.
- [ ] Make sure that the *public* cargo repo's stable and beta branches are in a state where they are passing CI.
  This may require backporting changes that fix problems that have already been fixed in master.
  This can be done publicly at any time, and helps with ensuring a smooth process once the security issue is released.
  (The WG may disable branch protections to push directly to the stable branch, but this step is still useful to assist with local testing and the beta branch.)
- [ ] After the fix is approved, create backports to the stable and beta master branches and post PRs to the private fork.
- [ ] If any internal dependencies are changed, make sure their versions are bumped appropriately, and dependency specifications are updated (stable, beta, and master branches).
- [ ] Thoroughly test the stable and beta PRs locally, too. We want to make sure everything goes smoothly, and we can't assume that just because a patch applied cleanly that there won't be issues.
- [ ] Make sure cargo's version in [`Cargo.toml`] is updated correctly on the stable branch private PR.
- [ ] Make sure cargo's `Cargo.lock` is updated (stable, beta, master branches).
- [ ] Update [`CHANGELOG.md`] on cargo's master branch private PR.
- [ ] Update [`RELEASES.md`] on rust's master branch private PR (and stable and beta?).
- [ ] Remove any temporary things in the patch, like a temporary `[patch]` table.

Showtime:
- [ ] Publish any embargoed external dependencies to crates.io.
- [ ] (WG) Merge the cargo stable change.
- [ ] (WG) Update the cargo submodule in the rust-lang/rust private PR to point to the new stable commit.
    - [ ] Also update `Cargo.lock`.
- [ ] (WG) Make a new stable release.
- [ ] (WG) Publish the GHSA.
- [ ] (WG) Send announcements.
- [ ] Make sure stable, beta, and master branches of `rust-lang/cargo` get updated.
- [ ] Make sure stable, beta, and master branches of `rust-lang/rust` get updated, pointing to the correct submodule versions.
- [ ] If any external dependencies are updated, make sure their back or forward ports are handled.

Post release:
- [ ] Verify that the appropriate crates are published on crates.io.
- [ ] Verify that `rust-lang/cargo` got a new tag.
- [ ] Verify that the patches were backported to the correct branches in the `rust-lang/cargo` repository (stable, beta, and master).
- [ ] Verify that the cargo submodule is updated on the correct branches in the `rust-lang/rust` repository (stable, beta, and master).
- [ ] Follow up on any non-critical tasks that were identified during review.

[Rust security policy]: https://www.rust-lang.org/policies/security
[github-advisory]: https://docs.github.com/en/code-security/security-advisories/repository-security-advisories
[private-fork]: https://docs.github.com/en/code-security/security-advisories/repository-security-advisories/collaborating-in-a-temporary-private-fork-to-resolve-a-repository-security-vulnerability
[calc]: https://www.first.org/cvss/calculator
[GHSA-r5w3-xm58-jv6j]: https://github.com/rust-lang/cargo/security/advisories/GHSA-r5w3-xm58-jv6j
[handling reports]: https://github.com/rust-lang/wg-security-response/blob/main/docs/handling-reports.md
[first-guide]: https://www.first.org/cvss/user-guide
[`CHANGELOG.md`]: https://github.com/rust-lang/cargo/blob/master/CHANGELOG.md
[`Cargo.toml`]: https://github.com/rust-lang/cargo/blob/master/Cargo.toml
[`RELEASES.md`]: https://github.com/rust-lang/rust/blob/master/RELEASES.md
