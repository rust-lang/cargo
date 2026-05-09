# Upstream Cargo as an edit.dev WASI CLI

Last updated: 2026-05-09

This checkout contains an experimental build of upstream Cargo that can run as a
`wasm32-wasip2` command component inside the edit.dev developer environment.
The goal is not full Cargo parity yet. The current target is a useful Cargo CLI
that can start, parse commands, operate on local workspace files, and support
simple flows such as `cargo version` and `cargo new --vcs none` under a WASIp2
command runtime.

The adjacent edit repo publishes the built `cargo.wasm` into the local Web
Assembly package registry as a CLI package named `edit-dev-env/cargo-upstream`.

## Repositories and branch

- Cargo checkout: `/Users/interpretations/projects/cargo`
- edit checkout: `/Users/interpretations/projects/integrate/edit`
- Cargo branch used for this work: `codex/wasm-cargo-runtime`
- Built artifact: `/Users/interpretations/projects/cargo/target/wasm32-wasip2/release/cargo.wasm`
- Artifact sha256 at the time of this handoff:
  `df8157e48537fa0c84bccf5178874e5e5056311fff66431300085b64393f7dad`
- edit local registry package version used for the WASIp2 artifact: `0.1.2`

## Build and smoke test

From the Cargo checkout:

```sh
cd /Users/interpretations/projects/cargo
cargo build --release --target wasm32-wasip2 --bin cargo
```

The current artifact was smoke-tested with Wasmtime:

```sh
wasmtime \
  --env HOME=/private/tmp \
  --dir /private/tmp \
  target/wasm32-wasip2/release/cargo.wasm \
  version
```

Expected output shape:

```text
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
```

Simple local filesystem command smoke test:

```sh
wasmtime \
  --env HOME=/private/tmp \
  --dir /private/tmp \
  target/wasm32-wasip2/release/cargo.wasm \
  new --vcs none /private/tmp/cargo-wasi-smoke
```

## Why shims are needed

Upstream Cargo assumes several host facilities that are either unavailable or
not yet bridged in a browser/WASI command environment:

- libgit2, gitoxide, and native git repository access.
- libcurl and native TLS/socket access.
- SQLite-backed global cache tracking.
- Unix-specific file descriptor and terminal behavior.
- Some symlink and path byte APIs.
- Process spawning and stdio replacement behavior that differs under WASI.

The current patches make those surfaces compile for `wasm32-wasip2` and provide
explicit unsupported behavior where runtime support is not available. The intent
is to keep the wasm build honest: commands should either work locally or fail
with a clear "not available in the WASI Cargo CLI yet" message.

## Cargo-side shims

### `cargo_wasm_cli` cfg

`build.rs` emits a custom cfg for WASI targets:

```rust
cargo:rustc-check-cfg=cfg(cargo_wasm_cli)
cargo:rustc-cfg=cargo_wasm_cli
```

It is enabled when `TARGET` starts with `wasm32-wasi`, which covers
`wasm32-wasip1` and `wasm32-wasip2`. Most Cargo-specific conditional code uses
this cfg instead of raw target checks.

### Native dependency gating

`Cargo.toml` moves native-only dependencies behind:

```toml
[target.'cfg(not(all(target_arch = "wasm32", target_os = "wasi")))'.dependencies]
```

This gates out:

- `curl`
- `curl-sys`
- `git2`
- `git2-curl`
- `gix`
- `libgit2-sys`
- `rusqlite`

The workspace dependency on `home` is redirected to the in-tree `crates/home`
crate so we can add WASI-specific home-directory behavior.

`crates/crates-io/Cargo.toml` also gates its `curl` dependency so the
`crates-io` helper crate can compile for WASI.

### curl compatibility stubs

Files:

- `src/cargo/wasm_curl.rs`
- `src/cargo/wasm_curl_sys.rs`

These provide a small API-compatible subset of `curl` and `curl-sys` used by
Cargo internals. They are not real HTTP clients. Most setters are accepted as
no-ops, `perform` does not perform network traffic, and error classification
methods return `false`.

`src/cargo/lib.rs` exposes these modules only under `cargo_wasm_cli`.

