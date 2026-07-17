# New Packages

This chapter sketches out how to add a new package to the cargo workspace.

## Steps

Choose the relevant parent directory
- `credential/` for credential-process related packages
- `benches/` for benchmarking of cargo itself
- `crates/` for everything else

Run `cargo new <name>`
- `<name>`:
  - We tend to use `-` over `_`
  - For internal APIs, to avoid collisions with third-party subcommands, we can use the `cargo-util-` prefix
  - For xtasks, we use the `xtask-` prefix
- `package.rust-version`
  - Internal packages tend to have a policy of "latest" with a [`# MSRV:1` comment](#msrv-policy)
  - Ecosystem packages tend to have a policy of "N-2" with a [`# MSRV:3` comment](#msrv-policy)
  - If the right choice is inherited from the workspace, feel free to keep it that way
- If running without [cargo new automatically adding to workspace](https://github.com/rust-lang/cargo/pull/12779), add it as a workspace member if not already captured by a glob

If its an xtask,
- Add it to `.cargo/config.toml`s `[alias]` table
- Mark `package.publish = false`

If needed to be published with `cargo`,
add the package to `publish.py` in the repo root,
in dependency order.

Note: by adding the package to the workspace, you automatically get
- CI running `cargo test`
- CI verifying MSRV
- CI checking for `cargo doc` warnings

## MSRV Policy

Our MSRV policies are
- Internal packages: support latest version
- Ecosystem packages: support latest 3 versions

We proactively update the MSRV
- So contributors don't shy away from using newer features, either assuming they
  can't ask or feeling like they have to have a justification when asking
- To avoid a de facto MSRV developing from staying on a version for a long
  period of time, leaving users unhappy when their expectations aren't met

To proactively update the MSRV, we use [RenovateBot](https://docs.renovatebot.com/)
with the configuration file in `.github/renovatebot.json5`.
To know what MSRV policy to use,
it looks for comments of the form `# MSRV:N`,
where `N` is the number of supported rust versions.
