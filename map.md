# Cargo Codebase Map

This repository is the implementation of Cargo, Rust's package manager and
build tool. The most useful mental model is:

```text
command line
    -> src/bin/cargo/commands/
    -> workspace discovery and configuration
    -> src/ops/
       -> manifest/package loading
       -> source queries and downloads
       -> dependency and feature resolution
       -> compilation graph construction
    -> src/compiler/
       -> fingerprints and cache checks
       -> parallel rustc/rustdoc/build-script execution
    -> target/ artifacts, Cargo.lock, user-facing output
```

This file is a navigation guide, not an API reference. When investigating a
bug, start with the command-specific entry point, then follow the operation
and the shared subsystem it calls.

## 1. Recommended Reading Order

Read these files in this order for a basic overview:

1. `README.md`: project purpose, build requirements, and high-level project
   information.
2. `Cargo.toml`: workspace members, package metadata, features, dependencies,
   and the `cargo` binary declaration.
3. `src/lib.rs`: public module map and Cargo's own architecture notes. Its
   module-level documentation is one of the best orientation documents in the
   repository.
4. `src/bin/cargo/main.rs`: process startup, logging, global context creation,
   jobserver setup, and error termination.
5. `src/bin/cargo/cli.rs`: global argument parsing, aliases, configuration
   setup, and selection of internal versus external subcommands.
6. `src/bin/cargo/commands/mod.rs`: the complete built-in command registry and
   command-to-executor dispatch table.
7. One command entry point, such as
   `src/bin/cargo/commands/build.rs`, to see how CLI arguments become an
   operation request.
8. `src/workspace/workspace.rs` and `src/workspace/parser/mod.rs`: how Cargo
   finds a workspace and turns `Cargo.toml` into packages and targets.
9. `src/ops/resolve.rs` and `src/resolver/mod.rs`: how dependency metadata,
   features, versions, and `Cargo.lock` become a resolved graph.
10. `src/ops/cargo_compile/mod.rs`: the complete compilation front end and its
    handoff to the compiler subsystem.
11. `src/compiler/mod.rs`: build contexts, units, fingerprints, job queues,
    build scripts, and compiler process execution.
12. `tests/testsuite/build.rs`, `tests/testsuite/workspaces.rs`, and focused
    files such as `tests/testsuite/update.rs`, `lockfile_compat.rs`, or
    `minimal_versions.rs`: expected behavior and realistic end-to-end
    examples.

For a first debugging pass, do not begin with individual utility files. Most
behavior is orchestrated by the files above and delegated into smaller
modules.

## 2. Process and Command Flow

### Process startup

`src/bin/cargo/main.rs` is the executable entry point.

- Installs `tracing` logging and optional Chrome profiling.
- Creates `GlobalContext`, which stores configuration, environment, shell
  output state, Cargo home, and caches.
- Handles shell completion when enabled.
- Handles the special proxy mode used by `cargo fix`.
- Initializes Cargo's jobserver before dispatching a normal command.
- Calls `cli::main` and converts failures into Cargo's formatted exit output.

`src/lib.rs` is the library crate root. It exports the main subsystems and
contains `exit_with_error` and error-display helpers. The binary uses this
library rather than containing the implementation itself.

### CLI parsing and dispatch

`src/bin/cargo/cli.rs` owns global CLI behavior.

- Builds the Clap command tree.
- Parses global flags and the selected subcommand.
- Changes directory for `-C` where supported.
- Expands user aliases and built-in aliases such as `b -> build`.
- Loads configuration only after enough arguments are known.
- Handles special top-level actions such as `--version`, `--list`, `--explain`,
  and `-Z help`.
- Initializes Git and invokes the selected executor.

`src/bin/cargo/commands/mod.rs` is the command index. `builtin()` defines what
appears in help, while `builtin_exec()` maps command names to `exec` functions.
Each file in `src/bin/cargo/commands/` normally has two responsibilities:

- `cli()`: declare command syntax, options, help text, and argument semantics.
- `exec()`: obtain a workspace, translate matches into options, and call
  `cargo::ops`.

Commands are intentionally thin. If a command appears to contain complicated
business logic, check whether it is actually argument validation or whether
the real implementation is in `src/ops/`.

Cargo also supports external subcommands. The executable searches `PATH` and
`$CARGO_HOME/bin` for programs named `cargo-<command>`, then replaces the
current process with that program. This behavior is in `main.rs`, alongside
alias and executable lookup helpers.

### Command groups

The command files map to operations approximately as follows:

