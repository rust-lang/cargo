# Upstream Cargo as an edit.dev WASI CLI

Last updated: 2026-05-13

This checkout contains an experimental build of upstream Cargo that can run as a
`wasm32-wasip2` command component inside the edit.dev developer environment.
The goal is not full Cargo parity yet. The current target is a useful Cargo CLI
that can start, parse commands, operate on local workspace files, fetch sparse
registry crates through WASI HTTP, and drive the edit-hosted rustc path for
fixture builds under a WASIp2 command runtime.

The adjacent edit repo publishes the built `cargo.wasm` into the local Web
Assembly package registry as a CLI package named `edit-dev-env/cargo-upstream`.

## Repositories and branch

- Cargo checkout: `/Users/interpretations/projects/cargo`
- edit checkout: `/Users/interpretations/projects/integrate/edit`
- edit test worktree used for the current fork harness:
  `/Users/interpretations/projects/integrate/edit-deployed-may-06`
- Cargo branch used for this work: `codex/wasm-cargo-runtime`
- Built artifact: `/Users/interpretations/projects/cargo/target/wasm32-wasip2/release/cargo.wasm`
- Artifact sha256 at the time of this handoff:
  `6e02981be209724a09bd2184a9f95a30fd69b7fc202bf5974ae7cc958824cbf8`
- edit local registry package version used for the WASIp2 artifact: `0.1.16`

## Build and smoke test

From the Cargo checkout:

```sh
cd /Users/interpretations/projects/cargo
cargo build --release --target wasm32-wasip2 --bin cargo
```

The current artifact imports the edit host process bridge
`edit-dev:upstream-cargo/process@0.1.0`, so plain Wasmtime execution is no
longer the main smoke test unless that host import is provided. Inspect imports
with:

```sh
wasm-tools component wit target/wasm32-wasip2/release/cargo.wasm
```

Expected Cargo version shape when run in the edit runtime or fork harness:

```text
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
```

The fork harness in the `edit-deployed-may-06` worktree uses the built Cargo
artifact directly:

```sh
cd /Users/interpretations/projects/integrate/edit-deployed-may-06/runtimes/rust/cargo/tools
pnpm upstream-cargo -- \
  --project ../tests/fixtures/test-json \
  --out /private/tmp/edit-upstream-cargo/test-json-out \
  -- build --target wasm32-wasip2
```

The manifest currently used by that harness is:

```text
/Users/interpretations/projects/integrate/edit-deployed-may-06/runtimes/rust/cargo/tools/upstream/upstream-fixtures.json
```

As of 2026-05-13, every fixture in that manifest passes through
`upstream-cargo-runner.ts`: `test-minimal`, `test-regex`, and `test-json`.

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

The root `Cargo.toml` adds a WASI-only dependency on `wasip2` for registry HTTP
requests. `crates/cargo-util/Cargo.toml` adds WASIp2-only `base64`, `serde`,
and `serde_json` dependencies for the edit host-process bridge and proc-macro
section injection.

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

The normal sparse HTTP registry implementation is replaced under
`cargo_wasm_cli` with a WASI HTTP-backed implementation. It preserves the shape
Cargo expects from `RegistryData`, but performs GET requests through
`wasi:http/outgoing-handler` rather than through curl.

Current behavior:

- sparse `config.json` loads use the registry cache when present and otherwise
  fetch over WASI HTTP when network is allowed
- sparse index entries support ETag and Last-Modified freshness checks
- crate downloads use Cargo's normal registry cache/download helpers after the
  response body is fetched through WASI HTTP
- downloaded tarballs are unpacked into the registry source cache
- `block_until_ready` is a no-op because the WASI implementation is synchronous

Git-index registries are still unsupported under `cargo_wasm_cli`; use sparse
registries for the current edit runtime path.

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
- `split_paths` and `join_paths` use `:` on WASI instead of relying on
  `std::env::{split_paths,join_paths}`, which are unavailable in the browser
  runtime.
