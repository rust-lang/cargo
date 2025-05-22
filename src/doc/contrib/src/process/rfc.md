# Writing an RFC

Generally, an RFC goes through:
1. Pre-RFC discussions on the [internals forum][irlo]
2. [RFC]
3. [Development and stabilization][unstable]

Please keep in mind our [design principles](../design.md).

For more concrete areas of consideration:

## `.cargo/config.toml` and `Cargo.toml`

`.cargo/config.toml` is for environment or transient configuration,
being dependent on what directory you are running from and settable on the command-line,
independent of other flags like `--manifest-path` or `--package`.

On the other hand `Cargo.toml` is for static, high-level project configuration.

For example,
- [RFC 3537] chose
  configuration for the MSRV-aware resolver because users would likely need
  to change this setting, like in CI to verify the opposite case of
  what they run by default.
- The Cargo team rejected a [`[cfg]` table][cfg table] to represent `rustc`
  `--cfg` flags as it was a direct port of low-level rustc behavior that didn't
  mesh with the other high level abstractions of manifests.
  - For stabilization, this was worked around through a build script directive and a `[lints]` field configuration.
- [#12738][cargo#12738] for exploring how existing config might be representable in `Cargo.toml`.


[irlo]: https://internals.rust-lang.org/
[RFC]: https://github.com/rust-lang/rfcs/
[unstable]: unstable.md
[RFC 3537]: https://rust-lang.github.io/rfcs/3537-msrv-resolver.html
[cfg table]: https://github.com/rust-lang/cargo/pull/11631#issuecomment-1487424886
[cargo#12738]: https://github.com/rust-lang/cargo/issues/12738

## `Cargo.toml`

When adding a table to a manifest,
- Should it be inheritable?
- Ensure the package table and the inheritable table under `workspace` align
- Care is needed to ensure a `workspace = true` field doesn't conflict with other entries
  - e.g. [RFC 3389] had to explicitly exclude ever supporting a `workspace` linter

When adding a field,
- Is it inheritable?
  - Consider whether sharing of the field would be driven by requirements or is a manifestation of the current implementation.
    For example, in most cases, dependency sources (e.g. `version` field) should be aligned across a workspace
    However, frequently dependency `features` will vary across a workspace.
- When inheriting, can specify it in your package?
- How does specifying a field in both `workspace` and a package interact?
  - e.g. dependency sources cannot be overridden
  - e.g. dependency `features` get merged
  - e.g. dependency `default-features` has been hard to get right ([#12162][cargo#12162])

When working extending `dependencies` tables:
- How does this affect `cargo add` or `cargo remove`?
- How does this affect `[patches]` which are just modified dependencies?

[RFC 3389]: https://rust-lang.github.io/rfcs/3389-manifest-lint.html
[cargo#12162]: https://github.com/rust-lang/cargo/issues/12162

