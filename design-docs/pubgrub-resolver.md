# PubGrub Resolver for Cargo (`-Zpubgrub-resolver`) — Design & Handoff

> Status: **experimental, working for fresh full-graph resolution.** This document
> is a handoff for the next agent/engineer. It captures the architecture, the
> hard-won correctness insights, how to build/test, what is verified, and the
> prioritized next steps.

Branch: `pubgrub` (off `rust-lang/cargo` master).

---

## 1. Goal

Replace Cargo's hand-rolled backtracking dependency resolver with one built on
the [`pubgrub`](https://crates.io/crates/pubgrub) v0.4 crate, **side by side**
with the existing resolver, gated behind the unstable flag
`-Zpubgrub-resolver`. When the flag is off, resolution is completely unchanged.

Initial acceptance bar: **resolve Cargo's own dependency tree** and produce a
lockfile identical to the default resolver.

---

## 2. Current status (verified)

- `cargo -Zpubgrub-resolver generate-lockfile`, run **fresh with no pre-existing
  `Cargo.lock`**, produces a **byte-identical** lockfile to the default
  resolver for Cargo's own ~5944-line dependency tree (`diff` = 0 lines).
- Validation in `crates/resolver-tests`:
  - `pubgrub_smoke.rs` — basic end-to-end (2 tests).
  - `pubgrub_validated.rs` — SAT-validated scenarios: features, `dep:`/`dep/feat`,
    incompatible majors, links conflicts, diamonds, missing deps (11 tests).
  - `pubgrub_graph.rs` — **graph/edge** comparison vs the default resolver,
    including regressions for the cycle and weak-dependency cases (3 tests).
  - `pubgrub_prop.rs` — property test vs the SAT reference resolver over 256
    randomly generated registries.
  - **Curated suite via `CARGO_TEST_PUBGRUB=1`** — the harness convenience
    helpers route through `-Zpubgrub-resolver` when this env var is set, so the
    pre-existing curated suites run on PubGrub:
    - `tests/resolve.rs`: **37/37 pass**.
    - `tests/pubgrub.rs`: **28/28 pass** (weak deps, feature unification, cyclic
      features — all SAT-validated where applicable).
    - Two `resolve.rs` tests have their *exact error-text* assertions gated off
      under PubGrub (it uses its own derivation-tree formatter); the resolution
      *outcome* is identical.

### Caveats on the verification
- Parity is verified against the **current crates.io index state**; index drift
  changes selected versions for both resolvers.
- Parity is verified for **`generate-lockfile` (fresh)** only. The
  conservative-update paths (`cargo update -p`, building against an existing
  lock, `--precise`) are **not** yet exercised/verified.
- The package-set/SAT tests do **not** check graph edges; only `pubgrub_graph.rs`
  does. Edge correctness is where the subtle bugs lived (see §6).

---

## 3. How to build & test (IMPORTANT)

This workspace needs OpenSSL/curl/libgit2 from a Nix dev shell. **All** cargo
commands must run inside it:

```sh
nix develop ~/dev/dotfiles#cargo --command bash -c '<cargo command>'
```

Common commands:

```sh
# Build the library
nix develop ~/dev/dotfiles#cargo --command bash -c 'cargo build -p cargo --lib'

# Build the cargo binary (needed for real lockfile tests)
nix develop ~/dev/dotfiles#cargo --command bash -c 'cargo build --bin cargo'

# Unit tests for the semver conversion
nix develop ~/dev/dotfiles#cargo --command bash -c 'cargo test -p cargo --lib core::resolver::pubgrub'

# Resolver-test suites
nix develop ~/dev/dotfiles#cargo --command bash -c \
  'cargo test -p resolver-tests --test pubgrub_graph --test pubgrub_validated --test pubgrub_smoke'

# Property test (slow, ~60-70s)
nix develop ~/dev/dotfiles#cargo --command bash -c 'cargo test -p resolver-tests --test pubgrub_prop'

# Re-run the ENTIRE curated suite through the PubGrub resolver
nix develop ~/dev/dotfiles#cargo --command bash -c \
  'CARGO_TEST_PUBGRUB=1 cargo test -p resolver-tests --test resolve --test pubgrub'
```

### Reproducing the full-graph parity check (the real acceptance test)
```sh
nix develop ~/dev/dotfiles#cargo --command bash -c '
  cargo build --bin cargo
  CARGO=$(pwd)/target/debug/cargo
  git checkout -- Cargo.lock
  rm -f Cargo.lock; $CARGO generate-lockfile >/dev/null 2>&1; cp Cargo.lock /tmp/fd.lock
  rm -f Cargo.lock; $CARGO -Zpubgrub-resolver generate-lockfile >/dev/null 2>&1; cp Cargo.lock /tmp/fp.lock
  git checkout -- Cargo.lock
  diff /tmp/fd.lock /tmp/fp.lock && echo IDENTICAL
'
```
> Always `rm -f Cargo.lock` before *each* resolver run. If a lock is present it
> seeds `version_prefs` and masks fresh-resolution bugs (this exact mistake led
> to a false "it works" claim early on).

---

## 4. Architecture

### 4.1 Dispatch (the only fork point)
`src/cargo/core/resolver/mod.rs::resolve()` checks
`gctx.cli_unstable().pubgrub_resolver` and, if set, calls
`pubgrub::resolve(...)` with the identical signature. Flag is declared in
`src/cargo/core/features.rs` (`unstable_cli_options!` + parse arm
`"pubgrub-resolver"`). The single upstream call site is
`src/cargo/ops/resolve.rs` (~line 505), unchanged.

### 4.2 Module layout — `src/cargo/core/resolver/pubgrub/`

| File | Responsibility |
|---|---|
| `mod.rs` | Entry `resolve()`. Builds `RegistryQueryer`, the `Root`s from workspace members + their requested features, the `Provider`, runs `pubgrub::resolve(Provider, Root, 0.0.0)`, then reconstructs via `solution`. Translates `PubGrubError` (stashed real errors take precedence over `NoSolution`). |
| `semver_pubgrub.rs` | `SemverPubgrub`: a `pubgrub::VersionSet` over `semver::Version`. Ported & specialized from the `semver-pubgrub` crate, adapted to published pubgrub 0.4 (`Range`/`VersionSet`). Bug-for-bug compatible with `VersionReq::matches`. Also `SemverCompatibility` (the compat-bucket enum) + `only_one_compatibility_range`, `as_singleton`. |
| `package.rs` | `PubGrubPackage` — the encoding (see §5). Plus `FeatureNamespace`, `BucketName`, `WideName`, and `OptVersionReq -> SemverPubgrub` conversion. |
| `provider.rs` | `Provider`: implements `pubgrub::DependencyProvider`. Wraps Cargo's async `RegistryQueryer` with a **blocking** poll loop. `choose_version`, `prioritize`, `get_dependencies` (the big translation from `Summary`/`Dependency`/`FeatureValue` into the encoding). |
| `solution.rs` | `into_resolve`: projects pubgrub's `SelectedDependencies` back into a Cargo `Resolve` (graph nodes, edges, features, checksums, replacements). Reuses the default resolver's `check_cycles` / `check_duplicate_pkgs_in_lockfile`. |

### 4.3 Data flow
```
ops::resolve  →  resolver::resolve  --flag-->  pubgrub::resolve
                                                   │
              RegistryQueryer (async, poll)        │ build Roots from (Summary, ResolveOpts)
                     ▲ blocking bridge             ▼
              Provider: DependencyProvider  ──►  pubgrub::resolve(Root, 0.0.0)
                                                   │ SelectedDependencies<PubGrubPackage, Version>
                                                   ▼
                               solution::into_resolve  →  Resolve  →  Cargo.lock
```

---

## 5. The encoding (the crux)

PubGrub selects **one version per package**. Cargo needs (a) the same crate at
multiple semver-incompatible versions and (b) feature unification. We encode
both into a richer package identity (`PubGrubPackage`), adapted from
`Eh2406/pubgrub-crates-benchmark`'s `Names` enum, extended to carry `SourceId`
(Cargo has multiple sources) and to own its data:

- `Root` — synthetic; its deps are the workspace members.
- `Bucket { name: (crate, source, SemverCompatibility), member, all_features }`
  — a concrete crate within one compat bucket. Distinct buckets coexist ⇒
  incompatible majors allowed. `member` ⇒ include dev-deps. `all_features` ⇒
  enable every feature (lockfile pass).
- `BucketFeatures { bucket, FeatureNamespace }` — "this feature (Feat) or
  optional-dep activation (Dep) is enabled". Feature unification falls out of
  version solving over these virtual packages.
- `BucketDefaultFeatures { bucket }` — default features enabled.
- `Wide { name, req, from, from_compat }` (+ `WideFeatures`,
  `WideDefaultFeatures`) — used when a requirement could span **multiple** compat
  buckets (rare; e.g. `>=1, <3`). Defers bucket choice to a second step.
- `Links { links }` — enforces global uniqueness of a `links` value.

`semver::Version` is used directly as pubgrub's `V` (it already implements
`Ord/Clone/Debug/Display`). pubgrub 0.4's `Package` trait needs only
`Clone+Eq+Hash+Debug+Display` (no `Ord`), so `PubGrubPackage` does not implement
`Ord`.