- filesystem timestamp helpers return deterministic zero times on WASI where
  the `filetime` crate cannot read creation/mtime metadata; Cargo still relies
  on content and path changes for the relevant fork-harness rebuilds.
- directory link/copy handling falls back to recursive copy with `walkdir`.
- directory symlink creation reports `Unsupported`.

This is intentionally conservative. The edit runtime filesystem is expected to
be UTF-8-path-oriented, and recursive copying is more useful than failing when
directory symlinks are unavailable.

### Process, stdio, and terminal adaptations

Files:

- `crates/cargo-util/src/process_builder.rs`
- `crates/cargo-util/src/wasm_host_process.rs`
- `crates/cargo-util/src/process_error.rs`
- `crates/cargo-util/src/read2.rs`
- `credential/cargo-credential/src/stdio.rs`
- `src/cargo/core/shell.rs`
- `src/cargo/util/job.rs`
- `src/cargo/util/flock.rs`

WASI-specific behavior:

- On `wasm32-wasip2`, `ProcessBuilder::{status,output,exec_with_streaming}`
  route through the edit host import
  `edit-dev:upstream-cargo/process@0.1.0`.
- The host-process request serializes program, `arg0`, args, cwd, environment,
  and optional stdin as JSON. The response carries exit code, stdout, stderr,
  and an optional host error.
- `ProcessBuilder::exec_replace` falls back to `exec`.
- command-line-too-big detection returns `false`.
- `read2` reads stdout to completion, then stderr to completion.
- credential stdio replacement is a no-op and uses `/dev/null`.
- terminal width reports `NoTty`; erase-line is a no-op.
- job setup is a no-op.
- unsupported file locking errors are recognized as unsupported.
- Cargo's jobserver helper thread is disabled under browser WASI. Builds run
  with Cargo's implicit token instead of creating a helper thread.

The host-process bridge also has a proc-macro post-processing hook. When a
rustc invocation carries `CARGO_PROC_MACRO_CUSTOM_SECTION_B64`, Cargo asks the
host to run rustc, then injects the decoded custom section into the generated
`.wasm` proc-macro artifact and mirrors the `.wasm`/`.rmeta` into the
`wasm32-wasip1/<profile>/deps` directory that downstream rustc invocations
search.

### Host rustc and proc-macro compilation

Files:

- `src/cargo/core/compiler/mod.rs`
- `src/cargo/core/compiler/wasm_proc_macro.rs`
- `crates/cargo-util/src/wasm_host_process.rs`

The fork now uses the existing edit rustc runtime through Cargo's normal
process boundary instead of trying to execute native tools directly inside the
component. For normal project crates, Cargo still emits ordinary rustc command
lines and lets the host-process import run the browser-hosted rustc.

Proc-macro crates need extra handling because the forked rustc path expects
Watt-compatible wasm proc macros:

- proc-macro crate sources are copied into
  `target/cargo-proc-macro-transform/<crate>`
- exported `#[proc_macro_derive]`, `#[proc_macro_attribute]`, and
  `#[proc_macro]` functions are rewritten to exported `extern "C"` wrappers
  that use `proc_macro2::TokenStream`
- wrapper outputs are normalized through `TokenStream::to_string().parse()` so
  downstream expansion sees stable token streams
- the transformed proc-macro crate is compiled as a `wasm32-wasip1` `cdylib`
  with `--watt-cdylib-proc-macro`
- Cargo generates the rustc proc-macro declaration custom section and passes it
  through `CARGO_PROC_MACRO_CUSTOM_SECTION_B64` for host-side injection
- transformed proc-macro builds emit metadata and link artifacts, and Cargo
  prefers `.rmeta` paths for downstream `--extern` wiring where appropriate