| User commands | Main operation files | What they do |
| --- | --- | --- |
| `build`, `check`, `bench`, `doc`, `rustc`, `rustdoc` | `ops/cargo_compile`, `ops/cargo_doc` | Configure and compile targets, or invoke documentation/compiler variants. |
| `run` | `ops/cargo_run` plus `ops/cargo_compile` | Compile a binary/example and execute it. |
| `test` | `ops/cargo_test` plus `ops/cargo_compile` | Compile test units, doctests, and examples, then run test processes. |
| `fetch` | `ops/cargo_fetch` | Resolve and download dependencies without compiling them. |
| `update`, `generate-lockfile` | `ops/cargo_update`, `ops/resolve`, `ops/lockfile` | Re-resolve dependencies and read/write `Cargo.lock`. |
| `new`, `init` | `ops/cargo_new` | Create a package or initialize an existing directory. |
| `add`, `remove` | `ops/cargo_add`, `ops/cargo_remove`, `workspace/editor` | Edit dependencies and manifest entries. |
| `metadata`, `pkgid`, `read-manifest`, `locate-project` | `ops/cargo_metadata`, `ops/cargo_pkgid`, `ops/cargo_read_manifest` | Inspect manifests, package IDs, workspace structure, and machine-readable metadata. |
| `package`, `vendor` | `ops/cargo_package`, `ops/cargo_vendor` | Produce publishable packages or a vendored source tree. |
| `publish`, `owner`, `search`, `info`, `login`, `logout`, `yank` | `ops/registry/` | Communicate with registries and manage publishing/authentication. |
| `install`, `uninstall` | `ops/cargo_install`, `ops/cargo_uninstall` | Build/install binaries and remove installed packages. |
| `clean` | `ops/cargo_clean` | Remove build artifacts. |
| `tree` | `ops/cargo_tree` | Display the resolved dependency graph. |
| `fix` | `ops/cargo_fix` and `crates/rustfix` | Apply compiler suggestions to source code. |
| `config`, `report`, `verify-project`, `version`, `help` | Corresponding command and operation modules | Configuration inspection, reports, validation, version output, and help. |

## 3. Shared State and Workspace Model

### Global configuration

`src/context/mod.rs` contains `GlobalContext`, Cargo's process-wide context.
It owns or exposes:

- Cargo home and important filesystem paths.
- Current working directory and environment variables.
- Shell, verbosity, color, and terminal reporting settings.
- Layered configuration from `.cargo/config.toml`, environment variables,
  `--config`, and credentials configuration.
- Network, registry, cache, authentication, and build configuration.
- Unstable feature flags and diagnostic behavior.

`GlobalContext::get` is the typed configuration API. It converts parsed
configuration values through a custom serde deserializer and applies source
precedence. Configuration bugs usually belong in `src/context/`, its `de/`,
`environment/`, `path/`, `schema/`, and `target/` modules rather than in the
command that first observes the bad value.

`src/config/` contains the lower-level configuration storage types:
`ConfigStore`, `ConfigValue`, `Definition`, and `ConfigKey`. `context/` is the
runtime-facing API; `config/` is the value/model layer.

### Manifest and workspace loading

`src/workspace/parser/` parses `Cargo.toml` (and unstable embedded manifests),
reports TOML diagnostics, normalizes fields, expands workspace inheritance,
and constructs real or virtual manifests.

`src/workspace/manifest.rs` defines normalized manifest data. It represents
package metadata, dependencies, targets, features, profiles, workspace
settings, and publish-related fields.

`src/workspace/workspace.rs` discovers the workspace root, loads members,
selects default members, tracks target/build directories, and exposes the
`Workspace` object threaded through most operations.

`src/workspace/package.rs` represents loaded package source and target files.
`Package`, `PackageSet`, and download coordination live here.

`src/workspace/summary.rs` creates the compact package summary used by the
resolver. `src/workspace/dependency.rs` models dependency declarations,
including path, Git, registry, target, optional, artifact, and feature data.
`src/workspace/package_id.rs` and `src/workspace/source_id.rs` provide stable
identities for packages and package sources.

### Features and profiles

`src/workspace/features.rs` handles Cargo feature syntax, unstable feature
flags, editions, and CLI feature configuration. Resolver-side feature
activation is in `src/resolver/features.rs`.

`src/workspace/profiles.rs` models profiles such as `dev`, `release`, `test`,
and custom profiles, including overrides and target-specific settings.