### Key encoding rules in `get_dependencies` (provider.rs)
- A feature/default-feature package depends on its `Bucket` pinned to the same
  exact version (singleton range) → ties feature selection to the crate version.
- `Bucket` with `all_features` enables every key in `summary.features()` (the
  feature map already contains implicit features for optional deps).
- Optional deps are pulled in only via `BucketFeatures{Dep(..)}` packages, except
  in the `all_features` bucket.
- **Weak dep features (`dep?/feat`)**: still activate the optional dependency
  (record the edge); the `weak` flag only suppresses enabling the dep's own
  *implicit feature*. This mirrors Cargo's v1 lock resolver — see §6.

### Reconstruction rules in `solution.rs`
- Real nodes = `Bucket` packages → `PackageId(name, version, source)`.
- A package's enabled features = the `Feat(..)` + `default` activations in the
  solution.
- Edges: for each resolved package, walk its `summary.dependencies()`; include an
  edge when:
  - dev-dependency: only if the package is a workspace `member`;
  - optional: only if activated (`BucketFeatures{Dep(name_in_toml)}` present);
  - otherwise (normal/build, non-optional): always.
- The child version is found via `from_dep` (re-derives the bucket; for `Wide`
  packages it reads the chosen bucket from the solution).

---

## 6. Hard-won correctness insights (READ THIS)