`proc-macro2` is special-cased as well. When the package version has a matching
vendored tree at `/vendor/proc-macro2-<version>`, Cargo compiles that vendored
source for the wasm proc-macro target. Cargo also adds an implicit
`proc_macro2` extern for transformed proc-macro crates that only receive it
transitively through crates such as `quote`.

### wstd attribute pre-expansion

File:

- `src/cargo/core/compiler/wasm_proc_macro.rs`

During the HTTP fixture investigation, the rustc/Watt bridge consistently
trapped on attribute macro expansion, even for tiny no-op attribute macros on
`async fn main`. Derive macros such as `serde_derive` and `clap_derive` pass,
but the generic attribute macro path is not healthy yet.

To keep the HTTP examples testable, Cargo now pre-expands the two wstd
attributes used by the fixtures before invoking rustc:

- `#[wstd::http_server]`
- `#[wstd::main]`

For `#[wstd::http_server]`, Cargo rewrites the annotated `main` function into
the same shape produced by wstd's macro: a `TheServer` implementation of the
WASI HTTP incoming-handler guest interface, a `Responder`, a `runtime::block_on`
call, and the `wstd::__internal::wasip2::http::proxy::export!` invocation.

For `#[wstd::main]`, Cargo rewrites `async fn main` into a synchronous `main`
that calls `wstd::runtime::block_on`.

This is intentionally narrow. It fixes the wstd HTTP fixtures and avoids the
known trapping path, but it is not a general fix for arbitrary proc-macro
attributes.

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
  --version 0.1.16 \
  --package edit-dev-env/cargo-upstream \
  --if-exists fail
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

## Deno fork harness status

The current fork-harness work lives in:

```text
/Users/interpretations/projects/integrate/edit-deployed-may-06/runtimes/rust/cargo
```

The runner is:

```text
tools/upstream/upstream-cargo-runner.ts
```

It uses the local Cargo fork artifact by default:

```text
/Users/interpretations/projects/cargo/target/wasm32-wasip2/release/cargo.wasm
```

The current upstream harness manifest is:

```text
tools/upstream/upstream-fixtures.json
```

As of 2026-05-13, the manifest contains and passes these fixtures through
`pnpm upstream-cargo`:

- `test-minimal`
- `test-regex`
- `test-json`

Commands used for the manifest fixtures:

```sh
cd /Users/interpretations/projects/integrate/edit-deployed-may-06/runtimes/rust/cargo/tools

pnpm upstream-cargo -- \
  --project ../tests/fixtures/test-minimal \
  --out /private/tmp/edit-upstream-cargo/manifest-test-minimal-out \
  -- build --target wasm32-wasip2

pnpm upstream-cargo -- \
  --project ../tests/fixtures/test-regex \
  --out /private/tmp/edit-upstream-cargo/manifest-test-regex-out \
  -- build --target wasm32-wasip2

pnpm upstream-cargo -- \
  --project ../tests/fixtures/test-json \
  --out /private/tmp/edit-upstream-cargo/manifest-test-json-out \
  -- build --target wasm32-wasip2
```

Additional non-manifest fixtures manually checked while debugging proc macros:

- `test-clap`: passes
- `test-http-request`: passes
- `test-http-server`: passes after wstd attribute pre-expansion
- `test-workspace`: passed earlier in the same fork-harness work

Do not confuse `tools/upstream/upstream-fixtures.json` with
`tools/parity-fixtures.json`. The parity manifest is useful for local/native
comparisons, but it is not the current fork-harness acceptance set.

## Current known limitations

- No real crates.io publish/yank/search HTTP API support.
- No git-index registry fetching.
- No git dependencies.
- No SQLite global cache tracking.
- No libgit2 or gitoxide integration.
- Credential stdio replacement is stubbed.
- Sparse registry fetching and remote crate downloads now use
  `wasi:http/outgoing-handler`; the edit runtime must provide that import.