`src/workspace/editor/` provides structured editing of manifests. Look at
`editor/manifest.rs` for document editing and `editor/dependency.rs` for
dependency insertion/update behavior.

## 4. Dependency Resolution and Package Sources

### High-level resolution

`src/ops/resolve.rs` is the orchestration layer. It combines a `Workspace`,
lockfile state, CLI feature choices, source configuration, patches/replacements,
and the package registry to produce `WorkspaceResolve`.

It also decides when to reuse or update the lockfile, when to resolve all
features versus selected features, which packages need downloading, and how
path overrides affect the final graph.

`src/ops/lockfile.rs` loads and writes `Cargo.lock`. `src/resolver/encode.rs`
contains lockfile serialization/version details.

### Resolver algorithm

`src/resolver/mod.rs` contains the core dependency solver. It queries package
metadata, activates compatible versions, unifies features, backtracks on
conflicts, checks cycles/duplicates, and returns `resolver::Resolve`.

Important resolver files:

- `resolver/resolve.rs`: resolved graph representation and graph queries.
- `resolver/features.rs`: requested and activated feature computation.
- `resolver/dep_cache.rs`: cached registry queries.
- `resolver/context.rs` and `resolver/types.rs`: activation state, dependency
  frames, conflicts, and progress state.
- `resolver/version_prefs.rs`: lockfile/version ordering and publish-age
  preferences.
- `resolver/conflict_cache.rs`: avoids repeating known failed activations.
- `resolver/errors.rs`: resolution and activation diagnostics.

The resolver chooses candidate versions; it does not fetch every source file or
run `rustc`. Those responsibilities belong to sources/package downloading and
the compiler pipeline.

### Source abstraction

`src/sources/source.rs` defines the `Source` trait. A source can query package
summaries, download packages, finish downloads, verify packages, and provide a
source fingerprint.

Implementations are split by source kind:

- `sources/registry/`: crates.io, sparse registries, local registries, index
  access, crate archives, checksums, and registry cache management.
- `sources/git/`: Git dependency fetching and checkout/cache behavior.
- `sources/path.rs`: local path dependencies.
- `sources/directory.rs`: vendored directory sources.
- `sources/replaced.rs`: source replacement redirection.
- `sources/overlay.rs`: local source overlays.
- `sources/config.rs`: parsing and applying `[source.*]` configuration.
- `sources/mod.rs`: public exports and source-family documentation.

`src/workspace/registry.rs` contains the package-registry façade used by
resolution. It combines `SourceMap`, patches, replacements, and source queries
into the interface consumed by the resolver.

### Network and authentication

`src/util/network/` handles HTTP clients, asynchronous transfers, retries,
proxies, network configuration, and offline behavior. Start at
`network/mod.rs`, then inspect `http.rs`, `http_async.rs`, `proxy.rs`, and
`retry.rs`.

`src/util/auth/` and `src/util/credential/` handle registry authentication and
credential-provider communication. The `credential/` workspace packages
implement platform-specific providers. A registry login/token problem usually
crosses `ops/registry`, `context`, `util/auth`, `util/credential`, and one of
those provider crates.

## 5. Compilation Pipeline

The complete build pipeline is documented in the module-level comments of
`src/ops/cargo_compile/mod.rs`; use that file as the primary starting point.

### Compilation front end

`ops/cargo_compile::compile` performs the high-level sequence:

1. Emit manifest/parse diagnostics.
2. Resolve workspace dependencies and requested features.
3. Download or load packages.
4. Generate root compilation units for selected Cargo targets.
5. Walk dependencies to build a `UnitGraph`.
6. Build a `BuildContext` containing compiler settings and static build data.
7. Create a `BuildRunner` and execute the dependency-aware job queue.
8. Return `Compilation`, which commands such as `run` and `test` use to launch
   produced binaries.

`src/ops/cargo_compile/` is also where package selection, target filters,
compile kinds, feature options, and unit generation are translated into the
compiler's internal representation.

### Compiler subsystem

`src/compiler/mod.rs` is Cargo's compiler driver layer. It prepares and executes
`rustc`, `rustdoc`, and build-script processes, while managing scheduling,
outputs, diagnostics, and incremental freshness.

Important files:

- `build_context/`: immutable/static information needed for a build.
- `build_runner/`: mutable state and coordination for an active build.
- `unit.rs`: identity and settings for one compilation action.
- `unit_dependencies.rs` and `unit_graph.rs`: convert package resolution into
  compiler-unit dependencies.