These cost real debugging time; do not regress them.

1. **Workspace members are not in the registry.** They are provided directly.
   The provider seeds its version cache with the root summaries in
   `Provider::new`; otherwise `choose_version` queries the registry for a member
   and finds nothing → immediate `NoSolution`.

2. **Cargo's lockfile graph is activation-gated, NOT feature-agnostic.** An
   optional-dependency edge appears only if the optional dep is activated.
   Drawing edges for any present optional dep (a tempting "fix") creates cycles
   such as `schemars → url` and fails `check_cycles`.

3. **Weak dependency features still create the edge.** Cargo's v1 lock resolver
   (`dep_cache.rs::Requirements::require_dep_feature`) runs
   `self.deps.entry(package).or_default().insert(feat)` **unconditionally** — so a
   `dep?/feat` reference in an *enabled* feature records the optional dependency
   in the lock graph. The `weak` flag only gates whether the dep's own implicit
   feature is enabled. Example: bstr's `std = ["serde?/std"]` causes
   `bstr → serde` to appear in the lock even though `serde` is never
   feature-activated (confirmed: even `cargo tree --all-features` shows bstr
   without `serde`, yet the lock has the edge). The v1 lock resolver is a
   deliberately coarse over-approximation; the precise feature resolver
   (`features.rs::FeatureResolver`) refines features at build time. **We are
   replacing the v1 lock resolver, so we must match its coarse behavior.**

4. **The SAT/scenario tests do not check edges.** They validate the package set
   and feature set. The cycle and weak-dep bugs only showed up via full-lockfile
   diff and the new `pubgrub_graph.rs`. Always add edge-level tests for graph
   bugs.

5. **Always `rm -f Cargo.lock` before a fresh-resolution test.** A present lock
   seeds `version_prefs` and hides bugs.

### How I root-caused #3 (technique worth reusing)
Temporarily instrumented `dep_cache.rs::resolve_features` to print, for a target
crate (env-gated), `parent`, `opts.features`, and `reqs.deps`. Running the
**default** resolver showed `serde` in bstr's `reqs.deps` despite features being
only `{std, unicode}` → pointed straight at `serde?/std`. (Instrumentation has
been removed; re-add ad hoc if needed.)

---

## 7. Reference material reused

- `pubgrub-rs/semver-pubgrub` — source ported/specialized into
  `semver_pubgrub.rs` (it targets pubgrub's git `dev` branch; adapted to
  published 0.4). MPL-2.0 — note for upstreaming.
- `Eh2406/pubgrub-crates-benchmark` — the `Names` encoding + `DependencyProvider`
  shape was the model for `package.rs`/`provider.rs`. Also a ready-made harness
  to resolve thousands of real crates with both resolvers (great for §8.4).