- Registry crate unpacking disables tar mtime restoration under
  `cargo_wasm_cli` and ignores WASI-only tar chmod/mtime restoration failures
  after tar has created each entry. The WASI `.cargo-ok` marker version is `3`
  so older partial unpack caches are rebuilt. If the marker is missing but the
  source directory exists, the WASI path clears that directory before unpacking.
- Some terminal behavior is no-op because the command runs without a native TTY.
- Some symlink behavior falls back to recursive copy or reports unsupported.
- External commands, including `git` and `rustc`, depend on what the edit
  runtime exposes as CLI tools.
- The WASIp2 artifact now imports `edit-dev:upstream-cargo/process@0.1.0`.
  edit provides that host import and routes `rustc` invocations to the existing
  browser `rustc.wasm` runtime assets.
- The Cargo component embeds the WIT metadata for that host import directly in
  the wasm module. Normal project builds do not inject custom WIT or linker
  arguments; they use rustc's standard `wasm32-wasip2` component linker path.
- Arbitrary proc-macro attributes can still trap in the rustc/Watt bridge. The
  current workaround only pre-expands the wstd attrs used by the HTTP fixtures.
- The current smoke-tested reliable command surface is local CLI behavior,
  especially `version`, `new --vcs none`, `check --target wasm32-wasip2`, and
  `build --target wasm32-wasip2` for the fork-harness fixtures listed above.

## Suggested next work

1. Validate sparse registry access inside the edit app runtime.
   Cargo now imports `wasi:http/outgoing-handler` for registry GET requests;
   the runtime needs to provide it and allow requests to registry hosts such as
   `index.crates.io` and `static.crates.io`. The Deno fork harness path is
   working for the manifest fixtures.

2. Investigate the generic proc-macro attribute trap.
   wstd attrs are pre-expanded as a narrow workaround, while derive macros and
   the current fixture set pass.

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
cargo fmt --check
cargo build --release --target wasm32-wasip2 --bin cargo
shasum -a 256 target/wasm32-wasip2/release/cargo.wasm
git diff --check -- src/cargo/core/compiler/mod.rs src/cargo/core/compiler/wasm_proc_macro.rs
```

The latest verified artifact hash was:

```text
6e02981be209724a09bd2184a9f95a30fd69b7fc202bf5974ae7cc958824cbf8  target/wasm32-wasip2/release/cargo.wasm
```

To inspect the component imports outside edit:

```sh
wasm-tools component wit target/wasm32-wasip2/release/cargo.wasm
```

The artifact requires the edit host import
`edit-dev:upstream-cargo/process@0.1.0`, so plain Wasmtime command execution is
no longer sufficient unless that import is also provided.

To verify the current fork harness manifest:

```sh
cd /Users/interpretations/projects/integrate/edit-deployed-may-06/runtimes/rust/cargo/tools
pnpm upstream-cargo -- \
  --project ../tests/fixtures/test-minimal \
  --out /private/tmp/edit-upstream-cargo/manifest-test-minimal-out \
  -- build --target wasm32-wasip2
pnpm upstream-cargo -- \
  --project ../tests/fixtures/test-regex \
  --out /private/tmp/edit-upstream-cargo/manifest-test-regex-out \
  -- build --target wasm32-wasip2
pnpm upstream-cargo -- \
  --project ../tests/fixtures/test-json \
  --out /private/tmp/edit-upstream-cargo/manifest-test-json-out \
  -- build --target wasm32-wasip2
```

To publish into edit's local registry:

```sh
cd /Users/interpretations/projects/integrate/edit
pnpm registry:seed:local -- \
  --version 0.1.16 \
  --package edit-dev-env/cargo-upstream \
  --if-exists fail
```

If that version already exists locally, use a new version or reset the local
registry. For idempotent setup, `--if-exists skip` is fine, but it preserves the
previously published artifact bytes.

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
cargo-upstream check --target wasm32-wasip2
cargo-upstream build --target wasm32-wasip2
```