- `job_queue/`: parallel scheduling, dependency completion, and job state.
- `fingerprint/`: determines whether an individual unit is fresh or must run.
- `layout.rs`: target directory layout, artifact paths, and target-dir locks.
- `custom_build.rs`: runs build scripts and parses `cargo:`/`cargo::` output.
- `links.rs`: coordinates native library `links` constraints and metadata.
- `compilation.rs`: final artifact and executable information returned to ops.
- `build_config.rs`: low-level rustc/build settings derived from user intent.
- `compile_kind.rs`, `crate_type.rs`, `profiles`, and `lto.rs`: target and
  optimization choices.
- `rustdoc.rs`: rustdoc-specific flags and doctest support.
- `standard_lib.rs`: sysroot/standard-library build integration.
- `timings/`: build timing collection and report generation.
- `future_incompat.rs`, `output_depinfo.rs`, `output_sbom.rs`, and
  `unused_deps.rs`: specialized reporting and generated-output support.

`DefaultExecutor` in `compiler/mod.rs` is the normal process boundary: it
streams compiler output and invokes the prepared command. Tests and selected
library users can provide a custom `Executor` to intercept invocations.

### Freshness and artifacts

The target directory is managed through `compiler/layout.rs`. Fingerprints are
stored under `target/<profile>/.fingerprint` and are evaluated by
`compiler/fingerprint/`. If a build unexpectedly recompiles, inspect in this
order:

1. `ops/cargo_compile/` for changed unit/feature selection.
2. `compiler/unit.rs` and `unit_dependencies.rs` for changed unit identity.
3. `compiler/fingerprint/` for freshness inputs and invalidation reasons.
4. `compiler/layout.rs` for paths, locks, and target-directory selection.
5. `compiler/build_runner/` and `job_queue/` for execution/scheduling issues.

## 6. Diagnostics, Output, and Utilities

`src/diagnostics/` contains diagnostic passes and rules that inspect manifests,
compiler messages, and future incompatibilities. `diagnostics/rules/` holds
individual lint-like rules; `diagnostics/mod.rs` contains shared rendering and
diagnostic helpers.

`src/util/machine_message.rs` defines structured JSON/message-format output.
`src/util/log_message.rs` and `src/util/logger.rs` support build logs and
diagnostic recording. `cargo-util-terminal` supplies terminal rendering,
colors, progress bars, and report formatting.

`src/util/errors.rs` defines Cargo's result/error conventions, including
`CargoResult`, `CliResult`, exit codes, internal errors, and already-printed
errors. `src/lib.rs` ultimately renders the error chain.

Other utility areas are intentionally broad:

- `util/command_prelude.rs`: shared command argument builders and
  `ArgMatches` helpers. This is the bridge between command files and ops.
- `util/flock.rs`, `cache_lock.rs`, and `lockserver.rs`: filesystem/cache
  locking and cross-process coordination.
- `util/job.rs`: jobserver setup and parallelism limits.
- `util/vcs/`: Git, Mercurial, Fossil, and Pijul repository operations.
- `util/rustc.rs`: locating and interrogating rustc/rustdoc.
- `util/graph.rs`, `dependency_queue.rs`, and `queue.rs`: generic graph and
  queue data structures used by resolution/build logic.
- `util/frontmatter.rs`: embedded Rust-script manifest front matter.
- `util/restricted_names.rs`: validation of names that are unsafe or invalid
  for package/target use.
- `util/sqlite.rs`: SQLite-backed cache support.
- `util/auth/`, `credential/`, and `network/`: external service integration,
  described above.

Do not assume a bug in printed output belongs to the command module. First
identify whether it is terminal rendering, machine-message serialization,
diagnostic generation, or the operation's data itself.

## 7. Supporting Workspace Crates

The root `Cargo.toml` includes `crates/*`, `credential/*`, and benchmark
packages. The most important supporting crates are:

| Location | Responsibility |
| --- | --- |
| `crates/cargo-util` | General filesystem, process, locking, and support code shared by Cargo and tests. |
| `crates/cargo-util-terminal` | Terminal styles, progress, reports, and shell output. |
| `crates/cargo-util-schemas` | Serde/TOML schemas for manifests and Cargo data formats. |
| `crates/cargo-platform` | Parsing and evaluating `cfg`/target platform expressions. |
| `crates/crates-io` | Access to the crates.io API. |
| `crates/rustfix` | In-memory representation and application of rustc suggestions. |
| `crates/cargo-test-support` | Test fixture builders, fake registries, process assertions, and test environment setup. |
| `crates/cargo-test-macro` | `#[cargo_test]` procedural macro used by the integration suite. |
| `crates/resolver-tests` | Dedicated resolver test package. |
| `crates/build-rs` | API used by build scripts. |
| `crates/mdman` | Generates man pages from Markdown. |
| `crates/home` | Shared home-directory definitions. |
| `crates/xtask-*` | Repository maintenance tasks such as docs, spellchecking, stale labels, and version/bump checks. |
| `crates/semver-check` | Compatibility/version checks used by repository tooling. |