Several Cargo modules import `crate::wasm_curl as curl` under `cargo_wasm_cli`
so existing Cargo code can keep referring to `curl::easy::Easy`,
`curl::multi::Multi`, etc.

Affected areas include:

- `src/cargo/core/package.rs`
- `src/cargo/sources/registry/http_remote.rs`
- `src/cargo/util/context/mod.rs`
- `src/cargo/util/errors.rs`
- `src/cargo/util/network/http.rs`
- `src/cargo/util/network/retry.rs`

### crates.io HTTP shim

File: `crates/crates-io/lib.rs`

For WASI, this crate defines an internal `curl` shim and returns an explicit
unsupported error for registry HTTP operations such as publish/yank/search API
requests. This gets the crate compiling while avoiding fake network success.

Current unsupported message:

```text
crates.io HTTP is not available in the WASI Cargo CLI yet
```

### Registry source shims

Files:

- `src/cargo/sources/registry/http_remote_wasm.rs`
- `src/cargo/sources/registry/remote_wasm.rs`
- `src/cargo/sources/registry/mod.rs`

The normal sparse HTTP registry and git-index registry implementations are
replaced under `cargo_wasm_cli`.

The WASI replacements preserve the shape Cargo expects from `RegistryData`, but
return explicit unsupported errors for:

- sparse registry loading
- git registry loading
- crate downloads
- `block_until_ready`

They still construct registry index/cache paths through `GlobalContext` so code
that only calculates paths can continue.

### Git source shim

Files:

- `src/cargo/sources/git/mod.rs`
- `src/cargo/sources/git/unsupported.rs`

Under `cargo_wasm_cli`, the normal `git2`/`gix` implementation is replaced with
types that satisfy Cargo's source interfaces and return clear unsupported
errors for git dependency queries, downloads, fetches, and ref resolution.

Current unsupported message shape:

```text
git sources are not available in the WASI Cargo CLI yet
```

### Global cache tracker shim

Files:

- `src/cargo/core/mod.rs`
- `src/cargo/core/global_cache_tracker_wasm.rs`
- `src/cargo/util/mod.rs`

The normal cache tracker depends on SQLite. The WASI shim replaces it with a
no-op tracker:

- auto-GC never runs
- last-use tracking is empty
- save operations are no-ops
- clean operations succeed without SQLite

`src/cargo/util/mod.rs` hides the `sqlite` module under `cargo_wasm_cli`.

### Package VCS shim

Files:

- `src/cargo/ops/cargo_package/mod.rs`
- `src/cargo/ops/cargo_package/vcs_wasm.rs`

`cargo package` normally records VCS information from git. The WASI shim returns
`Ok(None)` for VCS info, avoiding `git2` while keeping package code compiling.

### Path and filesystem adaptations

File: `crates/cargo-util/src/paths.rs`

WASI-specific behavior:

- `path2bytes` converts through UTF-8 instead of Unix `OsStrExt`.
- `bytes2path` requires valid UTF-8.
- directory link/copy handling falls back to recursive copy with `walkdir`.
- directory symlink creation reports `Unsupported`.

This is intentionally conservative. The edit runtime filesystem is expected to
be UTF-8-path-oriented, and recursive copying is more useful than failing when
directory symlinks are unavailable.

### Process, stdio, and terminal adaptations

Files:

- `crates/cargo-util/src/process_builder.rs`
- `crates/cargo-util/src/process_error.rs`
- `crates/cargo-util/src/read2.rs`
- `credential/cargo-credential/src/stdio.rs`
- `src/cargo/core/shell.rs`
- `src/cargo/util/job.rs`
- `src/cargo/util/flock.rs`

WASI-specific behavior:

- `ProcessBuilder::exec_replace` falls back to `exec`.
- command-line-too-big detection returns `false`.
- `read2` reads stdout to completion, then stderr to completion.
- credential stdio replacement is a no-op and uses `/dev/null`.
- terminal width reports `NoTty`; erase-line is a no-op.
- job setup is a no-op.
- unsupported file locking errors are recognized as unsupported.

### CLI startup and version output

Files:

- `src/bin/cargo/cli.rs`
- `src/bin/cargo/main.rs`

WASI behavior:

- `init_git` and git transport setup are disabled.
- verbose version output skips libgit2 and curl version details.
- WASI gets an `is_executable` implementation that treats files as executable.

This keeps `cargo version` and CLI dispatch working without pulling in native
git/curl dependencies.

### Path source and VCS command behavior

Files:

- `src/cargo/sources/path.rs`
- `src/cargo/util/vcs.rs`
- `src/cargo/ops/fix/mod.rs`

WASI behavior:

- Path source file listing avoids `gix` repository discovery and falls back to
  filesystem walking.
- `cargo fix` does not inspect git status with `git2`.
- `GitRepo::init` invokes an external `git init` command rather than libgit2.

In the edit runtime, whether `git init` works depends on whether the runtime can
resolve and execute a `git` command. For reliable smoke tests, use
`cargo new --vcs none`.

### Miscellaneous compile fix

File: `src/cargo/core/resolver/encode.rs`

One call passes `&dep` instead of `dep` when walking `ws.root_replace()`. This
is a small compatibility fix triggered by the current local build.

## edit runtime seeding

The edit repo publishes local WebAssembly CLI packages through:

```text
/Users/interpretations/projects/integrate/edit/scripts/seed-local-wa-registry.mjs
```

A package entry was added:

```js
{
  namespace: "edit-dev-env",
  name: "cargo-upstream",
  command: "cargo-upstream",
  aliases: ["cargo-wasm"],
  path: "../../cargo/target/wasm32-wasip2/release/cargo.wasm",
  artifactFormat: "wasip2-command-component",
  runtime: "wasip2-command",
  network: "outbound",
  description:
    "Experimental upstream Cargo WASIp2 command component built from the adjacent Cargo checkout.",
}
```

The seed script was also generalized so CLI packages can specify:

- `artifactFormat`
- `runtime`
- `entrypoint`
- `world`

Before this, CLI metadata was hardcoded as WASIp2 command-component metadata.
The script also now accepts:

```sh
--package <namespace/name>
```

That allows publishing only this Cargo package instead of reseeding everything.

Initial publish command used:

```sh
cd /Users/interpretations/projects/integrate/edit
pnpm registry:seed:local -- \
  --version 0.1.2 \
  --package edit-dev-env/cargo-upstream \
  --if-exists skip
```

Important: `--if-exists skip` will not replace an already-published package. If
the Cargo wasm artifact changes and the registry should point at the new bytes,
either bump the package version in the seed script or clear/reset the local dev
registry before publishing. Restarting the edit dev server does not require
reseeding by itself.

## Using the CLI in edit

Install package:

```text
edit-dev-env/cargo-upstream
```

Then in an edit shell:

```sh
export HOME=/
cargo-upstream version
```

Alias:

```sh
cargo-wasm version
```

The `HOME=/` export matters because the WASI Cargo build uses the `HOME`
environment variable through the patched `home` crate. Without it, Cargo may
fail while trying to locate Cargo home/config paths.

The command name `cargo` is intentionally not claimed by this package right now.
The existing edit runtime has a separate built-in Cargo host-adapter wrapper.
Use `cargo-upstream` or `cargo-wasm` when testing this upstream WASI build.

## edit runtime argv fix

A runtime issue showed up after publishing. Running:

```sh
cargo-upstream version
```

initially produced:

```text
runtime:wasip2-command:sync
error: no such command: `cargo-upstream`
help: view all installed commands with `cargo --list`
```

That meant the runtime had found the WASIp2 CLI package, but the wasm component
was receiving argv like:

```text
["cargo-upstream", "cargo-upstream", "version"]
```

Cargo interpreted the second `cargo-upstream` as a subcommand. The fix is in
the edit repo:

```text
app/src/lib/execution/local-wasm-execution-host.ts
```

The helper:

```ts
normalizeSessionCommandArgv(manifest, argv)
```

strips the first argv token when it matches the tool id, entrypoint, or alias.
The normalized argv is now passed both to the command factory and to the actual
WASI command invocation.

Regression test:

```text
app/src/lib/execution/local-wasm-execution-host-session-cli.test.ts
```

Focused test command:

```sh
cd /Users/interpretations/projects/integrate/edit
pnpm --filter @edit/app exec vitest run \
  src/lib/execution/local-wasm-execution-host-session-cli.test.ts \
  src/lib/execution/wasip2-command-process.test.ts
```

Observed result after the fix:

```text
2 test files passed
10 tests passed
```

If edit still reports `error: no such command: cargo-upstream`, the likely cause
is stale app/runtime code. Restart the dev server or hard-refresh the worker.
That error is not a registry publishing failure if `runtime:wasip2-command`
appears first.

## edit runtime VFS handoff fix

After the WASIp2 switch, `cargo-upstream build` could start Cargo but fail with:

```text
error: could not find `Cargo.toml` in `/` or any parent directory
```

The temporary diagnostic line from the edit runner made the issue explicit:

```text
wasip2-vfs source:provided cwd:/ preopens:/ root-Cargo.toml:no cwd-Cargo.toml:no
```

The runner was already using the root preopen. The missing piece was hydrating
the provided execution VFS from the current project file tree before invoking
Cargo. The edit-side prepare hook now localizes the workspace, writes project
files such as `/Cargo.toml` and `/src/main.rs` into the live VFS, and then hands
that VFS to the WASIp2 command process.

Relevant files:

```text
app/src/lib/project-session/service.ts
app/src/lib/execution/wasip2-command-process.ts
```

## Current known limitations

- No real crates.io HTTP API support.
- No sparse registry fetching.
- No git-index registry fetching.
- No git dependencies.
- No crate downloads from remote registries.
- No SQLite global cache tracking.
- No libgit2 or gitoxide integration.
- Credential stdio replacement is stubbed.
- Some terminal behavior is no-op because the command runs without a native TTY.
- Some symlink behavior falls back to recursive copy or reports unsupported.
- External commands, including `git` and `rustc`, depend on what the edit
  runtime exposes as CLI tools.
- The current smoke-tested reliable command surface is local CLI behavior,
  especially `version` and `new --vcs none`.

## Suggested next work

1. Decide how Cargo should reach registries from the edit runtime.
   The clean direction is a runtime HTTP bridge rather than pretending the curl
   shim is real. Once that exists, replace `wasm_curl` and the registry shims
   with bridge-backed implementations.

2. Decide how Cargo should invoke Rust tools.
   For `cargo build` inside edit, Cargo needs a visible `rustc` command and
   target sysroots. The edit runtime already has Rust runtime assets, but this
   upstream Cargo path still needs end-to-end validation.

3. Decide the package versioning policy for seeded local registry packages.
   `--if-exists skip` is good for idempotent setup, but it hides new artifact
   bytes if the package version is unchanged.

4. Add edit-side package metadata defaults for `HOME` if supported.
   Requiring users to run `export HOME=/` is acceptable for a prototype but not
   ideal.

5. Add Cargo-side wasm tests for the compile shims.
   The current confidence comes from a release build and manual smoke tests, not
   from a dedicated Cargo test suite under `wasm32-wasip2`.

## Agent handoff checklist

Before changing behavior:

```sh
cd /Users/interpretations/projects/cargo
git status --short
```

There are many local patches in this Cargo checkout. Treat them as intentional
unless the user explicitly asks to revert them.

To rebuild:

```sh
cd /Users/interpretations/projects/cargo
cargo build --release --target wasm32-wasip2 --bin cargo
shasum -a 256 target/wasm32-wasip2/release/cargo.wasm
```

To smoke test outside edit:

```sh
wasmtime --env HOME=/private/tmp --dir /private/tmp \
  target/wasm32-wasip2/release/cargo.wasm version
```

To publish into edit's local registry:

```sh
cd /Users/interpretations/projects/integrate/edit
pnpm registry:seed:local -- \
  --version 0.1.2 \
  --package edit-dev-env/cargo-upstream \
  --if-exists skip
```

To verify the edit-side argv fix:

```sh
cd /Users/interpretations/projects/integrate/edit
pnpm --filter @edit/app exec vitest run \
  src/lib/execution/local-wasm-execution-host-session-cli.test.ts \
  src/lib/execution/wasip2-command-process.test.ts
```

In an edit shell after installing `edit-dev-env/cargo-upstream`:

```sh
export HOME=/
cargo-upstream version
```