- pubgrub 0.4 published API notes: `DependencyConstraints<P,VS>` is a `Vec`
  newtype (build via `FromIterator`; no `entry`/`insert`).
  `SelectedDependencies` has `iter()`/`get()`. `Dependencies::{Available,
  Unavailable}`. `Range` is re-exported from `version-ranges` 0.1.

---

## 8. Known limitations / open questions

- **Conservative updates unverified.** `cargo update -p`, `--precise`, and
  building against an existing lock flow through `version_prefs` differently and
  are untested. (`choose_version` already iterates `version_prefs`-sorted
  candidates, so lockfile preference *should* work, but prove it.)
- **`[patch]`/`[replace]`** handled only insofar as `RegistryQueryer` applies
  them; not specifically tested.
- **Error reporting** is a thin wrapper over pubgrub's `DefaultStringReporter`,
  not Cargo-native messages.
- **Performance** is not tuned: blocking poll loop in `Provider::candidates`, no
  reuse of the provider across Cargo's two resolve passes, `RefCell` caches.
- **`Wide` packages** (multi-bucket requirements) are implemented but lightly
  exercised; most real reqs are single-bucket.
- **`features` map fidelity** in the reconstructed `Resolve` is approximate
  (Feat names + `default`); the lockfile itself doesn't store features, but
  downstream `cargo build` feature unification reads this map — verify it.
- **Public/private deps, artifact (bindeps), platform `cfg` deps** not
  specifically validated.

---

## 9. Prioritized next steps

1. ~~Run `tests/resolve.rs` through pubgrub.~~ **DONE** via `CARGO_TEST_PUBGRUB`
   (see §2/§3). `resolve.rs` 37/37, `pubgrub.rs` 28/28. Next: extend the switch
   to also run the proptests and the full `cargo test -p resolver-tests` under
   PubGrub in CI.
2. **Scale the property test** (bump cases way up; loop it). It is the Cargo
   team's de-facto correctness gate.
3. **Verify conservative-update paths**: existing-lock reuse, `cargo update -p`,
   `--precise`. Add tests that resolve, mutate one dep, and re-resolve.
4. **Real-world differential testing** via `Eh2406/pubgrub-crates-benchmark` —
   resolve many crates.io crates with both resolvers and diff.
5. **Weak-dep + feature-map fidelity** — stress more `dep?/feat` shapes and
   confirm the `Resolve.features` map matches the default resolver, not just the
   lockfile graph.
6. **Cargo-native error reporting** from the derivation tree (only after
   correctness is locked down).
7. **Performance** — defer until correctness is solid.

---

## 10. Commit history (this branch)

```
c916af4f5 fix(resolver): Match v1 lock graph for weak dependency features
cacdd97e9 docs(unstable): Document -Zpubgrub-resolver flag
1f17605b3 test(resolver): Add pubgrub vs SAT property test
c83889704 fix(resolver): Record feature-agnostic dependency edges in pubgrub lock
6d49e8644 test(resolver): Add SAT-validated pubgrub resolution suite
eb917c1f7 fix(resolver): Seed workspace members into the pubgrub version cache
913116cbb feat(resolver): Wire up pubgrub resolution and reconstruct Resolve
f1d92a2d1 feat(resolver): Implement pubgrub DependencyProvider over the registry
bc8028b86 feat(resolver): Add PubGrubPackage encoding for the pubgrub resolver
083c0686a feat(resolver): Add semver-to-pubgrub VersionSet conversion
9fa0e7f75 feat(resolver): Add -Zpubgrub-resolver flag and module skeleton
```

> Note on history: commit `c83889704` ("feature-agnostic edges") was a wrong
> turn; it is corrected by `c916af4f5`. The current `solution.rs`/`provider.rs`
> reflect the corrected (activation-gated + weak-records-edge) behavior.

---

## 11. Quick orientation for the next agent

- Start in `src/cargo/core/resolver/pubgrub/mod.rs`, then `provider.rs`
  (`get_dependencies` is the heart), then `solution.rs`.
- To debug an edge mismatch: instrument `dep_cache.rs::resolve_features`
  (default resolver) and `solution.rs` (pubgrub) for a target crate, compare.
- The acceptance command is in §3; remember `rm -f Cargo.lock` each run.
- Before claiming "it works," test **fresh** (no lock) and **diff the full
  lockfile**, not just exit codes.