Read each crate's `Cargo.toml` for its exact public purpose and dependencies.
Most supporting crates have their own `src/lib.rs`; the root crate imports
them through workspace path dependencies declared in the root manifest.

### Credential providers

`credential/cargo-credential/` defines the credential-provider library and
protocol helpers. Platform/provider implementations are in:

- `credential/cargo-credential-libsecret/`: Linux GNOME libsecret.
- `credential/cargo-credential-macos-keychain/`: macOS Keychain.
- `credential/cargo-credential-wincred/`: Windows Credential Manager.
- `credential/cargo-credential-1password/`: 1Password integration.

These packages are separate so Cargo can select platform-specific dependencies
without putting every operating system implementation in the core module.

## 8. Tests and Fixtures

`tests/testsuite/` is Cargo's large end-to-end integration suite. Each Rust
file generally exercises Cargo as an external process in a temporary project,
then asserts stdout, stderr, filesystem state, artifacts, or exit codes.

Start with:

- `tests/testsuite/main.rs`: test module registration and suite setup.
- `tests/testsuite/utils/mod.rs`: Cargo process construction helpers.
- `tests/testsuite/utils/ext.rs`: common test extensions and assertions.
- `tests/testsuite/utils/tools.rs`: fake tools and compiler/test fixtures.
- `crates/cargo-test-support/`: the lower-level fixture framework.
- `tests/testsuite/build.rs`: compilation behavior.
- `tests/testsuite/workspaces.rs`: workspace discovery/member behavior.
- `tests/testsuite/registry.rs`, `path.rs`, `git.rs`, and `vendor.rs`: source behavior.
- `tests/testsuite/lockfile_*.rs`, `update.rs`, and `minimal_versions.rs`: resolution and lockfiles.
- `tests/testsuite/build_script*.rs`: build-script environment and output.
- `tests/testsuite/profile_*.rs`: profile and compiler configuration.
- `tests/testsuite/messages.rs` and `message_format.rs`: output contracts.
- `tests/testsuite/script/`: embedded Cargo script/front-matter fixtures.

The `.stdout`, `.stderr`, `.term.svg`, and other fixture files under test
directories are expected outputs, not implementation code. When changing user
visible output, search for the matching snapshot/test before modifying the
implementation.

`tests/build-std/main.rs` is a separate build-std test/utility entry point.

## 9. Repository and Build Files

- `Cargo.toml`: root package plus workspace membership, features, and shared
  dependency versions.
- `Cargo.lock`: locked dependencies for this repository.
- `build.rs`: generated version/commit metadata, host target information, and
  the embedded Windows manifest configuration.
- `.github/workflows/`: CI, release, contributor, and audit workflows.
- `CHANGELOG.md`: release history.
- `typos.toml`: spelling checks.
- `triagebot.toml`: project automation configuration.
- `windows.manifest.xml`: Windows executable manifest.
- `LICENSE-*`: licensing and third-party notices.
- `doc/` if present: user/contributor/man-page documentation sources.

## 10. Issue-to-Component Routing

Use this table to choose where to look first when you see a symptom:

| Symptom | First files/directories |
| --- | --- |
| Cargo rejects a CLI flag or subcommand | `src/bin/cargo/cli.rs`, `src/bin/cargo/commands/<command>.rs`, `src/util/command_prelude.rs` |
| Alias or external `cargo-*` command is wrong | `src/bin/cargo/main.rs`, `src/bin/cargo/cli.rs` |
| Wrong directory/workspace/package selected | `src/workspace/workspace.rs`, `src/workspace/parser/`, command `exec`, `util/command_prelude.rs` |
| `Cargo.toml` parsing or validation error | `src/workspace/parser/`, `src/workspace/manifest.rs`, `src/diagnostics/` |
| Dependency version conflict or wrong feature | `src/ops/resolve.rs`, `src/resolver/`, `src/workspace/dependency.rs`, `src/workspace/features.rs` |
| `Cargo.lock` unexpectedly changes | `src/ops/lockfile.rs`, `src/ops/resolve.rs`, `src/resolver/encode.rs`, `src/resolver/version_prefs.rs` |
| Registry/Git/path dependency cannot be found | `src/sources/`, `src/workspace/registry.rs`, `src/util/network/`, `src/context/` |
| Download, proxy, offline, or retry failure | `src/util/network/`, source implementation, `src/context/mod.rs` |
| Authentication or token storage failure | `src/ops/registry/`, `src/util/auth/`, `src/util/credential/`, `credential/` |
| Build runs the wrong target or rustc flags | `src/ops/cargo_compile/`, `src/compiler/build_config.rs`, `unit.rs`, `profiles.rs` |
| Build recompiles unexpectedly | `src/compiler/fingerprint/`, `layout.rs`, `unit.rs`, `build_runner/` |
| Build hangs, races, or uses wrong parallelism | `src/compiler/job_queue/`, `src/compiler/build_runner/`, `src/util/job.rs`, locking modules |
| Build script output/env/linking is wrong | `src/compiler/custom_build.rs`, `src/compiler/links.rs`, build-script tests |
| rustc/rustdoc process invocation is wrong | `src/compiler/mod.rs`, `build_context/`, `rustdoc.rs`, `util/rustc.rs` |
| `cargo run` or `cargo test` launches wrong executable | `src/ops/cargo_run.rs`, `src/ops/cargo_test.rs`, command files, `compiler/compilation.rs` |
| Artifact path or target directory is wrong | `src/compiler/layout.rs`, workspace target-dir handling, `src/ops/cargo_clean.rs` |
| Error text, color, JSON, or progress output is wrong | `src/diagnostics/`, `src/util/machine_message.rs`, `src/util/errors.rs`, `cargo-util-terminal` |
| `cargo fix` does not apply suggestions | `src/ops/cargo_fix.rs`, `crates/rustfix/`, diagnostics/message parsing |
| Manifest editing (`add`/`remove`) is wrong | `src/workspace/editor/`, `src/ops/cargo_add.rs`, `src/ops/cargo_remove.rs` |
| Publish/package/vendor behavior is wrong | `src/ops/cargo_package.rs`, `src/ops/cargo_vendor.rs`, `src/ops/registry/`, source modules |
| A behavior regression is unclear | Find the narrowest `tests/testsuite/<topic>.rs`, then follow its command into the pipeline above |

## 11. End-to-End Example: `cargo build`

For a concrete trace, use this call chain:

```text
src/bin/cargo/main.rs
  -> src/bin/cargo/cli.rs::main
     -> commands/build.rs::exec
        -> ArgMatchesExt::workspace
           -> workspace/parser + workspace/workspace
        -> ArgMatchesExt::compile_options
        -> ops::compile
           -> ops/cargo_compile::compile_with_exec
              -> ops::resolve::resolve_ws_with_opts
                 -> workspace/registry + sources + resolver
              -> unit_generator + unit_graph
              -> compiler::BuildContext
              -> compiler::BuildRunner
                 -> fingerprint
                 -> job_queue
                 -> rustc/build scripts via DefaultExecutor
              -> compiler::Compilation
```

The same front end is reused by `check`, `doc`, `run`, `test`, and `bench`, but
each command changes `UserIntent`, target filters, profile, and post-build
behavior. That is why a bug that appears in multiple commands is usually in
`ops/cargo_compile`, `compiler`, resolution, or workspace loading rather than
in all of the command files independently.

## 12. Practical Investigation Method

When debugging an issue:

1. Reproduce it with the smallest command and package possible.
2. Locate the command's `exec` function in `src/bin/cargo/commands/`.
3. Identify the first `ops` function it calls.
4. Determine which data boundary is wrong: CLI options, `GlobalContext`,
   `Workspace`, `Resolve`, `Unit`, fingerprint, process command, or output.
5. Read the implementation of that boundary and its focused tests.
6. Search `tests/testsuite/` for the user-facing behavior before changing code.
7. Add or update a focused end-to-end test when the behavior is externally
   observable; add a unit test when the issue is isolated to an algorithm or
   data structure.

The most important boundaries to keep in mind are:

- `ArgMatches` -> typed command options.
- manifest text -> `Manifest`/`Package`/`Workspace`.
- source metadata -> `Summary`.
- dependency constraints -> `Resolve`.
- resolved packages/targets -> compiler `Unit` graph.
- unit graph + fingerprints -> executed jobs.
- compiler output -> `Compilation` and user-facing messages.
