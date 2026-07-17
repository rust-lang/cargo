# Design Principles

The purpose of Cargo is to formalize a canonical Rust workflow, by automating
the standard tasks associated with distributing software. Cargo simplifies
structuring a new project, adding dependencies, writing and running unit
tests, and more.

Cargo is not intended to be a general-purpose build tool. Ideally, it should
be easy to integrate it within another build tool, though admittedly that is
not as seamless as desired.

## Stability and compatibility

### Backwards compatibility

Cargo strives to remain backwards compatible with projects created in previous
versions. The CLI interface also strives to remain backwards compatible, such
that the commands and options behave the same. That being said, changes in
behavior, and even outright breakage are sometimes done in limited situations.
The following outlines some situations where backwards-incompatible changes are
made:

* Anything that addresses a security concern.
* Dropping support for older platforms and tooling. Cargo follows the Rust
  [tiered platform support].
* Changes to resolve possibly unsafe or unreliable behavior.

None of these changes should be taken lightly, and should be avoided if
possible, or possibly with some transition period to alert the user of the
potential change.

Behavior is sometimes changed in ways that have a high confidence that it
won't break existing workflows. Almost every change carries this risk, so it
is often a judgment call balancing the benefit of the change with the
perceived possibility of its negative consequences.

At times, some changes fall in the gray area, where the current behavior is
undocumented, or not working as intended. These are more difficult judgment
calls. The general preference is to balance towards avoiding breaking existing
workflows.

Support for older registry APIs and index formats may be dropped, if there is
high confidence that there aren't any active registries that may be affected.
This has never (to my knowledge) happened so far, and is unlikely to happen in
the future, but remains a possibility.

In all of the above, a transition period may be employed if a change is known
to cause breakage. A warning can be issued to alert the user that something
will change, and provide them with an alternative to resolve the issue
(preferably in a way that is compatible across versions if possible).

Cargo is only expected to work with the version of the related Rust tools
(`rustc`, `rustdoc`, etc.) that it is released with. As a matter of choice,
the latest nightly works with the most recent stable release, but that is
mostly to accommodate development of Cargo itself, and should not be expected
by users.

### Forwards compatibility

Additionally, Cargo strives a limited degree of *forwards compatibility*.
Changes should not egregiously prevent older versions from working. This is
mostly relevant for persistent data, such as on-disk files and the registry
interface and index. It also applies to a lesser degree to the registry API.

Changes to `Cargo.lock` require a transition time, where the new format is not
automatically written when the lock file is updated. The transition time
should not be less than 6 months, though preferably longer. New projects may
use the new format in a shorter time frame.

Changes to `Cargo.toml` can be made in any release. This is because the user
must manually modify the file, and opt-in to any new changes. Additionally,
Cargo will usually only issue a warning about new fields it doesn't
understand, but otherwise continue to function.

Changes to cache files (such as artifacts in the `target` directory, or cached
data in Cargo's home directory) should not *prevent* older versions from
running, but they may cause older versions to recreate the cache, which may
result in a performance impact.

Changes to the registry index should not prevent older versions from working.
Generally, older versions ignore new fields, so the format should be easily
extensible. Changes to the format or interpretation of existing fields should
be done very carefully to avoid preventing older versions of Cargo from
working. In some cases, this may mean that older versions of Cargo will not be
able to *select* a newly published crate, but it shouldn't prevent them from
working at all. This level of compatibility may not last forever, but the
exact time frame for such a change has not yet been decided.

The registry API may be changed in such a way to prevent older versions of
Cargo from working. Generally, compatibility should be retained for as long as
possible, but the exact length of time is not specified.

## Simplicity and layers

Standard workflows should be easy and consistent. Each knob that is added has
a high cost, regardless if it is intended for a small audience. Layering and
defaults can help avoid the surface area that the user needs to be concerned
with. Try to avoid small functionalities that may have complex interactions
with one another.

[tiered platform support]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
