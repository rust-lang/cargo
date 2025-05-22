# Changelog

## Cargo 1.89 (2025-08-07)
[873a0649...HEAD](https://github.com/rust-lang/cargo/compare/873a0649...HEAD)

### Added

- Add `*` and `?` pattern support for SSH known hosts matching.
  [#15508](https://github.com/rust-lang/cargo/pull/15508)

### Changed

- ❗️ `cargo fix` and `cargo clippy --fix` now run only on the default Cargo
  targets by default, matching the behavior of `cargo check`. To run on all
  Cargo targets, use the `--all-targets` flag. This change aligns the behavior
  with other commands. Edition flags like `--edition` and `--edition-idioms`
  remain implying `--all-targets` by default.
  [#15192](https://github.com/rust-lang/cargo/pull/15192)
- Respect `Retry-After` header for HTTP 429 responses when talking to registries.
  [#15463](https://github.com/rust-lang/cargo/pull/15463)
- Improved error message for the `CRATE[@<VER>]` argument prefixed with `v`.
  [#15484](https://github.com/rust-lang/cargo/pull/15484)
- Improved error message for the `CRATE[@<VER>]` argument with invalid package
  name characters.
  [#15441](https://github.com/rust-lang/cargo/pull/15441)
- cargo-add: suggest similarly named features 
  [#15438](https://github.com/rust-lang/cargo/pull/15438)

### Fixed

### Nightly only

- 🔥 `-Zno-embed-metadata`: This tells Cargo to pass the `-Zembed-metadata=no`
  flag to the compiler, which instructs it not to embed metadata within rlib
  and dylib artifacts. In this case, the metadata will only be stored in
  `.rmeta` files.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#no-embed-metadata))
  [#15378](https://github.com/rust-lang/cargo/pull/15378)
- `-Zscript`: Make cargo script ignore workspaces.
  [#15496](https://github.com/rust-lang/cargo/pull/15496)
- `-Zpackage-workspace`: keep dev-dependencies if they have a version.
  [#15470](https://github.com/rust-lang/cargo/pull/15470)

### Documentation

- Clarify what commands need and remove confusing example
  [#15457](https://github.com/rust-lang/cargo/pull/15457)
- Update fingerprint footnote
  [#15478](https://github.com/rust-lang/cargo/pull/15478)
- home: update version notice for deprecation removal 
  [#15511](https://github.com/rust-lang/cargo/pull/15511)

### Internal

- Refactor artifact deps in FeatureResolver::deps
  [#15492](https://github.com/rust-lang/cargo/pull/15492)
- Added tracing spans for rustc invocations
  [#15464](https://github.com/rust-lang/cargo/pull/15464)
- ci: migrate renovate config 
  [#15501](https://github.com/rust-lang/cargo/pull/15501)
- ci: Require schema job to pass
  [#15504](https://github.com/rust-lang/cargo/pull/15504)
- test: Remove unused nightly requirements
  [#15498](https://github.com/rust-lang/cargo/pull/15498)
- Update dependencies.
  [#15456](https://github.com/rust-lang/cargo/pull/15456)

## Cargo 1.88 (2025-06-26)
[a6c604d1...rust-1.88.0](https://github.com/rust-lang/cargo/compare/a6c604d1...rust-1.88.0)

### Added

- 🎉 Stabilize automatic garbage collection for global caches.

  When building, Cargo downloads and caches crates needed as dependencies.
  Historically, these downloaded files would never be cleaned up, leading to an
  unbounded amount of disk usage in Cargo's home directory. In this version,
  Cargo introduces a garbage collection mechanism to automatically clean up old
  files (e.g. .crate files). Cargo will remove files downloaded from the network
  if not accessed in 3 months, and files obtained from the local system if not
  accessed in 1 month. Note that this automatic garbage collection will not take
  place if running offline (using `--offline` or `--frozen`).

  Cargo 1.78 and newer track the access information needed for this garbage
  collection. If you regularly use versions of Cargo older than 1.78, in
  addition to running current versions of Cargo, and you expect to have some
  crates accessed exclusively by the older versions of Cargo and don't want to
  re-download those crates every ~3 months, you may wish to set
  `cache.auto-clean-frequency = "never"` in the Cargo configuration.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#cache))
  [#14287](https://github.com/rust-lang/cargo/pull/14287)
- Allow boolean literals as cfg predicates in Cargo.toml and configurations.
  For example, `[target.'cfg(not(false))'.dependencies]` is a valid cfg predicate.
  ([RFC 3695](https://github.com/rust-lang/rfcs/pull/3695))
  [#14649](https://github.com/rust-lang/cargo/pull/14649)

### Changed

- Don't canonicalize executable path for the `CARGO` environment variable.
  [#15355](https://github.com/rust-lang/cargo/pull/15355)
- Print target and package names formatted as file hyperlinks.
  [#15405](https://github.com/rust-lang/cargo/pull/15405)
- Make sure library search paths inside `OUT_DIR` precede external paths.
  [#15221](https://github.com/rust-lang/cargo/pull/15221)
- Suggest similar looking feature names when feature is missing.
  [#15454](https://github.com/rust-lang/cargo/pull/15454)
- Use `zlib-rs` for gzip (de)compression for `.crate` tarballs.
  [#15417](https://github.com/rust-lang/cargo/pull/15417)

### Fixed

- build-rs: Correct name of `CARGO_CFG_FEATURE`
  [#15420](https://github.com/rust-lang/cargo/pull/15420)
- cargo-tree: Make output more deterministic
  [#15369](https://github.com/rust-lang/cargo/pull/15369)
- cargo-package: dont fail the entire command when the dirtiness check failed,
  as git status check is mostly informational.
  [#15416](https://github.com/rust-lang/cargo/pull/15416)
  [#15419](https://github.com/rust-lang/cargo/pull/15419)

### Nightly only

- 🔥 `-Zrustdoc-depinfo`: A new unstable flag leveraging rustdoc’s dep-info
  files to determine whether documentations are required to re-generate.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#rustdoc-depinfo))
  [#15359](https://github.com/rust-lang/cargo/pull/15359)
  [#15371](https://github.com/rust-lang/cargo/pull/15371)
- `build-dir`: Added validation for unmatched brackets in build-dir template 
  [#15414](https://github.com/rust-lang/cargo/pull/15414)
- `build-dir`: Improved error message when build-dir template var is invalid 
  [#15418](https://github.com/rust-lang/cargo/pull/15418)
- `build-dir`: Added `build_directory` field to cargo metadata output 
  [#15377](https://github.com/rust-lang/cargo/pull/15377)
- `build-dir`: Added symlink resolution for `workspace-path-hash` 
  [#15400](https://github.com/rust-lang/cargo/pull/15400)
- `build-dir`: Added build_directory to cargo metadata documentation 
  [#15410](https://github.com/rust-lang/cargo/pull/15410)
- `unit-graph`: switch to Package ID Spec.
  [#15447](https://github.com/rust-lang/cargo/pull/15447)
- `-Zgc`: Rename the `gc` config table to `[cache]`.
  Low-level settings is now under `[cache.global-clean]`.
  [#15367](https://github.com/rust-lang/cargo/pull/15367)
- `-Zdoctest-xcompile`: Update doctest xcompile flags.
  [#15455](https://github.com/rust-lang/cargo/pull/15455)

### Documentation

- Mention the convention of kebab-case for Cargo targets naming.
  [#14439](https://github.com/rust-lang/cargo/pull/14439)
- Use better example value in `CARGO_CFG_TARGET_ABI` 
  [#15404](https://github.com/rust-lang/cargo/pull/15404)

### Internal

- Fix formatting of CliUnstable parsing
  [#15434](https://github.com/rust-lang/cargo/pull/15434)
- ci: restore semver-checks for cargo-util
  [#15389](https://github.com/rust-lang/cargo/pull/15389)
- ci: add aarch64 linux runner
  [#15077](https://github.com/rust-lang/cargo/pull/15077)
- rustfix: Use `snapbox` for snapshot testing
  [#15429](https://github.com/rust-lang/cargo/pull/15429)
- test:Prevent undeclared public network access 
  [#15368](https://github.com/rust-lang/cargo/pull/15368)
- Update dependencies.
  [#15373](https://github.com/rust-lang/cargo/pull/15373)
  [#15381](https://github.com/rust-lang/cargo/pull/15381)
  [#15391](https://github.com/rust-lang/cargo/pull/15391)
  [#15394](https://github.com/rust-lang/cargo/pull/15394)
  [#15403](https://github.com/rust-lang/cargo/pull/15403)
  [#15415](https://github.com/rust-lang/cargo/pull/15415)
  [#15421](https://github.com/rust-lang/cargo/pull/15421)
  [#15446](https://github.com/rust-lang/cargo/pull/15446)


## Cargo 1.87 (2025-05-15)
[ce948f46...rust-1.87.0](https://github.com/rust-lang/cargo/compare/ce948f46...rust-1.87.0)

### Added

- Add terminal integration via ANSI OSC 9;4 sequences via the
  `term.progress.term-integration` configuration field. This reports progress
  to the terminal emulator for display in places like the task bar.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#termprogressterm-integration))
  [#14615](https://github.com/rust-lang/cargo/pull/14615)
- Forward bash completions of third party subcommands 
  [#15247](https://github.com/rust-lang/cargo/pull/15247)
- cargo-tree: Color the output.
  [#15242](https://github.com/rust-lang/cargo/pull/15242)
- cargo-package: add `--exclude-lockfile` flag, which will stop verifying the
  lock file if present.
  [#15234](https://github.com/rust-lang/cargo/pull/15234)

### Changed

- ❗️ Cargo now depends on OpenSSL v3. This implies that Cargo in the official
  Rust distribution will have a hard dependency on libatomic on 32-bit platforms.
  [#15232](https://github.com/rust-lang/cargo/pull/15232)
- Report `<target>.edition` deprecation to users.
  [#15321](https://github.com/rust-lang/cargo/pull/15321)
- Leverage clap for providing default values for `--vcs`, `--color`, and `--message-format` flags.
  [#15322](https://github.com/rust-lang/cargo/pull/15322)
- Mention "3" as a valid value for "resolver" field in error message 
  [#15215](https://github.com/rust-lang/cargo/pull/15215)
- Uplift windows Cygwin DLL import libraries 
  [#15193](https://github.com/rust-lang/cargo/pull/15193)
- Include the package name also in the target hint message.
  [#15199](https://github.com/rust-lang/cargo/pull/15199)
  [#15497](https://github.com/rust-lang/cargo/pull/15497)
- cargo-add: collapse large feature lists
  [#15200](https://github.com/rust-lang/cargo/pull/15200)
- cargo-vendor: Add context which workspace failed to resolve
  [#15297](https://github.com/rust-lang/cargo/pull/15297)

### Fixed

- Do not pass cdylib link args from `cargo::rustc-link-arg-cdylib` to tests.
  [#15317](https://github.com/rust-lang/cargo/pull/15317)
  [#15326](https://github.com/rust-lang/cargo/pull/15326)
- Don't use `$CARGO_BUILD_TARGET` in `cargo metadata`.
  [#15271](https://github.com/rust-lang/cargo/pull/15271)
- Allow `term.progress.when` to have default values. `CARGO_TERM_PROGRESS_WIDTH`
  can now be correctly set even when other settings are missing.
  [#15287](https://github.com/rust-lang/cargo/pull/15287)
- Fix the `CARGO` environment variable setting for external subcommands
  pointing to the wrong Cargo binary path . Note that the environment variable
  is never designed as a general Cargo wrapper.
  [#15208](https://github.com/rust-lang/cargo/pull/15208)
- Fix some issues with future-incompat report generation.
  [#15345](https://github.com/rust-lang/cargo/pull/15345)
- Respect `--frozen` everywhere `--offline` or `--locked` is accepted.
  [#15263](https://github.com/rust-lang/cargo/pull/15263)
- cargo-package: report also the VCS status of the workspace manifest if dirty.
  [#15276](https://github.com/rust-lang/cargo/pull/15276)
  [#15341](https://github.com/rust-lang/cargo/pull/15341)
- cargo-publish: Fix man page with malformed `{{#options}}` block
  [#15191](https://github.com/rust-lang/cargo/pull/15191)
- cargo-run: Disambiguate bins from different packages that share a name.
  [#15298](https://github.com/rust-lang/cargo/pull/15298)
- cargo-rustc: de-duplicate crate types.
  [#15314](https://github.com/rust-lang/cargo/pull/15314)
- cargo-vendor: dont remove non-cached sources.
  [#15260](https://github.com/rust-lang/cargo/pull/15260)

### Nightly only

- 🔥 cargo-package: add unstable `--message-format` flag. The flag is providing
  an alternative JSON output format for file listing of the `--list` flag.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#package-message-format))
  [#15311](https://github.com/rust-lang/cargo/pull/15311)
  [#15354](https://github.com/rust-lang/cargo/pull/15354)
- 🔥 `build-dir`: the `build.build-dir` config option to set the directory where
  intermediate build artifacts will be stored.
  Intermediate artifacts are produced by Rustc/Cargo during the build process.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-dir))
  [#15104](https://github.com/rust-lang/cargo/pull/15104)
  [#15236](https://github.com/rust-lang/cargo/pull/15236)
  [#15334](https://github.com/rust-lang/cargo/pull/15334)
- 🔥 `-Zsbom`: The `build.sbom` config allows to generate so-called SBOM
  pre-cursor files alongside each compiled artifact.
  ([RFC 3553](https://github.com/rust-lang/rfcs/pull/3553))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#sbom))
  [#13709](https://github.com/rust-lang/cargo/pull/13709)
- 🔥 `-Zpublic-dependency`: new `--depth public` value for `cargo tree` to
  display public dependencies.
  [#15243](https://github.com/rust-lang/cargo/pull/15243)
- `-Zscript`: Handle more frontmatter parsing corner cases
  [#15187](https://github.com/rust-lang/cargo/pull/15187)
- `-Zpackage-workspace`: Fix lookups to capitalized workspace member's index entry 
  [#15216](https://github.com/rust-lang/cargo/pull/15216)
- `-Zpackage-workspace`: Register workspace member renames in overlay 
  [#15228](https://github.com/rust-lang/cargo/pull/15228)
- `-Zpackage-workspace`: Ensure we can package directories ending with '.rs' 
  [#15240](https://github.com/rust-lang/cargo/pull/15240)
- `native-completions`: add completions for `--profile`
  [#15308](https://github.com/rust-lang/cargo/pull/15308)
- `native-completions`: add completions for aliases
  [#15319](https://github.com/rust-lang/cargo/pull/15319)
- `native-completions`: add completions for `cargo add --path`
  [#15288](https://github.com/rust-lang/cargo/pull/15288)
- `native-completions`: add completions for `--manifest-path` 
  [#15225](https://github.com/rust-lang/cargo/pull/15225)
- `native-completions`: add completions for `--lockfile-path` 
  [#15238](https://github.com/rust-lang/cargo/pull/15238)
- `native-completions`: add completions for `cargo install --path`
  [#15266](https://github.com/rust-lang/cargo/pull/15266)
- `native-completions`: add completions fro `+<toolchain>`
  [#15301](https://github.com/rust-lang/cargo/pull/15301)

### Documentation

- Note that target-edition is deprecated 
  [#15292](https://github.com/rust-lang/cargo/pull/15292)
- Mention wrong URLs as a cause of git authentication errors 
  [#15304](https://github.com/rust-lang/cargo/pull/15304)
- Shift focus to resolver v3 
  [#15213](https://github.com/rust-lang/cargo/pull/15213)
- Lockfile is always included since 1.84 
  [#15257](https://github.com/rust-lang/cargo/pull/15257)
- Remove `Cargo.toml` from `package.include` in example 
  [#15253](https://github.com/rust-lang/cargo/pull/15253)
- Make it clearer that `rust_version` is enforced during compile
  [#15303](https://github.com/rust-lang/cargo/pull/15303)
- Fix `[env]` `relative` description in reference 
  [#15332](https://github.com/rust-lang/cargo/pull/15332)
- Add `unsafe` to `extern` while using build scripts in Cargo Book 
  [#15294](https://github.com/rust-lang/cargo/pull/15294)
- Mention `x.y.*` as a kind of version requirement to avoid. 
  [#15310](https://github.com/rust-lang/cargo/pull/15310)
- contrib: Expand the description of team meetings 
  [#15349](https://github.com/rust-lang/cargo/pull/15349)

### Internal

- Show extra build description from bootstrap via the `CFG_VER_DESCRIPTION` env var.
  [#15269](https://github.com/rust-lang/cargo/pull/15269)
- Control byte display precision with `std::fmt` options.
  [#15246](https://github.com/rust-lang/cargo/pull/15246)
- Replace humantime crate with jiff.
  [#15290](https://github.com/rust-lang/cargo/pull/15290)
- Dont check cargo-util semver until 1.86 is released 
  [#15222](https://github.com/rust-lang/cargo/pull/15222)
- Redox OS is part of the unix family 
  [#15307](https://github.com/rust-lang/cargo/pull/15307)
- cargo-tree: Abstract the concept of a NodeId 
  [#15237](https://github.com/rust-lang/cargo/pull/15237)
- cargo-tree: Abstract the concept of an edge 
  [#15233](https://github.com/rust-lang/cargo/pull/15233)
- ci: Auto-update cargo-semver-checks 
  [#15212](https://github.com/rust-lang/cargo/pull/15212)
- ci: Visually group output in Github 
  [#15218](https://github.com/rust-lang/cargo/pull/15218)
- manifest: Centralize Cargo target descriptions 
  [#15291](https://github.com/rust-lang/cargo/pull/15291)
- Update dependencies.
  [#15250](https://github.com/rust-lang/cargo/pull/15250)
  [#15249](https://github.com/rust-lang/cargo/pull/15249)
  [#15245](https://github.com/rust-lang/cargo/pull/15245)
  [#15224](https://github.com/rust-lang/cargo/pull/15224)
  [#15282](https://github.com/rust-lang/cargo/pull/15282)
  [#15211](https://github.com/rust-lang/cargo/pull/15211)
  [#15217](https://github.com/rust-lang/cargo/pull/15217)
  [#15268](https://github.com/rust-lang/cargo/pull/15268)

## Cargo 1.86 (2025-04-03)
[d73d2caf...rust-1.86.0](https://github.com/rust-lang/cargo/compare/d73d2caf...rust-1.86.0)

### Added

### Changed

- ❗️ When merging, replace rather than combine configuration keys that refer
  to a program path and its arguments.
  [#15066](https://github.com/rust-lang/cargo/pull/15066)  
  These keys include:
  - `registry.credential-provider`
  - `registries.*.credential-provider`
  - `target.*.runner`
  - `host.runner`
  - `credential-alias.*`
  - `doc.browser`
- ❗️ Error if both `--package` and `--workspace` are passed but the requested
  package is missing. This was previously silently ignored, which was considered
  a bug since missing packages should be reported.
  [#15071](https://github.com/rust-lang/cargo/pull/15071)
- Added warning when failing to update index cache.
  [#15014](https://github.com/rust-lang/cargo/pull/15014)
- Don't use "did you mean" in errors. Be upfront about what the suggestion is.
  [#15138](https://github.com/rust-lang/cargo/pull/15138)
- Provide a better error message for invalid SSH URLs in dependency sources.
  [#15185](https://github.com/rust-lang/cargo/pull/15185)
- Suggest similar feature names when the package doesn't have given features.
  [#15133](https://github.com/rust-lang/cargo/pull/15133)
- Print globs when workspace members can't be found.
  [#15093](https://github.com/rust-lang/cargo/pull/15093)
- cargo-fix: Make `--allow-dirty` imply `--allow-staged`
  [#15013](https://github.com/rust-lang/cargo/pull/15013)
- cargo-login: hide the `token` argument from CLI help for the preparation of deprecation.
  [#15057](https://github.com/rust-lang/cargo/pull/15057)
- cargo-login: Don't suggest `cargo login` when using incompatible credential providers.
  [#15124](https://github.com/rust-lang/cargo/pull/15124)
- cargo-package: improve the performance of VCS status check by matching certain
  path prefixes with pathspec.
  [#14997](https://github.com/rust-lang/cargo/pull/14997)

### Fixed

- The `rerun-if-env-changed` build script instruction can now correctly detect
  changes in the `[env]` configuration table.
  [#14756](https://github.com/rust-lang/cargo/pull/14756)
- Force emitting warnings as warnings when learning Rust target info for an unsupported crate type.
  [#15036](https://github.com/rust-lang/cargo/pull/15036)
- cargo-package: Verify the VCS status of symlinks
  when they point to paths outside the current package root.
  [#14981](https://github.com/rust-lang/cargo/pull/14981)

### Nightly only

- 🔥 `-Z feature-unification`: This new unstable flag enables the
  `resolver.feature-unification` configuration option to control how features
  are unified across a workspace.
  ([RFC 3529](https://github.com/rust-lang/rfcs/blob/master/text/3692-feature-unification.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#feature-unification))
  [#15157](https://github.com/rust-lang/cargo/pull/15157)
- cargo-util-schemas: Correct and update the JSON Schema
  [#15000](https://github.com/rust-lang/cargo/pull/15000)
- cargo-util-schemas: Fix the `[lints]` JSON Schema 
  [#15035](https://github.com/rust-lang/cargo/pull/15035)
- cargo-util-schemas: Fix 'metadata' JSON Schema 
  [#15033](https://github.com/rust-lang/cargo/pull/15033)
- `cargo rustc --print`: Setup cargo environment for `cargo rustc --print`.
  [#15026](https://github.com/rust-lang/cargo/pull/15026)
- `-Zbuild-std`: parse value as comma-separated list, also extends the behavior
  to `build-std-features`.
  [#15065](https://github.com/rust-lang/cargo/pull/15065)
- `-Zgc`: Make cache tracking resilient to unexpected files.
  [#15147](https://github.com/rust-lang/cargo/pull/15147)
- `-Zscript`: Consolidate creation of SourceId from manifest path 
  [#15172](https://github.com/rust-lang/cargo/pull/15172)
- `-Zscript`: Integrate cargo-script logic into main parser 
  [#15168](https://github.com/rust-lang/cargo/pull/15168)
- `-Zscript`: add `cargo pkgid` support for cargo-script 
  [#14961](https://github.com/rust-lang/cargo/pull/14961)
- `-Zpackage-workspace`: Report all unpublishable packages 
  [#15070](https://github.com/rust-lang/cargo/pull/15070)

### Documentation

- Document that Cargo automatically registers variables used in the `env!`
  macro to trigger rebuilds since 1.46.
  [#15062](https://github.com/rust-lang/cargo/pull/15062)
- Move the changelog to The Cargo Book.
  [#15119](https://github.com/rust-lang/cargo/pull/15119)
  [#15123](https://github.com/rust-lang/cargo/pull/15123)
  [#15142](https://github.com/rust-lang/cargo/pull/15142)
- Note `package.authors` is deprecated.
  [#15068](https://github.com/rust-lang/cargo/pull/15068)
- Fix the wrong grammar of a Package Id Specification.
  [#15049](https://github.com/rust-lang/cargo/pull/15049)
- Fix the inverted logic about MSRV
  [#15044](https://github.com/rust-lang/cargo/pull/15044)
- cargo-metadata: Fix description of the `"root"` field.
  [#15182](https://github.com/rust-lang/cargo/pull/15182)
- cargo-package: note the lock file is always included.
  [#15067](https://github.com/rust-lang/cargo/pull/15067)
- contrib: Start guidelines for schema design.
  [#15037](https://github.com/rust-lang/cargo/pull/15037)

### Internal

- Don't use `libc::LOCK_*` on Solaris.
  [#15143](https://github.com/rust-lang/cargo/pull/15143)
- Clean up field -> env var handling.
  [#15008](https://github.com/rust-lang/cargo/pull/15008)
- Simplify SourceID Ord/Eq.
  [#14980](https://github.com/rust-lang/cargo/pull/14980)
  [#15103](https://github.com/rust-lang/cargo/pull/15103)
- Add manual Hash impl for SourceKind and document the reason.
  [#15029](https://github.com/rust-lang/cargo/pull/15029)
- ci: allow Windows reserved names in CI 
  [#15135](https://github.com/rust-lang/cargo/pull/15135)
- cargo-test-macro: Remove condition on `RUSTUP_WINDOWS_PATH_ADD_BIN`
  [#15017](https://github.com/rust-lang/cargo/pull/15017)
- resolver: Simplify backtrack
  [#15150](https://github.com/rust-lang/cargo/pull/15150)
- resolver: Small cleanups 
  [#15040](https://github.com/rust-lang/cargo/pull/15040)
- test: Clean up shallow fetch tests
  [#15002](https://github.com/rust-lang/cargo/pull/15002)
- test: Fix `https::self_signed_should_fail` for macOS
  [#15016](https://github.com/rust-lang/cargo/pull/15016)
- test: Fix benchsuite issue with newer versions of git
  [#15069](https://github.com/rust-lang/cargo/pull/15069)
- test: Fix shared_std_dependency_rebuild running on Windows
  [#15111](https://github.com/rust-lang/cargo/pull/15111)
- test: Update tests to fix nightly errors
  [#15110](https://github.com/rust-lang/cargo/pull/15110)
- test: Remove unused `-C link-arg=-fuse-ld=lld`
  [#15097](https://github.com/rust-lang/cargo/pull/15097)
- test: Remove `unsafe` by using `LazyLock`
  [#15096](https://github.com/rust-lang/cargo/pull/15096)
- test: Remove unnecessary into conversions
  [#15042](https://github.com/rust-lang/cargo/pull/15042)
- test: Fix race condition in panic_abort_tests
  [#15169](https://github.com/rust-lang/cargo/pull/15169)
- Update deny.toml
  [#15164](https://github.com/rust-lang/cargo/pull/15164)
- Update dependencies.
  [#14995](https://github.com/rust-lang/cargo/pull/14995)
  [#14996](https://github.com/rust-lang/cargo/pull/14996)
  [#14998](https://github.com/rust-lang/cargo/pull/14998)
  [#15012](https://github.com/rust-lang/cargo/pull/15012)
  [#15018](https://github.com/rust-lang/cargo/pull/15018)
  [#15041](https://github.com/rust-lang/cargo/pull/15041)
  [#15050](https://github.com/rust-lang/cargo/pull/15050)
  [#15121](https://github.com/rust-lang/cargo/pull/15121)
  [#15128](https://github.com/rust-lang/cargo/pull/15128)
  [#15129](https://github.com/rust-lang/cargo/pull/15129)
  [#15162](https://github.com/rust-lang/cargo/pull/15162)
  [#15163](https://github.com/rust-lang/cargo/pull/15163)
  [#15165](https://github.com/rust-lang/cargo/pull/15165)
  [#15166](https://github.com/rust-lang/cargo/pull/15166)

## Cargo 1.85 (2025-02-20)
[66221abd...rust-1.85.0](https://github.com/rust-lang/cargo/compare/66221abd...rust-1.85.0)

### Added

- 🎉 Cargo now supports the 2024 edition.
  More information is available in the [edition guide](https://doc.rust-lang.org/nightly/edition-guide/rust-2024/index.html).
  [#14828](https://github.com/rust-lang/cargo/pull/14828)
- cargo-tree: The `--depth` flag now accepts `workspace`,
  which shows only dependencies that are members of the current workspace.
  [#14928](https://github.com/rust-lang/cargo/pull/14928)
- Build scripts now receive a new environment variable, `CARGO_CFG_FEATURE`,
  which contains each activated feature of the package being built.
  [#14902](https://github.com/rust-lang/cargo/pull/14902)
- perf: Dependency resolution is now faster due to a more efficient hash for `ActivationsKey`
  [#14915](https://github.com/rust-lang/cargo/pull/14915)

### Changed

- ❗️ cargo-rustc: Trailing flags now have higher precedence.
  This behavior was nightly-only since 1.83 and is now stabilized.
  [#14900](https://github.com/rust-lang/cargo/pull/14900)
- ❗️ Cargo now uses a cross-platform hash algorithm from `rustc-stable-hash`.
  As a result, the hash part of paths to dependency caches
  (e.g., `$CARGO_HOME/registry/index/index.crates.io-<hash>`) will change.
  This will trigger re-downloads of registry indices and `.crate` tarballs,
  as well as re-cloning of Git dependencies.
  [#14917](https://github.com/rust-lang/cargo/pull/14917)
- Added a future-incompatibility warning for keywords in `cfg`s in Cargo.toml
  and Cargo configuration. `cfg`s with keywords like `cfg(true)` and `cfg(false)`
  were incorrectly accepted. For backward compatibility, support for raw
  identifiers has been introduced; for example, use `cfg(r#true)` instead.
  [#14671](https://github.com/rust-lang/cargo/pull/14671)
- Dependency resolution now provides richer error messages explaining why some versions were rejected, unmatched, or invalid.  
  [#14897](https://github.com/rust-lang/cargo/pull/14897)
  [#14921](https://github.com/rust-lang/cargo/pull/14921)
  [#14923](https://github.com/rust-lang/cargo/pull/14923)
  [#14927](https://github.com/rust-lang/cargo/pull/14927)
- cargo-doc: improve the error message when `--open`ing a doc while no doc generated.
  [#14969](https://github.com/rust-lang/cargo/pull/14969)
- cargo-package: warn if symlinks checked out as plain text files 
  [#14994](https://github.com/rust-lang/cargo/pull/14994)
- cargo-package: Shows dirty file paths relative to the Git working directory.
  [#14968](https://github.com/rust-lang/cargo/pull/14968)
  [#14970](https://github.com/rust-lang/cargo/pull/14970)

### Fixed

- Set `GIT_DIR` to ensure compatibility with bare repositories
  for `net.git-fetch-with-cli=true`.
  [#14860](https://github.com/rust-lang/cargo/pull/14860)
- Fixed workspace Cargo.toml modification didn't invalidate build cache.
  [#14973](https://github.com/rust-lang/cargo/pull/14973)
- Prevented build caches from being discarded after changes to `RUSTFLAGS`.
  [#14830](https://github.com/rust-lang/cargo/pull/14830)
  [#14898](https://github.com/rust-lang/cargo/pull/14898)
- cargo-add: Don't select yanked versions when normalizing names.
  [#14895](https://github.com/rust-lang/cargo/pull/14895)
- cargo-fix: Migrate workspace dependencies to the 2024 edition also for virtual manifests.
  [#14890](https://github.com/rust-lang/cargo/pull/14890)
- cargo-package: Verify the VCS status of `package.readme` and `package.license-file`
  when they point to paths outside the current package root.
  [#14966](https://github.com/rust-lang/cargo/pull/14966)
- cargo-package: assure possibly blocking non-files (like FIFOs) won't be
  picked up for publishing.
  [#14977](https://github.com/rust-lang/cargo/pull/14977)

### Nightly only

- `path-bases`: Support bases in `[patch]`es in virtual manifests
  [#14931](https://github.com/rust-lang/cargo/pull/14931)
- `unit-graph`: Use the configured shell to print output.
  [#14926](https://github.com/rust-lang/cargo/pull/14926)
- `-Zbuild-std`: Check if the build target supports `std`
  by probing the `metadata.std` field in the target spec JSON.
  [#14183](https://github.com/rust-lang/cargo/pull/14183)
  [#14938](https://github.com/rust-lang/cargo/pull/14938)
  [#14899](https://github.com/rust-lang/cargo/pull/14899)
- `-Zbuild-std`: always link to std when testing proc-macros.
  [#14850](https://github.com/rust-lang/cargo/pull/14850)
  [#14861](https://github.com/rust-lang/cargo/pull/14861)
- `-Zbuild-std`: clean up build-std tests
  [#14943](https://github.com/rust-lang/cargo/pull/14943)
  [#14933](https://github.com/rust-lang/cargo/pull/14933)
  [#14896](https://github.com/rust-lang/cargo/pull/14896)
- `-Zbuild-std`: Hash relative paths to std workspace instead of absolute paths.
  [#14951](https://github.com/rust-lang/cargo/pull/14951)
- `-Zpackage-workspace`: Allow dry-run of a non-bumped workspace.
  [#14847](https://github.com/rust-lang/cargo/pull/14847)
- `-Zscript`: Allow adding/removing dependencies from cargo scripts
  [#14857](https://github.com/rust-lang/cargo/pull/14857)
- `-Zscript`: Migrate cargo script manifests across editions
  [#14864](https://github.com/rust-lang/cargo/pull/14864)
- `-Zscript`: Don't override the release profile.
  [#14925](https://github.com/rust-lang/cargo/pull/14925)
- `-Ztrim-paths`: Use `Path::push` to construct the `remap-path-prefix` flag.
  [#14908](https://github.com/rust-lang/cargo/pull/14908)

### Documentation

- Clarify how `cargo::metadata` env var is selected.
  [#14842](https://github.com/rust-lang/cargo/pull/14842)
- cargo-info: Remove references to the default registry in `cargo-info` docs
  [#14880](https://github.com/rust-lang/cargo/pull/14880)
- contrib: add missing argument to Rustup Cargo workaround 
  [#14954](https://github.com/rust-lang/cargo/pull/14954)
- SemVer: Add section on RPIT capturing 
  [#14849](https://github.com/rust-lang/cargo/pull/14849)

### Internal

- Add the `test` cfg as a well known cfg before of compiler change.
  [#14963](https://github.com/rust-lang/cargo/pull/14963)
- Enable triagebot merge conflict notifications
  [#14972](https://github.com/rust-lang/cargo/pull/14972)
- Limit release trigger to `0.*` tags
  [#14940](https://github.com/rust-lang/cargo/pull/14940)
- Simplify `SourceID` Hash.
  [#14800](https://github.com/rust-lang/cargo/pull/14800)
- build-rs: Automatically emits `rerun-if-env-changed`
  when accessing environment variables Cargo sets for build script executions.
  [#14911](https://github.com/rust-lang/cargo/pull/14911)
- build-rs: Correctly refer to the item in assert 
  [#14913](https://github.com/rust-lang/cargo/pull/14913)
- build-rs: Add the 'error' directive 
  [#14910](https://github.com/rust-lang/cargo/pull/14910)
- build-rs: Remove meaningless 'cargo_cfg_debug_assertions' 
  [#14901](https://github.com/rust-lang/cargo/pull/14901)
- cargo-package: split `cargo_package` to modules 
  [#14959](https://github.com/rust-lang/cargo/pull/14959)
  [#14982](https://github.com/rust-lang/cargo/pull/14982)
- cargo-test-support: `requires` attribute accepts string literals for cmds
  [#14875](https://github.com/rust-lang/cargo/pull/14875)
- cargo-test-support: Switch from 'exec_with_output' to 'run'
  [#14848](https://github.com/rust-lang/cargo/pull/14848)
- cargo-test-support: track caller for `.crate` file publish verification 
  [#14992](https://github.com/rust-lang/cargo/pull/14992)
- test: Verify `-Cmetadata` directly, not through `-Cextra-filename`
  [#14846](https://github.com/rust-lang/cargo/pull/14846)
- test: ensure PGO works
  [#14859](https://github.com/rust-lang/cargo/pull/14859)
  [#14874](https://github.com/rust-lang/cargo/pull/14874)
  [#14887](https://github.com/rust-lang/cargo/pull/14887)
- Update dependencies.
  [#14867](https://github.com/rust-lang/cargo/pull/14867)
  [#14871](https://github.com/rust-lang/cargo/pull/14871)
  [#14878](https://github.com/rust-lang/cargo/pull/14878)
  [#14879](https://github.com/rust-lang/cargo/pull/14879)
  [#14975](https://github.com/rust-lang/cargo/pull/14975)

## Cargo 1.84 (2025-01-09)
[15fbd2f6...rust-1.84.0](https://github.com/rust-lang/cargo/compare/15fbd2f6...rust-1.84.0)

### Added

- 🎉 Stabilize resolver v3, a.k.a the MSRV-aware dependency resolver.
  The stabilization includes `package.resolver = "3"` in Cargo.toml,
  and the `[resolver]` table in Cargo configuration.
  ([RFC 3537](https://github.com/rust-lang/rfcs/blob/master/text/3537-msrv-resolver.md))
  ([manifest docs](https://doc.rust-lang.org/nightly/cargo/reference/resolver.html#resolver-versions))
  ([config docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#resolver))
  [#14639](https://github.com/rust-lang/cargo/pull/14639)
  [#14662](https://github.com/rust-lang/cargo/pull/14662)
  [#14711](https://github.com/rust-lang/cargo/pull/14711)
  [#14725](https://github.com/rust-lang/cargo/pull/14725)
  [#14748](https://github.com/rust-lang/cargo/pull/14748)
  [#14753](https://github.com/rust-lang/cargo/pull/14753)
  [#14754](https://github.com/rust-lang/cargo/pull/14754)
- Added a new build script invocation `cargo::error=MESSAGE` to report error messages.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#cargo-error))
  [#14743](https://github.com/rust-lang/cargo/pull/14743)

### Changed

- ❗️ cargo-publish: Always include Cargo.lock in published crates.
  Originally it was only included for packages that have executables or examples
  for use with `cargo install`.
  [#14815](https://github.com/rust-lang/cargo/pull/14815)
- Dependency resolver performance improvements, including shared caching,
  reduced iteration overhead, and removing redundant fetches and clones.
  [#14663](https://github.com/rust-lang/cargo/pull/14663)
  [#14690](https://github.com/rust-lang/cargo/pull/14690)
  [#14692](https://github.com/rust-lang/cargo/pull/14692)
  [#14694](https://github.com/rust-lang/cargo/pull/14694)
- Deprecate `cargo verify-project`.
  [#14736](https://github.com/rust-lang/cargo/pull/14736)
- Add source replacement info when no matching package found during dependency resolving.
  [#14715](https://github.com/rust-lang/cargo/pull/14715)
- Hint for using `crates-io` when `[patch.crates.io]` found.
  [#14700](https://github.com/rust-lang/cargo/pull/14700)
- Normalize source paths of Cargo targets for better diagnostics.
  [#14497](https://github.com/rust-lang/cargo/pull/14497)
  [#14750](https://github.com/rust-lang/cargo/pull/14750)
- Allow registries to omit empty/default fields in index metadata JSON.
  Due to backward compatibility, crates.io continues to emit them.
  [#14838](https://github.com/rust-lang/cargo/pull/14838)
  [#14839](https://github.com/rust-lang/cargo/pull/14839)
- cargo-doc: display env vars in extra verbose mode.
  [#14812](https://github.com/rust-lang/cargo/pull/14812)
- cargo-fix: replace special-case handling of duplicate insert-only replacement.
  [#14765](https://github.com/rust-lang/cargo/pull/14765)
  [#14782](https://github.com/rust-lang/cargo/pull/14782)
- cargo-remove: when a dependency is not found, try suggesting other dependencies
  with similar names.
  [#14818](https://github.com/rust-lang/cargo/pull/14818)
- git: skip unnecessary submodule validations for fresh checkouts on Git dependencies.
  [#14605](https://github.com/rust-lang/cargo/pull/14605)
- git: Enhanced the error message for fetching Git dependencies when refspec not found.
  [#14806](https://github.com/rust-lang/cargo/pull/14806)
- git: Pass `--no-tags` by default to git CLI when `net.git-fetch-with-cli = true`.
  [#14688](https://github.com/rust-lang/cargo/pull/14688)

### Fixed

- Fixed old Cargos failing to read the newer format of dep-info in build caches.
  [#14751](https://github.com/rust-lang/cargo/pull/14751)
  [#14745](https://github.com/rust-lang/cargo/pull/14745)
- Fixed rebuild detection not respecting changes in the `[env]` table.
  [#14701](https://github.com/rust-lang/cargo/pull/14701)
  [#14730](https://github.com/rust-lang/cargo/pull/14730)
- cargo-fix: Added transactional semantics to `rustfix` to keep code fix in a
  valid state when multiple suggestions contain overlapping spans.
  [#14747](https://github.com/rust-lang/cargo/pull/14747)

### Nightly only

- The unstable environment variable `CARGO_RUSTC_CURRENT_DIR` has been removed.
  [#14799](https://github.com/rust-lang/cargo/pull/14799)
- 🔥 Cargo now includes an experimental JSON Schema file for `Cargo.toml` in the source code.
  It helps external tools validate or auto-complete the schema of the manifest.
  ([manifest.schema.json](https://github.com/rust-lang/cargo/blob/master/crates/cargo-util-schemas/manifest.schema.json))
  [#14683](https://github.com/rust-lang/cargo/pull/14683)
- 🔥 `Zroot-dir`: A new unstable `-Zroot-dir` flag to configure the path from
  which rustc should be invoked.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#root-dir))
  [#14752](https://github.com/rust-lang/cargo/pull/14752)
- 🔥 `-Zwarnings`: A new unstable feature to control how Cargo handles warnings
  via the `build.warnings` configuration field.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#warnings))
  [#14388](https://github.com/rust-lang/cargo/pull/14388)
  [#14827](https://github.com/rust-lang/cargo/pull/14827)
  [#14836](https://github.com/rust-lang/cargo/pull/14836)
- `edition2024`: Verify 2024 edition / resolver=3 doesn't affect resolution
  [#14724](https://github.com/rust-lang/cargo/pull/14724)
- `native-completions`: Include descriptions in zsh
  [#14726](https://github.com/rust-lang/cargo/pull/14726)
- `-Zbindeps`: Fix panic when running cargo tree on a package with a cross compiled bindep
  [#14593](https://github.com/rust-lang/cargo/pull/14593)
- `-Zbindeps`: download targeted transitive deps of with artifact deps' target platform
  [#14723](https://github.com/rust-lang/cargo/pull/14723)
- `-Zbuild-std`: Remove the requirement for `--target`.
  [#14317](https://github.com/rust-lang/cargo/pull/14317)
- `-Zpackage-workspace`: Support package selection options, such as `--exclude`,
  in `cargo publish`
  [#14659](https://github.com/rust-lang/cargo/pull/14659)
- `-Zscript`: Remove support for accepting `Cargo.toml`.
  [#14670](https://github.com/rust-lang/cargo/pull/14670)
- `-Zscript`: Change config paths to only check `CARGO_HOME`
  [#14749](https://github.com/rust-lang/cargo/pull/14749)
- `-Zscript`: Update the frontmatter parser for RFC 3503.
  [#14792](https://github.com/rust-lang/cargo/pull/14792)

### Documentation

- Clarify the meaning of `--tests` and `--benches` flags.
  [#14675](https://github.com/rust-lang/cargo/pull/14675)
- Clarify tools should only interpret messages with a line starting with `{` as JSON.
  [#14677](https://github.com/rust-lang/cargo/pull/14677)
- Clarify what is and isn't included by `cargo package`
  [#14684](https://github.com/rust-lang/cargo/pull/14684)
- Document official external commands: `cargo-clippy`, `cargo-fmt`, and `cargo-miri`.
  [#14669](https://github.com/rust-lang/cargo/pull/14669)
  [#14805](https://github.com/rust-lang/cargo/pull/14805)
- Enhance documentation on environment variables
  [#14676](https://github.com/rust-lang/cargo/pull/14676)
- Simplify English used in documentations.
  [#14825](https://github.com/rust-lang/cargo/pull/14825)
  [#14829](https://github.com/rust-lang/cargo/pull/14829)
- A new doc page for deprecated and removed commands.
  [#14739](https://github.com/rust-lang/cargo/pull/14739)
- cargo-test-support: Document `Execs` assertions based on port effort 
  [#14793](https://github.com/rust-lang/cargo/pull/14793)

### Internal

- 🎉 Migrate `build-rs` crate to the `rust-lang/cargo` repository as an
  intentional artifact of the Cargo team.
  [#14786](https://github.com/rust-lang/cargo/pull/14786)
  [#14817](https://github.com/rust-lang/cargo/pull/14817)
- Enable transfer feature in triagebot
  [#14777](https://github.com/rust-lang/cargo/pull/14777)
- clone-on-write when needed for InternedString
  [#14808](https://github.com/rust-lang/cargo/pull/14808)
- ci: Switch CI from bors to merge queue 
  [#14718](https://github.com/rust-lang/cargo/pull/14718)
- ci: make the `lint-docs` job required 
  [#14797](https://github.com/rust-lang/cargo/pull/14797)
- ci: Check for clippy `correctness`
  [#14796](https://github.com/rust-lang/cargo/pull/14796)
- ci: Switch matchPackageNames to matchDepNames for renovate
  [#14704](https://github.com/rust-lang/cargo/pull/14704)
- fingerprint: Track the intent for each use of `UnitHash`
  [#14826](https://github.com/rust-lang/cargo/pull/14826)
- fingerprint: Add more metadata to `rustc_fingerprint`.
  [#14761](https://github.com/rust-lang/cargo/pull/14761)
- test: Migrate remaining snapshotting to snapbox
  [#14642](https://github.com/rust-lang/cargo/pull/14642)
  [#14760](https://github.com/rust-lang/cargo/pull/14760)
  [#14781](https://github.com/rust-lang/cargo/pull/14781)
  [#14785](https://github.com/rust-lang/cargo/pull/14785)
  [#14790](https://github.com/rust-lang/cargo/pull/14790)
- Update dependencies.
  [#14668](https://github.com/rust-lang/cargo/pull/14668)
  [#14705](https://github.com/rust-lang/cargo/pull/14705)
  [#14762](https://github.com/rust-lang/cargo/pull/14762)
  [#14766](https://github.com/rust-lang/cargo/pull/14766)
  [#14772](https://github.com/rust-lang/cargo/pull/14772)

## Cargo 1.83 (2024-11-28)
[8f40fc59...rust-1.83.0](https://github.com/rust-lang/cargo/compare/8f40fc59...rust-1.83.0)

### Added

- `--timings` HTML output can now auto-switch between light and dark color
  schemes based on browser preference.
  [#14588](https://github.com/rust-lang/cargo/pull/14588)
- Introduced a new `CARGO_MANIFEST_PATH` environment variable,
  similar to `CARGO_MANIFEST_DIR` but pointing directly to the manifest file.
  [#14404](https://github.com/rust-lang/cargo/pull/14404)
- manifest: Added `package.autolib`, allowing `[lib]` auto-discovery to be disabled.
  [#14591](https://github.com/rust-lang/cargo/pull/14591)

### Changed

- ❗️ Lockfile format v4 is now the default for creating/updating a lockfile.
  Rust toolchains 1.78+ support lockfile v4.
  For compatibility with earlier MSRV,
  consdier setting the `package.rust-version` to 1.82 or earlier.
  [#14595](https://github.com/rust-lang/cargo/pull/14595)
- ❗️ cargo-package: When using the `--package` flag, only the specified
  packages are packaged. Previously, the package in the current working
  directory was automatically selected for packaging.
  [#14488](https://github.com/rust-lang/cargo/pull/14488)
- cargo-publish: Now fails fast if the package version is already published.
  [#14448](https://github.com/rust-lang/cargo/pull/14448)
- Improved error messages for missing features.
  [#14436](https://github.com/rust-lang/cargo/pull/14436)
- Log details of `rustc` invocation failure if no errors are seen
  [#14453](https://github.com/rust-lang/cargo/pull/14453)
- Uplifted `windows-gnullvm` import libraries, aligning them with `windows-gnu`.
  [#14451](https://github.com/rust-lang/cargo/pull/14451)
- Suggest `cargo info` command in the `cargo search` result
  [#14537](https://github.com/rust-lang/cargo/pull/14537)
- Enhanced dependency update status messages, now displaying updates
  (compatible, incompatible, direct-dep) in different colors,
  along with messages and MSRVs.
  [#14440](https://github.com/rust-lang/cargo/pull/14440)
  [#14457](https://github.com/rust-lang/cargo/pull/14457)
  [#14459](https://github.com/rust-lang/cargo/pull/14459)
  [#14461](https://github.com/rust-lang/cargo/pull/14461)
  [#14471](https://github.com/rust-lang/cargo/pull/14471)
  [#14568](https://github.com/rust-lang/cargo/pull/14568)
- The `Locking` status message no longer displays workspace members.
  [#14445](https://github.com/rust-lang/cargo/pull/14445)

### Fixed

- Prevented duplicate library search environment variables when calling `cargo` recursively.
  [#14464](https://github.com/rust-lang/cargo/pull/14464)
- Don't double-warn about `$CARGO_HOME/config` not having `.toml` extension.
  [#14579](https://github.com/rust-lang/cargo/pull/14579)
- Correct diagnostic count message when using `--message-format json`.
  [#14598](https://github.com/rust-lang/cargo/pull/14598)
- cargo-add: Perform fuzzy searches when translating package names
  [#13765](https://github.com/rust-lang/cargo/pull/13765)
- cargo-new: only auto-add new packages to the workspace relative to the manifest,
  rather than the current directory.
  [#14505](https://github.com/rust-lang/cargo/pull/14505)
- cargo-rustc: Fixed parsing of comma-separated values in the `--crate-type` flag.
  [#14499](https://github.com/rust-lang/cargo/pull/14499)
- cargo-vendor: trusts the crate version only when it originates from registries.
  This causes git dependencies to be re-vendored even if they haven't changed.
  [#14530](https://github.com/rust-lang/cargo/pull/14530)
- cargo-publish: Downgrade version-exists error to warning on dry-run
  [#14742](https://github.com/rust-lang/cargo/pull/14742)
  [#14744](https://github.com/rust-lang/cargo/pull/14744)

### Nightly only

- ❗️ cargo-rustc: give trailing flags higher precedence on nightly.
  The nightly gate will be removed after a few releases.
  Please give feedback if it breaks any workflow.
  A temporary environment variable `__CARGO_RUSTC_ORIG_ARGS_PRIO=1` is provided
  to opt-out of the behavior.
  [#14587](https://github.com/rust-lang/cargo/pull/14587)
- 🔥 cargo-install: a new `--dry-run` flag without actually installing binaries.
  [#14280](https://github.com/rust-lang/cargo/pull/14280)
- 🔥 `native-completions`: moves the handwritten shell completion scripts to
  Rust native, making it easier for us to add, extend, and test new completions.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#native-completions))
  [#14493](https://github.com/rust-lang/cargo/pull/14493)
  [#14531](https://github.com/rust-lang/cargo/pull/14531)
  [#14532](https://github.com/rust-lang/cargo/pull/14532)
  [#14533](https://github.com/rust-lang/cargo/pull/14533)
  [#14534](https://github.com/rust-lang/cargo/pull/14534)
  [#14535](https://github.com/rust-lang/cargo/pull/14535)
  [#14536](https://github.com/rust-lang/cargo/pull/14536)
  [#14546](https://github.com/rust-lang/cargo/pull/14546)
  [#14547](https://github.com/rust-lang/cargo/pull/14547)
  [#14548](https://github.com/rust-lang/cargo/pull/14548)
  [#14552](https://github.com/rust-lang/cargo/pull/14552)
  [#14557](https://github.com/rust-lang/cargo/pull/14557)
  [#14558](https://github.com/rust-lang/cargo/pull/14558)
  [#14563](https://github.com/rust-lang/cargo/pull/14563)
  [#14564](https://github.com/rust-lang/cargo/pull/14564)
  [#14573](https://github.com/rust-lang/cargo/pull/14573)
  [#14590](https://github.com/rust-lang/cargo/pull/14590)
  [#14592](https://github.com/rust-lang/cargo/pull/14592)
  [#14653](https://github.com/rust-lang/cargo/pull/14653)
  [#14656](https://github.com/rust-lang/cargo/pull/14656)
- 🔥 `-Zchecksum-freshness`: replace the use of file mtimes in cargo’s rebuild
  detection with a file checksum algorithm. This is most useful on systems with
  a poor mtime implementation, or in CI/CD.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#checksum-freshness))
  [#14137](https://github.com/rust-lang/cargo/pull/14137)
- cargo-update: Add `matches_prerelease` semantic
  [#14305](https://github.com/rust-lang/cargo/pull/14305)
- `build-plan`: document it as being deprecated.
  [#14657](https://github.com/rust-lang/cargo/pull/14657)
- `edition2024`: Remove implicit feature removal from 2024 edition.
  [#14630](https://github.com/rust-lang/cargo/pull/14630)
- `lockfile-path`: implies `--locked` on `cargo install`.
  [#14556](https://github.com/rust-lang/cargo/pull/14556)
- `open-namespaces`: Allow open namespaces in `PackageIdSpec`s
  [#14467](https://github.com/rust-lang/cargo/pull/14467)
- `path-bases`: `cargo [add|remove|update]` support 
  [#14427](https://github.com/rust-lang/cargo/pull/14427)
- `-Zmsrv-policy`: determine the workspace's MSRV by the most number of MSRVs within it.
  [#14569](https://github.com/rust-lang/cargo/pull/14569)
- `-Zpackage-workspace`: allows to publish multiple crates in a workspace,
  even if they have inter-dependencies.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#package-workspace))
  [#14433](https://github.com/rust-lang/cargo/pull/14433)
  [#14496](https://github.com/rust-lang/cargo/pull/14496)
- `-Zpublic-dependency`: Include public/private dependency status in `cargo metadata` 
  [#14504](https://github.com/rust-lang/cargo/pull/14504)
- `-Zpublic-dependency`: Don't require MSRV bump
  [#14507](https://github.com/rust-lang/cargo/pull/14507)

### Documentation

- 🎉 New chapter about the uses, support expectations, and management of
  `package.rust-version` a.k.a MSRV.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/rust-version.html))
  [#14619](https://github.com/rust-lang/cargo/pull/14619)
  [#14636](https://github.com/rust-lang/cargo/pull/14636)
- Clarify `target.'cfg(...)'` doesn't respect cfg from build script 
  [#14312](https://github.com/rust-lang/cargo/pull/14312)
- Clarify `[[bin]]` target auto-discovery can be `src/main.rs` and/or in `src/bin/`
  [#14515](https://github.com/rust-lang/cargo/pull/14515)
- Disambiguate the use of 'target' in the feature resolver v2 doc.
  [#14540](https://github.com/rust-lang/cargo/pull/14540)
- Make `--config <PATH>` more prominent 
  [#14631](https://github.com/rust-lang/cargo/pull/14631)
- Minor re-grouping of pages.
  [#14620](https://github.com/rust-lang/cargo/pull/14620)
- contrib: Update docs for how cargo is published 
  [#14539](https://github.com/rust-lang/cargo/pull/14539)
- contrib: Declare support level for each crate in Cargo's Charter / crate docs
  [#14600](https://github.com/rust-lang/cargo/pull/14600)
- contrib: Declare new Intentional Artifacts as 'small' changes
  [#14599](https://github.com/rust-lang/cargo/pull/14599)

### Internal

- Cleanup duplicated check-cfg lint logic 
  [#14567](https://github.com/rust-lang/cargo/pull/14567)
- Fix elided lifetime due to nightly rustc changes
  [#14487](https://github.com/rust-lang/cargo/pull/14487)
- Improved error reporting when a feature is not found in `activated_features`.
  [#14647](https://github.com/rust-lang/cargo/pull/14647)
- cargo-info: Use the `shell.note` to print the note 
  [#14554](https://github.com/rust-lang/cargo/pull/14554)
- ci: bump CI tools
  [#14503](https://github.com/rust-lang/cargo/pull/14503)
  [#14628](https://github.com/rust-lang/cargo/pull/14628)
- perf: zero-copy deserialization for compiler messages when possible
  [#14608](https://github.com/rust-lang/cargo/pull/14608)
- resolver: Add more SAT resolver tests
  [#14583](https://github.com/rust-lang/cargo/pull/14583)
  [#14614](https://github.com/rust-lang/cargo/pull/14614)
- test: Migrated more tests to snapbox
  [#14576](https://github.com/rust-lang/cargo/pull/14576)
  [#14577](https://github.com/rust-lang/cargo/pull/14577)
- Update dependencies.
  [#14475](https://github.com/rust-lang/cargo/pull/14475)
  [#14478](https://github.com/rust-lang/cargo/pull/14478)
  [#14489](https://github.com/rust-lang/cargo/pull/14489)
  [#14607](https://github.com/rust-lang/cargo/pull/14607)
  [#14624](https://github.com/rust-lang/cargo/pull/14624)
  [#14632](https://github.com/rust-lang/cargo/pull/14632)

## Cargo 1.82 (2024-10-17)
[a2b58c3d...rust-1.82.0](https://github.com/rust-lang/cargo/compare/a2b58c3d...rust-1.82.0)

### Added

- 🎉 Added `cargo info` command for displaying information about a package.
  [docs](https://doc.rust-lang.org/nightly/cargo/commands/cargo-info.html)
  [#14141](https://github.com/rust-lang/cargo/pull/14141)
  [#14418](https://github.com/rust-lang/cargo/pull/14418)
  [#14430](https://github.com/rust-lang/cargo/pull/14430)

### Changed

- ❗️ Doctest respects Cargo's color options by passing `--color` to rustdoc invocations.
  [#14425](https://github.com/rust-lang/cargo/pull/14425)
- Improved error message for missing both `[package]` and `[workspace]` in Cargo.toml.
  [#14261](https://github.com/rust-lang/cargo/pull/14261)
- Enumerate all possible values of `profile.*.debug` for the error message.
  [#14413](https://github.com/rust-lang/cargo/pull/14413)

### Fixed

- Use longhand gitoxide path-spec patterns. Previously the implementation used 
  shorthand pathspecs, which could produce invalid syntax, for example, if the
  path to the manifest file contained a leading `_` underscore
  [#14380](https://github.com/rust-lang/cargo/pull/14380)
- cargo-package: fix failures on bare commit git repo.
  [#14359](https://github.com/rust-lang/cargo/pull/14359)
- cargo-publish: Don't strip non-dev features for renamed dependencies from the
  HTTP JSON body sent to the registry.
  The bug only affected third-party registries.
  [#14325](https://github.com/rust-lang/cargo/pull/14325)
  [#14327](https://github.com/rust-lang/cargo/pull/14327)
- cargo-vendor: don't copy source files of excluded Cargo targets when vendoring.
  [#14367](https://github.com/rust-lang/cargo/pull/14367)

### Nightly only

- 🔥 `lockfile-path`: Added `--lockfile-path` flag that allows specifying a path
  to the lockfile other than the default path `<workspace_root>/Cargo.lock`.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#lockfile-path))
  [#14326](https://github.com/rust-lang/cargo/pull/14326)
  [#14417](https://github.com/rust-lang/cargo/pull/14417)
  [#14423](https://github.com/rust-lang/cargo/pull/14423)
  [#14424](https://github.com/rust-lang/cargo/pull/14424)
- 🔥 `path-bases`: Introduced a table of path "bases" in Cargo configuration files
  that can be used to prefix the paths of path dependencies and patch entries.
  ([RFC 3529](https://github.com/rust-lang/rfcs/blob/master/text/3529-cargo-path-bases.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#path-bases))
  [#14360](https://github.com/rust-lang/cargo/pull/14360)
- 🔥 `-Zpackage-workspace`: Enhanced the experience of `cargo package --workspace`
  when there are dependencies between crates in the workspace.
  Crates in a workspace are no longer required to publish to actual registries.
  This is a step toward supporting `cargo publish --workspace`.
  [#13947](https://github.com/rust-lang/cargo/pull/13947)
  [#14408](https://github.com/rust-lang/cargo/pull/14408)
  [#14340](https://github.com/rust-lang/cargo/pull/14340)
- cargo-update: Limit pre-release match semantics to use only on `OptVersionReq::Req` 
  [#14412](https://github.com/rust-lang/cargo/pull/14412)
- `edition2024`: Revert "fix: Ensure dep/feature activates the dependency on 2024".
  [#14295](https://github.com/rust-lang/cargo/pull/14295)
- `update-breaking`: Improved error message when `update --breaking` has an invalid spec
  [#14279](https://github.com/rust-lang/cargo/pull/14279)
- `update-breaking`: Don’t downgrade on prerelease `VersionReq` when updating with `--breaking`
  [#14250](https://github.com/rust-lang/cargo/pull/14250)
- `-Zbuild-std`: remove hack on creating virtual std workspace 
  [#14358](https://github.com/rust-lang/cargo/pull/14358)
  [#14370](https://github.com/rust-lang/cargo/pull/14370)
- `-Zmsrv-policy`: Adjust MSRV resolve config field name / values.
  The previous placeholder `resolver.something-like-precedence`
  is now renamed to `resolver.incompatible-rust-versions`.
  [#14296](https://github.com/rust-lang/cargo/pull/14296)
- `-Zmsrv-policy`: : Report when incompatible-rust-version packages are selected
  [#14401](https://github.com/rust-lang/cargo/pull/14401)
- `-Ztarget-applies-to-host`: Fixed passing of links-overrides with
  target-applies-to-host and an implicit target
  [#14205](https://github.com/rust-lang/cargo/pull/14205)
- `-Ztarget-applies-to-host`: `-Cmetadata` includes whether extra rustflags is same as host
  [#14432](https://github.com/rust-lang/cargo/pull/14432)
- `-Ztrim-paths`: rustdoc supports trim-paths for diagnostics 
  [#14389](https://github.com/rust-lang/cargo/pull/14389)

### Documentation

- Convert comments to doc comments for `Workspace`.
  [#14397](https://github.com/rust-lang/cargo/pull/14397)
- Fix MSRV indicator for `workspace.package` and `workspace.dependencies`.
  [#14400](https://github.com/rust-lang/cargo/pull/14400)
- FAQ: remove outdated Cargo offline usage section.
  [#14336](https://github.com/rust-lang/cargo/pull/14336)

### Internal

- Enhanced `cargo-test-support` usability and documentation.
  [#14266](https://github.com/rust-lang/cargo/pull/14266)
  [#14268](https://github.com/rust-lang/cargo/pull/14268)
  [#14269](https://github.com/rust-lang/cargo/pull/14269)
  [#14270](https://github.com/rust-lang/cargo/pull/14270)
  [#14272](https://github.com/rust-lang/cargo/pull/14272)
- Made summary sync by using Arc instead of Rc
  [#14260](https://github.com/rust-lang/cargo/pull/14260)
- Used `Rc` instead of `Arc` for storing rustflags
  [#14273](https://github.com/rust-lang/cargo/pull/14273)
- Removed rustc probe for `--check-cfg` support 
  [#14302](https://github.com/rust-lang/cargo/pull/14302)
- Renamed 'resolved' to 'normalized' for all manifest normalization related items.
  [#14342](https://github.com/rust-lang/cargo/pull/14342)
- cargo-util-schemas: Added `TomlPackage::new`, `Default` for `TomlWorkspace`
  [#14271](https://github.com/rust-lang/cargo/pull/14271)
- ci: Switch macos aarch64 to nightly 
  [#14382](https://github.com/rust-lang/cargo/pull/14382)
- mdman: Normalize newlines when rendering options 
  [#14428](https://github.com/rust-lang/cargo/pull/14428)
- perf: dont call wrap in a no-op `source_id::with*`
  [#14318](https://github.com/rust-lang/cargo/pull/14318)
- test: Migrated more tests to snapbox
  [#14242](https://github.com/rust-lang/cargo/pull/14242)
  [#14244](https://github.com/rust-lang/cargo/pull/14244)
  [#14293](https://github.com/rust-lang/cargo/pull/14293)
  [#14297](https://github.com/rust-lang/cargo/pull/14297)
  [#14319](https://github.com/rust-lang/cargo/pull/14319)
  [#14402](https://github.com/rust-lang/cargo/pull/14402)
  [#14410](https://github.com/rust-lang/cargo/pull/14410)
- test: don't rely on absence of `RUST_BACKTRACE`
  [#14441](https://github.com/rust-lang/cargo/pull/14441)
- test: Use gmake on AIX 
  [#14323](https://github.com/rust-lang/cargo/pull/14323)
- Updated to `gix` 0.64.0 
  [#14332](https://github.com/rust-lang/cargo/pull/14332)
- Updated to `rusqlite` 0.32.0 
  [#14334](https://github.com/rust-lang/cargo/pull/14334)
- Updated to `windows-sys` 0.59
  [#14335](https://github.com/rust-lang/cargo/pull/14335)
- Update dependencies.
  [#14299](https://github.com/rust-lang/cargo/pull/14299)
  [#14303](https://github.com/rust-lang/cargo/pull/14303)
  [#14324](https://github.com/rust-lang/cargo/pull/14324)
  [#14329](https://github.com/rust-lang/cargo/pull/14329)
  [#14331](https://github.com/rust-lang/cargo/pull/14331)
  [#14391](https://github.com/rust-lang/cargo/pull/14391)

## Cargo 1.81 (2024-09-05)
[34a6a87d...rust-1.81.0](https://github.com/rust-lang/cargo/compare/34a6a87d...rust-1.81.0)

### Added

### Changed

- ❗️ cargo-package: Disallow `package.license-file` and `package.readme` pointing
  to non-existent files during packaging.
  [#13921](https://github.com/rust-lang/cargo/pull/13921)
- ❗️ cargo-package: generated `.cargo_vcs_info.json` is always included,
  even when `--allow-dirty` is passed.
  [#13960](https://github.com/rust-lang/cargo/pull/13960)
- ❗️ Disallow passing `--release`/`--debug` flag along with the `--profile` flag.
  [#13971](https://github.com/rust-lang/cargo/pull/13971)
- ❗️ Remove `lib.plugin` key support in Cargo.toml.
  Rust plugin support has been deprecated for four years and was removed in 1.75.0.
  [#13902](https://github.com/rust-lang/cargo/pull/13902)
  [#14038](https://github.com/rust-lang/cargo/pull/14038)
- Make the calculation of `-Cmetadata` for rustc consistent across platforms.
  [#14107](https://github.com/rust-lang/cargo/pull/14107)
- Emit a warning when `edition` is unset, even when MSRV is unset.
  [#14110](https://github.com/rust-lang/cargo/pull/14110)

### Fixed

- Fix a proc-macro example from a dependency affecting feature resolution.
  [#13892](https://github.com/rust-lang/cargo/pull/13892)
- Don't warn on duplicate packages from using '..'.
  [#14234](https://github.com/rust-lang/cargo/pull/14234)
- Don't `du` on every git source load.
  [#14252](https://github.com/rust-lang/cargo/pull/14252)
- Don't warn about unreferenced duplicate packages 
  [#14239](https://github.com/rust-lang/cargo/pull/14239)
- cargo-publish: Don't strip non-dev features for renamed dependencies from the
  HTTP JSON body sent to the registry.
  The bug only affected third-party registries.
  [#14328](https://github.com/rust-lang/cargo/pull/14328)
- cargo-vendor: don't copy source files of excluded Cargo targets when vendoring.
  [#14368](https://github.com/rust-lang/cargo/pull/14368)

### Nightly only

- 🔥 `update-breaking`: Add `--breaking` to `cargo update`,
  allowing upgrading dependencies to breaking versions.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#update-breaking)
  [#13979](https://github.com/rust-lang/cargo/pull/13979)
  [#14047](https://github.com/rust-lang/cargo/pull/14047)
  [#14049](https://github.com/rust-lang/cargo/pull/14049)
- `--artifact-dir`: Rename `--out-dir` to `--artifact-dir`.
  The `--out-dir` flag is kept for compatibility
  and may be removed when the feature gets stabilized.
  [#13809](https://github.com/rust-lang/cargo/pull/13809)
- `edition2024`: Ensure unused optional dependencies fire for shadowed dependencies.
  [#14028](https://github.com/rust-lang/cargo/pull/14028)
- `edition2024`: Address problems with implicit -> explicit feature migration 
  [#14018](https://github.com/rust-lang/cargo/pull/14018)
- `-Zcargo-lints`: Add `unknown_lints` to lints list.
  [#14024](https://github.com/rust-lang/cargo/pull/14024)
- `-Zcargo-lints`: Add tooling to document lints.
  [#14025](https://github.com/rust-lang/cargo/pull/14025)
- `-Zcargo-lints`: Keep lints updated and sorted.
  [#14030](https://github.com/rust-lang/cargo/pull/14030)
- `-Zconfig-include`: Allow enabling `config-include` feature in config.
  [#14196](https://github.com/rust-lang/cargo/pull/14196)
- `-Zpublic-dependency`: remove some legacy public dependency code from the resolver
  [#14090](https://github.com/rust-lang/cargo/pull/14090)
- `-Ztarget-applies-to-host`: Pass rustflags to artifacts built with implicit targets when using target-applies-to-host
  [#13900](https://github.com/rust-lang/cargo/pull/13900)
  [#14201](https://github.com/rust-lang/cargo/pull/14201)
- cargo-update: Track the behavior of `--precise <prerelease>`.
  [#14013](https://github.com/rust-lang/cargo/pull/14013)

### Documentation

- Clarify `CARGO_CFG_TARGET_FAMILY` is multi-valued.
  [#14165](https://github.com/rust-lang/cargo/pull/14165)
- Document `CARGO_CFG_TARGET_ABI`
  [#14164](https://github.com/rust-lang/cargo/pull/14164)
- Document MSRV for each manifest field and build script invocations.
  [#14224](https://github.com/rust-lang/cargo/pull/14224)
- Remove duplicate `strip` section. 
  [#14146](https://github.com/rust-lang/cargo/pull/14146)
- Update summary of Cargo configuration to include missing keys. 
  [#14145](https://github.com/rust-lang/cargo/pull/14145)
- Update index of Cargo documentation.
  [#14228](https://github.com/rust-lang/cargo/pull/14228)
- Don't mention non-existent `workspace.badges` field.
  [#14042](https://github.com/rust-lang/cargo/pull/14042)
- contrib: Suggest atomic commits with separate test commits.
  [#14014](https://github.com/rust-lang/cargo/pull/14014)
- contrib: Document how to write an RFC for Cargo.
  [#14222](https://github.com/rust-lang/cargo/pull/14222)
- contrib: Improve triage instructions 
  [#14052](https://github.com/rust-lang/cargo/pull/14052)

### Internal

- cargo-package: Change verification order during packaging. 
  [#14074](https://github.com/rust-lang/cargo/pull/14074)
- ci: Add workflow to publish Cargo automatically 
  [#14202](https://github.com/rust-lang/cargo/pull/14202)
- ci: bump CI tools 
  [#14062](https://github.com/rust-lang/cargo/pull/14062)
  [#14257](https://github.com/rust-lang/cargo/pull/14257)
- registry: Add local registry overlays.
  [#13926](https://github.com/rust-lang/cargo/pull/13926)
- registry: move `get_source_id` out of registry
  [#14218](https://github.com/rust-lang/cargo/pull/14218)
- resolver: Simplify checking for dependency cycles 
  [#14089](https://github.com/rust-lang/cargo/pull/14089)
- rustfix: Add `CodeFix::apply_solution` and impl `Clone` 
  [#14092](https://github.com/rust-lang/cargo/pull/14092)
- source: Clean up after `PathSource`/`RecursivePathSource` split
  [#14169](https://github.com/rust-lang/cargo/pull/14169)
  [#14231](https://github.com/rust-lang/cargo/pull/14231)
- Remove the temporary `__CARGO_GITOXIDE_DISABLE_LIST_FILES` environment variable.
  [#14036](https://github.com/rust-lang/cargo/pull/14036)
- Simplify checking feature syntax 
  [#14106](https://github.com/rust-lang/cargo/pull/14106)
- Dont make new constant `InternedString` in hot path 
  [#14211](https://github.com/rust-lang/cargo/pull/14211)
- Use `std::fs::absolute` instead of reimplementing it 
  [#14075](https://github.com/rust-lang/cargo/pull/14075)
- Remove unnecessary feature activations from cargo.
  [#14122](https://github.com/rust-lang/cargo/pull/14122)
  [#14160](https://github.com/rust-lang/cargo/pull/14160)
- Revert #13630 as rustc ignores `-C strip` on MSVC.
  [#14061](https://github.com/rust-lang/cargo/pull/14061)
- test: Allow `unexpected_builtin_cfgs` lint in `user_specific_cfgs` test 
  [#14153](https://github.com/rust-lang/cargo/pull/14153)
- test: Add cargo_test to test-support prelude 
  [#14243](https://github.com/rust-lang/cargo/pull/14243)
- test: migrate Cargo testsuite to `snapbox`.
  For the complete list of migration pull requests,
  see [#14039](https://github.com/rust-lang/cargo/issues/14039#issuecomment-2158974033)
- Updated to `gix` 0.64.0 
  [#14431](https://github.com/rust-lang/cargo/pull/14431)
- Update dependencies.
  [#13995](https://github.com/rust-lang/cargo/pull/13995)
  [#13998](https://github.com/rust-lang/cargo/pull/13998)
  [#14037](https://github.com/rust-lang/cargo/pull/14037)
  [#14063](https://github.com/rust-lang/cargo/pull/14063)
  [#14067](https://github.com/rust-lang/cargo/pull/14067)
  [#14174](https://github.com/rust-lang/cargo/pull/14174)
  [#14186](https://github.com/rust-lang/cargo/pull/14186)
  [#14254](https://github.com/rust-lang/cargo/pull/14254)

## Cargo 1.80 (2024-07-25)
[b60a1555...rust-1.80.0](https://github.com/rust-lang/cargo/compare/b60a1555...rust-1.80.0)

### Added

- 🎉 Stabilize `-Zcheck-cfg`! This by default enables rustc's checking of
  conditional compilation at compile time, which verifies that the crate is
  correctly handling conditional compilation for different target platforms or
  features. Internally, cargo will be passing a new command line option
  `--check-cfg` to all rustc and rustdoc invocations.

  A new build script invocation
  [`cargo::rustc-check-cfg=CHECK_CFG`](https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#rustc-check-cfg)
  is added along with this stabilization, as a way to add custom cfgs to the
  list of expected cfg names and values.

  If a build script is not an option for your package, Cargo provides a config
  [`[lints.rust.unexpected_cfgs.check-cfg]`](https://doc.rust-lang.org/nightly/rustc/check-cfg/cargo-specifics.html#check-cfg-in-lintsrust-table)
  to add known custom cfgs statically.

  ([RFC 3013](https://github.com/rust-lang/rfcs/blob/master/text/3013-conditional-compilation-checking.md))
  ([docs](https://doc.rust-lang.org/nightly/rustc/check-cfg/cargo-specifics.html))
  [#13571](https://github.com/rust-lang/cargo/pull/13571)
  [#13865](https://github.com/rust-lang/cargo/pull/13865)
  [#13869](https://github.com/rust-lang/cargo/pull/13869)
  [#13884](https://github.com/rust-lang/cargo/pull/13884)
  [#13913](https://github.com/rust-lang/cargo/pull/13913)
  [#13937](https://github.com/rust-lang/cargo/pull/13937)
  [#13958](https://github.com/rust-lang/cargo/pull/13958)

- 🎉 cargo-update: Allows `--precise` to specify a yanked version of a package,
  and will update the lockfile accordingly.
  [#13974](https://github.com/rust-lang/cargo/pull/13974)

### Changed

- ❗️ manifest: Disallow `[badges]` to inherit from `[workspace.package.badges]`.
  This was considered a bug.
  Keep in mind that `[badges]` is effectively deprecated.
  [#13788](https://github.com/rust-lang/cargo/pull/13788)
- build-script: Suggest old syntax based on MSRV.
  [#13874](https://github.com/rust-lang/cargo/pull/13874)
- cargo-add: Avoid escaping double quotes by using string literals.
  [#14006](https://github.com/rust-lang/cargo/pull/14006)
- cargo-clean: Performance improvements for cleaning specific packages via `-p` flag.
  [#13818](https://github.com/rust-lang/cargo/pull/13818)
- cargo-new: Use `i32` rather than `usize` as the "default integer" in library template.
  [#13939](https://github.com/rust-lang/cargo/pull/13939)
- cargo-package: Warn, rather than fail, if a Cargo target is excluded during packaging.
  [#13713](https://github.com/rust-lang/cargo/pull/13713)
- manifest: Warn, not error, on unsupported lint tool in the `[lints]` table.
  [#13833](https://github.com/rust-lang/cargo/pull/13833)
- perf: Avoid inferring when Cargo targets are known.
  [#13849](https://github.com/rust-lang/cargo/pull/13849)
- Populate git information when building Cargo from Rust's source tarball.
  [#13832](https://github.com/rust-lang/cargo/pull/13832)
- Improve the error message when deserializing Cargo configuration from partial environment variables.
  [#13956](https://github.com/rust-lang/cargo/pull/13956)

### Fixed

- resolver: Make path dependencies with the same name stay locked.
  [#13572](https://github.com/rust-lang/cargo/pull/13572)
- cargo-add: Preserve file permissions on Unix during `write_atomic`.
  [#13898](https://github.com/rust-lang/cargo/pull/13898)
- cargo-clean: Remove symlink directory on Windows.
  [#13910](https://github.com/rust-lang/cargo/pull/13910)
- cargo-fix: Don't fix into the standard library.
  [#13792](https://github.com/rust-lang/cargo/pull/13792)
- cargo-fix: Support IPv6-only networks.
  [#13907](https://github.com/rust-lang/cargo/pull/13907)
- cargo-new: Don't say we're adding to a workspace when a regular package is in the root.
  [#13987](https://github.com/rust-lang/cargo/pull/13987)
- cargo-vendor: Silence the warning about forgetting the vendoring.
  [#13886](https://github.com/rust-lang/cargo/pull/13886)
- cargo-publish/cargo-vendor: Ensure targets in generated Cargo.toml are in a deterministic order.
  [#13989](https://github.com/rust-lang/cargo/pull/13989)
  [#14004](https://github.com/rust-lang/cargo/pull/14004)
- cargo-credential-libsecret: Load `libsecret` by its `SONAME`, `libsecret-1.so.0`.
  [#13927](https://github.com/rust-lang/cargo/pull/13927)
- Don't panic when an alias doesn't include a subcommand.
  [#13819](https://github.com/rust-lang/cargo/pull/13819)
- Workaround copying file returning EAGAIN on ZFS on macOS.
  [#13845](https://github.com/rust-lang/cargo/pull/13845)
- Fetch specific commits even if the GitHub fast path fails.
  [#13946](https://github.com/rust-lang/cargo/pull/13946)
  [#13969](https://github.com/rust-lang/cargo/pull/13969)
- Distinguish Cargo config from different environment variables that share the same prefix.
  [#14000](https://github.com/rust-lang/cargo/pull/14000)

### Nightly only

- `-Zcargo-lints`: Don't always inherit workspace lints.
  [#13812](https://github.com/rust-lang/cargo/pull/13812)
- `-Zcargo-lints`: Add a test to ensure cap-lints works.
  [#13829](https://github.com/rust-lang/cargo/pull/13829)
- `-Zcargo-lints`: Error when unstable lints are specified but not enabled.
  [#13805](https://github.com/rust-lang/cargo/pull/13805)
- `-Zcargo-lints`: Add cargo-lints to unstable docs.
  [#13881](https://github.com/rust-lang/cargo/pull/13881)
- `-Zcargo-lints`: Refactor cargo lint tests.
  [#13880](https://github.com/rust-lang/cargo/pull/13880)
- `-Zcargo-lints`: Remove ability to specify `-` in lint name.
  [#13837](https://github.com/rust-lang/cargo/pull/13837)
- `-Zscript`: Remove unstable rejected frontmatter syntax for cargo script.
  The only allowed frontmatter syntax now is `---`.
  [#13861](https://github.com/rust-lang/cargo/pull/13861)
  [#13893](https://github.com/rust-lang/cargo/pull/13893)
- `-Zbindeps`: Build only the specified artifact library when multiple types are available.
  [#13842](https://github.com/rust-lang/cargo/pull/13842)
- `-Zmsrv-policy`: Treat unset MSRV as compatible.
  [#13791](https://github.com/rust-lang/cargo/pull/13791)
- `-Zgit`/`-Zgitoxide`: Default configuration to be obtained from both environment variables and Cargo configuration.
  [#13687](https://github.com/rust-lang/cargo/pull/13687)
- `-Zpublic-dependency`: Don't lose 'public' when inheriting a dependency.
  [#13836](https://github.com/rust-lang/cargo/pull/13836)
- `edition2024`: Disallow ignored `default-features` when inheriting.
  [#13839](https://github.com/rust-lang/cargo/pull/13839)
- `edition2024`: Validate crate-types/proc-macro for bin like other Cargo targets.
  [#13841](https://github.com/rust-lang/cargo/pull/13841)

### Documentation

- cargo-package: Clarify no guarantee of VCS provenance.
  [#13984](https://github.com/rust-lang/cargo/pull/13984)
- cargo-metadata: Clarify dash replacement rule in Cargo target names.
  [#13887](https://github.com/rust-lang/cargo/pull/13887)
- config: Fix wrong type of `rustc-flags` in build script overrides.
  [#13957](https://github.com/rust-lang/cargo/pull/13957)
- resolver: Add README for `resolver-tests`.
  [#13977](https://github.com/rust-lang/cargo/pull/13977)
- contrib: Update UI example code in contributor guide.
  [#13864](https://github.com/rust-lang/cargo/pull/13864)
- Fix libcurl proxy documentation link.
  [#13990](https://github.com/rust-lang/cargo/pull/13990)
- Add missing `CARGO_MAKEFLAGS` env for plugins.
  [#13872](https://github.com/rust-lang/cargo/pull/13872)
- Include CircleCI reference in the Continuous Integration chapter.
  [#13850](https://github.com/rust-lang/cargo/pull/13850)

### Internal

- ci: Don't check `cargo` against beta channel.
  [#13827](https://github.com/rust-lang/cargo/pull/13827)
- test: Set safe.directory for git repo in apache container.
  [#13920](https://github.com/rust-lang/cargo/pull/13920)
- test: Silence warnings running embedded unittests.
  [#13929](https://github.com/rust-lang/cargo/pull/13929)
- test: Update test formatting due to nightly rustc changes.
  [#13890](https://github.com/rust-lang/cargo/pull/13890)
  [#13901](https://github.com/rust-lang/cargo/pull/13901)
  [#13964](https://github.com/rust-lang/cargo/pull/13964)
- test: Make `git::use_the_cli` test truly locale independent.
  [#13935](https://github.com/rust-lang/cargo/pull/13935)
- cargo-test-support: Transition direct assertions from cargo-test-support to snapbox.
  [#13980](https://github.com/rust-lang/cargo/pull/13980)
- cargo-test-support: Auto-redact elapsed time.
  [#13973](https://github.com/rust-lang/cargo/pull/13973)
- cargo-test-support: Clean up unnecessary uses of `match_exact`.
  [#13879](https://github.com/rust-lang/cargo/pull/13879)
- Split `RecursivePathSource` out of `PathSource`.
  [#13993](https://github.com/rust-lang/cargo/pull/13993)
- Adjust custom errors from cert-check due to libgit2 1.8 change.
  [#13970](https://github.com/rust-lang/cargo/pull/13970)
- Move diagnostic printing to Shell.
  [#13813](https://github.com/rust-lang/cargo/pull/13813)
- Update dependencies.
  [#13834](https://github.com/rust-lang/cargo/pull/13834)
  [#13840](https://github.com/rust-lang/cargo/pull/13840)
  [#13948](https://github.com/rust-lang/cargo/pull/13948)
  [#13963](https://github.com/rust-lang/cargo/pull/13963)
  [#13976](https://github.com/rust-lang/cargo/pull/13976)

## Cargo 1.79 (2024-06-13)
[2fe739fc...rust-1.79.0](https://github.com/rust-lang/cargo/compare/2fe739fc...rust-1.79.0)

### Added

- 🎉 `cargo add` respects `package.rust-version` a.k.a. MSRV when adding new
  dependencies. The behavior can be overridden by specifying a version requirement,
  or passing the `--ignore-rust-version` flag.
  ([RFC 3537](https://github.com/rust-lang/rfcs/blob/master/text/3537-msrv-resolver.md))
  [#13608](https://github.com/rust-lang/cargo/pull/13608)
- A new `Locking` status message shows dependency changes on any command.
  For `cargo update`, it also tells you if any dependency version is outdated.
  [#13561](https://github.com/rust-lang/cargo/pull/13561)
  [#13647](https://github.com/rust-lang/cargo/pull/13647)
  [#13651](https://github.com/rust-lang/cargo/pull/13651)
  [#13657](https://github.com/rust-lang/cargo/pull/13657)
  [#13759](https://github.com/rust-lang/cargo/pull/13759)
  [#13764](https://github.com/rust-lang/cargo/pull/13764)

### Changed

- ❗️ `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, and variables from the `[env]`
  table now also apply to the initial `rustc -vV` invocation Cargo uses for
  probing rustc information.
  [#13659](https://github.com/rust-lang/cargo/pull/13659)
- ❗️ Turns dependencies like `foo = { optional = true }` from `version="*"`
  dependencies with a warning into errors.
  This behavior has been considered a bug from the beginning.
  [#13775](https://github.com/rust-lang/cargo/pull/13775)
- ❗️ Replace dashes with underscores also if `lib.name` is inferred from `package.name`.
  This change aligns to the documented behavior. One caveat is that JSON messages
  emitted by Cargo, like via `cargo metadata` or `--message-format=json`,
  will start reporting underscore lib names.
  [#12783](https://github.com/rust-lang/cargo/pull/12783)
- Switch to `gitoxide` for listing files. This improves the performance of
  build script and `cargo doc` for computing cache freshness,
  as well as fixes some subtle bugs for `cargo publish`.
  [#13592](https://github.com/rust-lang/cargo/pull/13592)
  [#13696](https://github.com/rust-lang/cargo/pull/13696)
  [#13704](https://github.com/rust-lang/cargo/pull/13704)
  [#13777](https://github.com/rust-lang/cargo/pull/13777)
- Warn on `-Zlints` being passed and no longer necessary.
  [#13632](https://github.com/rust-lang/cargo/pull/13632)
- Warn on unused `workspace.dependencies` keys on virtual workspaces.
  [#13664](https://github.com/rust-lang/cargo/pull/13664)
- Emit 1.77 build script syntax error only when msrv is incompatible.
  [#13808](https://github.com/rust-lang/cargo/pull/13808)
- Don't warn on `lints.rust.unexpected_cfgs.check-cfg`.
  [#13925](https://github.com/rust-lang/cargo/pull/13925)
- cargo-init: don't assign `target.name` in Cargo.toml if the value can be inferred.
  [#13606](https://github.com/rust-lang/cargo/pull/13606)
- cargo-package: normalize paths in `Cargo.toml`, including replacing `\` with `/`.
  [#13729](https://github.com/rust-lang/cargo/pull/13729)
- cargo-test: recategorize cargo test's `--doc` flag under “Target Selection”.
  [#13756](https://github.com/rust-lang/cargo/pull/13756)

### Fixed

- Ensure `--config net.git-fetch-with-cli=true` is respected.
  [#13992](https://github.com/rust-lang/cargo/pull/13992)
  [#13997](https://github.com/rust-lang/cargo/pull/13997)
- Dont panic when resolving an empty alias.
  [#13613](https://github.com/rust-lang/cargo/pull/13613)
- When using `--target`, the default debuginfo strip rule also applies.
  Note that on Windows MSVC Cargo no longer strips by default.
  [#13618](https://github.com/rust-lang/cargo/pull/13618)
- Don't crash on Cargo.toml parse errors that point to multi-byte character
  [#13780](https://github.com/rust-lang/cargo/pull/13780)
- Don't emit deprecation warning if one of `.cargo/{config,config.toml}` is
  a symlink to the other.
  [#13793](https://github.com/rust-lang/cargo/pull/13793)
- Follow HTTP redirections when checking if a repo on GitHub is up-to-date.
  [#13718](https://github.com/rust-lang/cargo/pull/13718)
- Bash completion fallback in `nounset` mode.
  [#13686](https://github.com/rust-lang/cargo/pull/13686)
- Rerun build script when rustflags changed and `--target` was passed.
  [#13560](https://github.com/rust-lang/cargo/pull/13560)
- Fix doc collision for lib/bin with a dash in the inferred name.
  [#13640](https://github.com/rust-lang/cargo/pull/13640)
- cargo-add: Maintain sorting of dependency features.
  [#13682](https://github.com/rust-lang/cargo/pull/13682)
- cargo-add: Preserve comments when updating simple deps
  [#13655](https://github.com/rust-lang/cargo/pull/13655)
- cargo-fix: dont apply same suggestion twice.
  [#13728](https://github.com/rust-lang/cargo/pull/13728)
- cargo-package: error when the package specified via `--package` cannot be found
  [#13735](https://github.com/rust-lang/cargo/pull/13735)
- credential-provider: trim newlines in tokens from stdin.
  [#13770](https://github.com/rust-lang/cargo/pull/13770)

### Nightly only

- 🔥 cargo-update: allows `--precise` to specify a pre-release version of a package
  ([RFC 3493](https://github.com/rust-lang/rfcs/blob/master/text/3493-precise-pre-release-cargo-update.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#precise-pre-release))
  [#13626](https://github.com/rust-lang/cargo/pull/13626)
- RFC 3491: Unused dependencies cleanup
  [#13778](https://github.com/rust-lang/cargo/pull/13778)
- `-Zcargo-lints`: Add a basic linting system for Cargo.
  This is still under development and not available for general use.
  [#13621](https://github.com/rust-lang/cargo/pull/13621)
  [#13635](https://github.com/rust-lang/cargo/pull/13635)
  [#13797](https://github.com/rust-lang/cargo/pull/13797)
  [#13740](https://github.com/rust-lang/cargo/pull/13740)
  [#13801](https://github.com/rust-lang/cargo/pull/13801)
  [#13852](https://github.com/rust-lang/cargo/pull/13852)
  [#13853](https://github.com/rust-lang/cargo/pull/13853)
- 🔥 `edition2024`: Add default Edition2024 to resolver v3 (MSRV-aware resolver).
  [#13785](https://github.com/rust-lang/cargo/pull/13785)
- `edition2024`: Remove underscore field support in 2024.
  [#13783](https://github.com/rust-lang/cargo/pull/13783)
  [#13798](https://github.com/rust-lang/cargo/pull/13798)
  [#13800](https://github.com/rust-lang/cargo/pull/13800)
  [#13804](https://github.com/rust-lang/cargo/pull/13804)
- `edition2024`: Error on `[project]` in Edition 2024
  [#13747](https://github.com/rust-lang/cargo/pull/13747)
- `-Zmsrv-policy`: Respect '--ignore-rust-version'
  [#13738](https://github.com/rust-lang/cargo/pull/13738)
- `-Zmsrv-policy`: Add `--ignore-rust-version` to update/generate-lockfile
  [#13741](https://github.com/rust-lang/cargo/pull/13741)
  [#13742](https://github.com/rust-lang/cargo/pull/13742)
- `-Zmsrv-policy`: Put MSRV-aware resolver behind a config
  [#13769](https://github.com/rust-lang/cargo/pull/13769)
- `-Zmsrv-policy`: Error, rather than panic, on rust-version 'x'
  [#13771](https://github.com/rust-lang/cargo/pull/13771)
- `-Zmsrv-policy`: Fallback to 'rustc -V' for MSRV resolving.
  [#13743](https://github.com/rust-lang/cargo/pull/13743)
- `-Zmsrv-policy`: Add v3 resolver for MSRV-aware resolving
  [#13776](https://github.com/rust-lang/cargo/pull/13776)
- `-Zmsrv-policy`: Don't respect MSRV for non-local installs
  [#13790](https://github.com/rust-lang/cargo/pull/13790)
- `-Zmsrv-policy`: Track when MSRV is explicitly set, either way
  [#13732](https://github.com/rust-lang/cargo/pull/13732)
- test: don't compress test registry crates.
  [#13744](https://github.com/rust-lang/cargo/pull/13744)

### Documentation

- Clarify `--locked` ensuring that Cargo uses dependency versions in lockfile
  [#13665](https://github.com/rust-lang/cargo/pull/13665)
- Clarify the precedence of  `RUSTC_WORKSPACE_WRAPPER` and `RUSTC_WRAPPER`.
  [#13648](https://github.com/rust-lang/cargo/pull/13648)
- Clarify only in the root Cargo.toml the `[workspace]` section is allowed.
  [#13753](https://github.com/rust-lang/cargo/pull/13753)
- Clarify the differences between virtual and real manifests.
  [#13794](https://github.com/rust-lang/cargo/pull/13794)

### Internal

- 🎉 New member crates [`cargo-test-support`](https://crates.io/crates/cargo-test-support)
  and [`cargo-test-macro`](https://crates.io/crates/cargo-test-macro)!
  They are designed for testing Cargo itself,
  so no guarantee on any stability across versions.
  The crates.io publish of this crate is the same as other members crates.
  They follow Rust's [6-week release process](https://doc.crates.io/contrib/process/release.html#cratesio-publishing).
  [#13418](https://github.com/rust-lang/cargo/pull/13418)
- Fix publish script due to crates.io CDN change
  [#13614](https://github.com/rust-lang/cargo/pull/13614)
- Push diagnostic complexity on annotate-snippets
  [#13619](https://github.com/rust-lang/cargo/pull/13619)
- cargo-package: Simplify getting of published Manifest
  [#13666](https://github.com/rust-lang/cargo/pull/13666)
- ci: update macos images to macos-13
  [#13685](https://github.com/rust-lang/cargo/pull/13685)
- manifest: Split out an explicit step to resolve `Cargo.toml`
  [#13693](https://github.com/rust-lang/cargo/pull/13693)
- manifest: Decouple target discovery from Target creation
  [#13701](https://github.com/rust-lang/cargo/pull/13701)
- manifest: Expose surce/spans for VirtualManifests
  [#13603](https://github.com/rust-lang/cargo/pull/13603)
- Update dependencies
  [#13609](https://github.com/rust-lang/cargo/pull/13609)
  [#13674](https://github.com/rust-lang/cargo/pull/13674)
  [#13675](https://github.com/rust-lang/cargo/pull/13675)
  [#13679](https://github.com/rust-lang/cargo/pull/13679)
  [#13680](https://github.com/rust-lang/cargo/pull/13680)
  [#13692](https://github.com/rust-lang/cargo/pull/13692)
  [#13731](https://github.com/rust-lang/cargo/pull/13731)
  [#13760](https://github.com/rust-lang/cargo/pull/13760)
  [#13950](https://github.com/rust-lang/cargo/pull/13950)

## Cargo 1.78 (2024-05-02)
[7bb7b539...rust-1.78.0](https://github.com/rust-lang/cargo/compare/7bb7b539...rust-1.78.0)

### Added

- Stabilize global cache data tracking. The `-Zgc` flag is still unstable.
  This is only for Cargo to start data collection, so that when automatic gc
  is stabilized, it's less likely to see cache misses.
  [#13492](https://github.com/rust-lang/cargo/pull/13492)
  [#13467](https://github.com/rust-lang/cargo/pull/13467)
- Stabilize lockfile format v4. Lockfile v3 is still the default version.
  [#12852](https://github.com/rust-lang/cargo/pull/12852)
- Auto-detecting whether output can be rendered using non-ASCII Unicode characters.
  A configuration value `term.unicode` is added to control the behavior manually.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#termunicode)
  [#13337](https://github.com/rust-lang/cargo/pull/13337)
- Support `target.<triple>.rustdocflags` in Cargo configuration.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#targettriplerustdocflags)
  [#13197](https://github.com/rust-lang/cargo/pull/13197)

### Changed

- cargo-add: Print a status when a dep feature is being created
  [#13434](https://github.com/rust-lang/cargo/pull/13434)
- cargo-add: improve the error message when adding a package from a replaced source.
  [#13281](https://github.com/rust-lang/cargo/pull/13281)
- cargo-doc: Collapse down `Generated` statuses without `--verbose`.
  [#13557](https://github.com/rust-lang/cargo/pull/13557)
- cargo-new: Print a 'Creating', rather than 'Created' status
  [#13367](https://github.com/rust-lang/cargo/pull/13367)
- cargo-new: Print a note, rather than a comment, for more information
  [#13371](https://github.com/rust-lang/cargo/pull/13371)
- cargo-new: Print a hint when adding members to workspace
  [#13411](https://github.com/rust-lang/cargo/pull/13411)
- cargo-test: Suggest `--` for libtest arguments 
  [#13448](https://github.com/rust-lang/cargo/pull/13448)
- cargo-update: Tell users when some dependencies are still behind latest.
  [#13372](https://github.com/rust-lang/cargo/pull/13372)
- Deprecate non-extension `.cargo/config` files.
  [#13349](https://github.com/rust-lang/cargo/pull/13349)
- Don't print rustdoc command lines on failure by default
  [#13387](https://github.com/rust-lang/cargo/pull/13387)
- Respect `package.rust-version` when generating new lockfiles.
  [#12861](https://github.com/rust-lang/cargo/pull/12861)
- Send `User-Agent: cargo/1.2.3` header when communicating with remote registries.
  Previously it was `cargo 1.2.3`, which didn't follow the HTTP specifications.
  [#13548](https://github.com/rust-lang/cargo/pull/13548)
- Emit a warning when `package.edition` field is missing in Cargo.toml.
  [#13499](https://github.com/rust-lang/cargo/pull/13499)
  [#13504](https://github.com/rust-lang/cargo/pull/13504)
  [#13505](https://github.com/rust-lang/cargo/pull/13505)
  [#13533](https://github.com/rust-lang/cargo/pull/13533)
- Emit warnings from parsing virtual manifests.
  [#13589](https://github.com/rust-lang/cargo/pull/13589)
- Mention the workspace root location in the error message when collecting
  workspace members.
  [#13480](https://github.com/rust-lang/cargo/pull/13480)
- Clarify the profile in use in `Finished` status message.
  [#13422](https://github.com/rust-lang/cargo/pull/13422)
- Switched more notes/warnings to lowercase.
  [#13410](https://github.com/rust-lang/cargo/pull/13410)
- Report all packages incompatible with `package.rust-version.`, not just a random one.
  [#13514](https://github.com/rust-lang/cargo/pull/13514)

### Fixed

- cargo-add: don't add the new package to `workspace.members` if there is no
  existing workspace in Cargo.toml.
  [#13391](https://github.com/rust-lang/cargo/pull/13391)
- cargo-add: Fix markdown line break in cargo-add
  [#13400](https://github.com/rust-lang/cargo/pull/13400)
- cargo-run: use Package ID Spec match packages
  [#13335](https://github.com/rust-lang/cargo/pull/13335)
- cargo-doc: doctest searches native libs in build script outputs.
  [#13490](https://github.com/rust-lang/cargo/pull/13490)
- cargo-publish: strip also features from dev-dependencies from Cargo.toml to publish.
  [#13518](https://github.com/rust-lang/cargo/pull/13518)
- Don't duplicate comments when editing TOML via `cargo add/rm/init/new`.
  [#13402](https://github.com/rust-lang/cargo/pull/13402)
- Fix confusing error messages for sparse index replaced source.
  [#13433](https://github.com/rust-lang/cargo/pull/13433)
- Respect `CARGO_TERM_COLOR` in '--list' and '-Zhelp'.
  [#13479](https://github.com/rust-lang/cargo/pull/13479)
- Control colors of errors and help texts from clap through `CARGO_TERM_COLOR`.
  [#13463](https://github.com/rust-lang/cargo/pull/13463)
- Don't panic on empty spans in Cargo.toml.
  [#13375](https://github.com/rust-lang/cargo/pull/13375)
  [#13376](https://github.com/rust-lang/cargo/pull/13376)

### Nightly only

- 🔥 cargo-update: allows `--precise` to specify a yanked version of a package
  [#13333](https://github.com/rust-lang/cargo/pull/13333)
- `-Zcheck-cfg`: Add `docsrs` cfg as a well known `--check-cfg`
  [#13383](https://github.com/rust-lang/cargo/pull/13383)
- `-Zcheck-cfg`: Silently ignore `cargo::rustc-check-cfg` to avoid MSRV
  annoyance when stabilizing `-Zcheck-cfg`.
  [#13438](https://github.com/rust-lang/cargo/pull/13438)
- `-Zmsrv-policy`: Fallback to `rustc -v` when no MSRV is set 
  [#13516](https://github.com/rust-lang/cargo/pull/13516)
- `-Zscript`: Improve errors related to cargo script
  [#13346](https://github.com/rust-lang/cargo/pull/13346)
- `-Zpanic-abort-tests`: applies to doctests too
  [#13388](https://github.com/rust-lang/cargo/pull/13388)
- `-Zpublic-dependency`: supports enabling via the `-Zpublic-dependency` flag.
  [#13340](https://github.com/rust-lang/cargo/pull/13340)
  [#13556](https://github.com/rust-lang/cargo/pull/13556)
  [#13547](https://github.com/rust-lang/cargo/pull/13547)
- `-Zpublic-dependency`: test for packaging a public dependency
  [#13536](https://github.com/rust-lang/cargo/pull/13536)
- `-Zrustdoc-map`: Add all unit's children recursively for `doc.extern-map` option 
  [#13481](https://github.com/rust-lang/cargo/pull/13481)
  [#13544](https://github.com/rust-lang/cargo/pull/13544)
- `edition2024`: Enable edition migration for 2024.
  [#13429](https://github.com/rust-lang/cargo/pull/13429)
- `open-namespaces`: basic support for open namespaces
  ([RFC 3243](https://github.com/rust-lang/rfcs/blob/master/text/3243-packages-as-optional-namespaces.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#open-namespaces))
  [#13591](https://github.com/rust-lang/cargo/pull/13591)

### Documentation

- cargo-fetch: hide `cargo-fetch` recursive link in `--offline` man page.
  [#13364](https://github.com/rust-lang/cargo/pull/13364)
- cargo-install: `--list` option description starting with uppercase
  [#13344](https://github.com/rust-lang/cargo/pull/13344)
- cargo-vendor: clarify vendored sources as read-only and ways to modify them
  [#13512](https://github.com/rust-lang/cargo/pull/13512)
- build-script: clarification of build script metadata set via `cargo::metadata=KEY=VALUE`.
  [#13436](https://github.com/rust-lang/cargo/pull/13436)
- Clarify the `version` field in `[package]` is optional in Cargo.toml
  [#13390](https://github.com/rust-lang/cargo/pull/13390)
- Improve "Registry Authentication" docs
  [#13351](https://github.com/rust-lang/cargo/pull/13351)
- Improve "Specifying Dependencies" docs
  [#13341](https://github.com/rust-lang/cargo/pull/13341)
- Remove `package.documentation` from the “before publishing” list.
  [#13398](https://github.com/rust-lang/cargo/pull/13398)

### Internal

- 🎉 Integrated tracing-chrome as a basic profiler for Cargo itself.
  [docs](https://doc.crates.io/contrib/tests/profiling.html)
  [#13399](https://github.com/rust-lang/cargo/pull/13399)
  [#13551](https://github.com/rust-lang/cargo/pull/13551)
- Updated to `gix` 0.58.0
  [#13380](https://github.com/rust-lang/cargo/pull/13380)
- Updated to `git2` 0.18.2
  [#13412](https://github.com/rust-lang/cargo/pull/13412)
- Updated to `jobserver` 0.1.28 
  [#13419](https://github.com/rust-lang/cargo/pull/13419)
- Updated to `supports-hyperlinks` 3.0.0
  [#13511](https://github.com/rust-lang/cargo/pull/13511)
- Updated to `rusqlite` 0.31.0
  [#13510](https://github.com/rust-lang/cargo/pull/13510)
- bump-check: use symmetric difference when comparing source code
    [#13581](https://github.com/rust-lang/cargo/pull/13581)
- bump-check: include rustfix and cargo-util-schemas
    [#13421](https://github.com/rust-lang/cargo/pull/13421)
- ci: enable m1 runner
  [#13377](https://github.com/rust-lang/cargo/pull/13377)
- ci: Ensure lockfile is respected during MSRV testing via `cargo-hack`.
  [#13523](https://github.com/rust-lang/cargo/pull/13523)
- cargo-util-schemas: Consistently compare MSRVs via `RustVersion::is_compatible_with`.
  [#13537](https://github.com/rust-lang/cargo/pull/13537)
- console: Use new fancy `anstyle` API
  [#13368](https://github.com/rust-lang/cargo/pull/13368)
  [#13562](https://github.com/rust-lang/cargo/pull/13562)
- fingerprint: remove unnecessary Option in `Freshness::Dirty`
  [#13361](https://github.com/rust-lang/cargo/pull/13361)
- fingerprint: abstract `std::fs` away from on-disk index cache
  [#13515](https://github.com/rust-lang/cargo/pull/13515)
- mdman: Updated to `pulldown-cmark` 0.10.0
  [#13517](https://github.com/rust-lang/cargo/pull/13517)
- refactor: Renamed `Config` to `GlobalContext` 
  [#13409](https://github.com/rust-lang/cargo/pull/13409)
  [#13486](https://github.com/rust-lang/cargo/pull/13486)
  [#13506](https://github.com/rust-lang/cargo/pull/13506)
- refactor: Removed unused `sysroot_host_libdir`.
  [#13468](https://github.com/rust-lang/cargo/pull/13468)
- refactor: Expose source/spans to Manifest for emitting lints
  [#13593](https://github.com/rust-lang/cargo/pull/13593)
- refactor: Flatten manifest parsing 
  [#13589](https://github.com/rust-lang/cargo/pull/13589)
- refactor: Make lockfile diffing/printing more reusable
  [#13564](https://github.com/rust-lang/cargo/pull/13564)
- test: Updated to `snapbox` 0.5.0
  [#13441](https://github.com/rust-lang/cargo/pull/13441)
- test: Verify terminal styling via snapbox's `term-svg` feature.
  [#13461](https://github.com/rust-lang/cargo/pull/13461)
  [#13465](https://github.com/rust-lang/cargo/pull/13465)
  [#13520](https://github.com/rust-lang/cargo/pull/13520)
- test: Ensure `nonzero_exit_code` test isn't affected by developers `RUST_BACKTRACE` setting 
  [#13385](https://github.com/rust-lang/cargo/pull/13385)
- test: Add tests for using worktrees.
  [#13567](https://github.com/rust-lang/cargo/pull/13567)
- test: Fix old_cargos tests 
  [#13435](https://github.com/rust-lang/cargo/pull/13435)
- test: Fixed tests due to changes in rust-lang/rust.
  [#13362](https://github.com/rust-lang/cargo/pull/13362)
  [#13382](https://github.com/rust-lang/cargo/pull/13382)
  [#13415](https://github.com/rust-lang/cargo/pull/13415)
  [#13424](https://github.com/rust-lang/cargo/pull/13424)
  [#13444](https://github.com/rust-lang/cargo/pull/13444)
  [#13455](https://github.com/rust-lang/cargo/pull/13455)
  [#13464](https://github.com/rust-lang/cargo/pull/13464)
  [#13466](https://github.com/rust-lang/cargo/pull/13466)
  [#13469](https://github.com/rust-lang/cargo/pull/13469)
- test: disable lldb test as it requires privileges to run on macOS 
  [#13416](https://github.com/rust-lang/cargo/pull/13416)

## Cargo 1.77.1 (2024-03-28)

### Fixed

- Debuginfo is no longer stripped by default for Windows MSVC targets. This caused an unexpected regression in 1.77.0 that broke backtraces.
  [#13654](https://github.com/rust-lang/cargo/pull/13654)

## Cargo 1.77 (2024-03-21)
[1a2666dd...rust-1.77.0](https://github.com/rust-lang/cargo/compare/1a2666dd...rust-1.77.0)

### Added

- 🎉 Stabilize the package identifier format as [Package ID Spec](https://doc.rust-lang.org/nightly/cargo/reference/pkgid-spec.html).
  This format can be used across most of the commands in Cargo, including the
  `--package`/`-p` flag, `cargo pkgid`, `cargo metadata`, and JSON messages
  from `--message-format=json`.
  [#12914](https://github.com/rust-lang/cargo/pull/12914)
  [#13202](https://github.com/rust-lang/cargo/pull/13202)
  [#13311](https://github.com/rust-lang/cargo/pull/13311)
  [#13298](https://github.com/rust-lang/cargo/pull/13298)
  [#13322](https://github.com/rust-lang/cargo/pull/13322)
- Add colors to `-Zhelp` console output
  [#13269](https://github.com/rust-lang/cargo/pull/13269)
- build script: Extend the build directive syntax with `cargo::`.
  [#12201](https://github.com/rust-lang/cargo/pull/12201)
  [#13212](https://github.com/rust-lang/cargo/pull/13212)

### Changed

- 🎉 Disabling debuginfo now implies `strip = "debuginfo"` (when `strip` is not set)
  to strip pre-existing debuginfo coming from the standard library,
  reducing the default size of release binaries considerably
  (from ~4.5 MiB down to ~450 KiB for helloworld on Linux x64).
  [#13257](https://github.com/rust-lang/cargo/pull/13257)
- Add `rustc` style errors for manifest parsing.
  [#13172](https://github.com/rust-lang/cargo/pull/13172)
- Deprecate rustc plugin support in cargo
  [#13248](https://github.com/rust-lang/cargo/pull/13248)
- cargo-vendor: Hold the mutate exclusive lock when vendoring.
  [#12509](https://github.com/rust-lang/cargo/pull/12509)
- crates-io: Set `Content-Type: application/json` only for requests with a body payload
  [#13264](https://github.com/rust-lang/cargo/pull/13264)

### Fixed

- jobserver: inherit jobserver from env for all kinds of runner
  [#12776](https://github.com/rust-lang/cargo/pull/12776)
- build script: Set `OUT_DIR` for all units with build scripts
  [#13204](https://github.com/rust-lang/cargo/pull/13204)
- cargo-add: find the correct package with given features from Git repositories
  with multiple packages.
  [#13213](https://github.com/rust-lang/cargo/pull/13213)
- cargo-fix: always inherit the jobserver
  [#13225](https://github.com/rust-lang/cargo/pull/13225)
- cargo-fix: Call rustc fewer times to improve the performance.
  [#13243](https://github.com/rust-lang/cargo/pull/13243)
- cargo-new: only inherit workspace package table if the new package is a member
  [#13261](https://github.com/rust-lang/cargo/pull/13261)
- cargo-update: `--precise` accepts arbitrary git revisions
  [#13250](https://github.com/rust-lang/cargo/pull/13250)
- manifest: Provide unused key warnings for lints table
  [#13262](https://github.com/rust-lang/cargo/pull/13262)
- rustfix: Support inserting new lines.
  [#13226](https://github.com/rust-lang/cargo/pull/13226)

### Nightly only

- 🔥 `-Zgit`: Implementation of shallow libgit2 fetches behind an unstable flag
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#git)
  [#13252](https://github.com/rust-lang/cargo/pull/13252)
- 🔥 Add unstable `--output-format` option to  `cargo rustdoc`, providing tools
  with a way to lean on rustdoc’s experimental JSON format.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#output-format-for-rustdoc)
  [#12252](https://github.com/rust-lang/cargo/pull/12252)
  [#13284](https://github.com/rust-lang/cargo/pull/13284)
  [#13325](https://github.com/rust-lang/cargo/pull/13325)
- `-Zcheck-cfg`: Rework `--check-cfg` generation comment
  [#13195](https://github.com/rust-lang/cargo/pull/13195)
- `-Zcheck-cfg`: Go back to passing an empty `values()` when no features are declared
  [#13316](https://github.com/rust-lang/cargo/pull/13316)
- `-Zprecise-pre-release`: the flag is added but not implemented yet.
  [#13296](https://github.com/rust-lang/cargo/pull/13296)
  [#13320](https://github.com/rust-lang/cargo/pull/13320)
- `-Zpublic-dependency`: support publish package with a `public` field.
  [#13245](https://github.com/rust-lang/cargo/pull/13245)
- `-Zpublic-dependency`: help text of `--public`/`--no-public` flags for `cargo add`
  [#13272](https://github.com/rust-lang/cargo/pull/13272)
- `-Zscript`: Add prefix-char frontmatter syntax support
  [#13247](https://github.com/rust-lang/cargo/pull/13247)
- `-Zscript`: Add multiple experimental manifest syntaxes
  [#13241](https://github.com/rust-lang/cargo/pull/13241)
- `-Ztrim-paths`: remap common prefix only
  [#13210](https://github.com/rust-lang/cargo/pull/13210)

### Documentation

- Added guidance on setting homepage in manifest
  [#13293](https://github.com/rust-lang/cargo/pull/13293)
- Clarified how custom subcommands are looked up.
  [#13203](https://github.com/rust-lang/cargo/pull/13203)
- Clarified why `du` function uses mutex
  [#13273](https://github.com/rust-lang/cargo/pull/13273)
- Highlighted "How to find features enabled on dependencies"
  [#13305](https://github.com/rust-lang/cargo/pull/13305)
- Delete sentence about parentheses being unsupported in license
  [#13292](https://github.com/rust-lang/cargo/pull/13292)
- resolver: clarify how pre-release version is handled in dependency resolution.
  [#13286](https://github.com/rust-lang/cargo/pull/13286)
- cargo-test: clarify the target selection of the test options.
  [#13236](https://github.com/rust-lang/cargo/pull/13236)
- cargo-install: clarify `--path` is the installation source not destination
  [#13205](https://github.com/rust-lang/cargo/pull/13205)
- contrib: Fix team HackMD links
  [#13237](https://github.com/rust-lang/cargo/pull/13237)
- contrib: Highlight the non-blocking feature gating technique
  [#13307](https://github.com/rust-lang/cargo/pull/13307)

### Internal

- 🎉 New member crate [`cargo-util-schemas`](https://crates.io/crates/cargo-util-schemas)!
  This contains low-level Cargo schema types, focusing on `serde` and `FromStr`
  for use in reading files and parsing command-lines.
  Any logic for getting final semantics from these will likely need other tools
  to process, like `cargo metadata`.
  The crates.io publish of this crate is the same as other members crates.
  It follows Rust's [6-week release process](https://doc.crates.io/contrib/process/release.html#cratesio-publishing).
  [#13178](https://github.com/rust-lang/cargo/pull/13178)
  [#13185](https://github.com/rust-lang/cargo/pull/13185)
  [#13186](https://github.com/rust-lang/cargo/pull/13186)
  [#13209](https://github.com/rust-lang/cargo/pull/13209)
  [#13267](https://github.com/rust-lang/cargo/pull/13267)
- Updated to `gix` 0.57.1.
  [#13230](https://github.com/rust-lang/cargo/pull/13230)
- cargo-fix: Remove error-format special-case in `cargo fix`
  [#13224](https://github.com/rust-lang/cargo/pull/13224)
- cargo-credential: bump to 0.4.3
  [#13221](https://github.com/rust-lang/cargo/pull/13221)
- mdman: updated to `handlebars` 5.0.0.
  [#13168](https://github.com/rust-lang/cargo/pull/13168)
  [#13249](https://github.com/rust-lang/cargo/pull/13249)
- rustfix: remove useless clippy rules and fix a typo
  [#13182](https://github.com/rust-lang/cargo/pull/13182)
- ci: fix Dependabot's MSRV auto-update
  [#13265](https://github.com/rust-lang/cargo/pull/13265)
  [#13324](https://github.com/rust-lang/cargo/pull/13324)
  [#13268](https://github.com/rust-lang/cargo/pull/13268)
- ci: Add [dependency dashboard](https://github.com/rust-lang/cargo/issues/13256).
  [#13255](https://github.com/rust-lang/cargo/pull/13255)
- ci: update alpine docker tag to v3.19
  [#13228](https://github.com/rust-lang/cargo/pull/13228)
- ci: Improve GitHub Actions CI config
  [#13317](https://github.com/rust-lang/cargo/pull/13317)
- resolver: do not panic when sorting empty summaries
  [#13287](https://github.com/rust-lang/cargo/pull/13287)

## Cargo 1.76 (2024-02-08)
[6790a512...rust-1.76.0](https://github.com/rust-lang/cargo/compare/6790a512...rust-1.76.0)

### Added

- Added a Windows application manifest file to the built `cargo.exe` for windows msvc.
  [#13131](https://github.com/rust-lang/cargo/pull/13131)  
  Notable changes:
  - States the compatibility with Windows versions 7, 8, 8.1, 10 and 11.
  - Sets the code page to UTF-8.
  - Enables long path awareness.
- Added color output for `cargo --list`.
  [#12992](https://github.com/rust-lang/cargo/pull/12992)
- cargo-add: `--optional <dep>` would create a `<dep> = "dep:<dep>"` feature.
  [#13071](https://github.com/rust-lang/cargo/pull/13071)
- Extends Package ID spec for unambiguous specs.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/pkgid-spec.html)
  [#12933](https://github.com/rust-lang/cargo/pull/12933)  
  Specifically,
  - Supports `git+` and `path+` schemes.
  - Supports Git ref query strings, such as `?branch=dev` or `?tag=1.69.0`.

### Changed

- ❗️ Disallow `[lints]` in virtual workspaces as they are ignored  and users likely meant `[workspace.lints]`.
  This was an oversight in the initial implementation (e.g. a `[dependencies]` produces the same error).
  [#13155](https://github.com/rust-lang/cargo/pull/13155)
- Disallow empty name in several places like package ID spec and `cargo new`.
  [#13152](https://github.com/rust-lang/cargo/pull/13152)
- Respect `rust-lang/rust`'s `omit-git-hash` option.
  [#12968](https://github.com/rust-lang/cargo/pull/12968)
- Displays error count with a number, even when there is only one error.
  [#12484](https://github.com/rust-lang/cargo/pull/12484)
- `all-static` feature now includes `vendored-libgit2`.
  [#13134](https://github.com/rust-lang/cargo/pull/13134)
- crates-io: Add support for other 2xx HTTP status codes when interacting with registries.
  [#13158](https://github.com/rust-lang/cargo/pull/13158)
  [#13160](https://github.com/rust-lang/cargo/pull/13160)
- home: Replace SHGetFolderPathW with SHGetKnownFolderPath.
  [#13173](https://github.com/rust-lang/cargo/pull/13173)

### Fixed

- Print rustc messages colored on wincon.
  [#13140](https://github.com/rust-lang/cargo/pull/13140)
- Fixed bash completion in directory with spaces.
  [#13126](https://github.com/rust-lang/cargo/pull/13126)
- Fixed uninstall a running binary failed on Windows.
  [#13053](https://github.com/rust-lang/cargo/pull/13053)
  [#13099](https://github.com/rust-lang/cargo/pull/13099)
- Fixed the error message for duplicate links.
  [#12973](https://github.com/rust-lang/cargo/pull/12973)
- Fixed `--quiet` being used with nested subcommands.
  [#12959](https://github.com/rust-lang/cargo/pull/12959)
- Fixed panic when there is a cycle in dev-dependencies.
  [#12977](https://github.com/rust-lang/cargo/pull/12977)
- Don't panic when failed to parse rustc commit-hash.
  [#12963](https://github.com/rust-lang/cargo/pull/12963)
  [#12965](https://github.com/rust-lang/cargo/pull/12965)
- Don't do git fetches when updating workspace members.
  [#12975](https://github.com/rust-lang/cargo/pull/12975)
- Avoid writing CACHEDIR.TAG if it already exists.
  [#13132](https://github.com/rust-lang/cargo/pull/13132)
- Accept `?` in the `--package` flag if it's a valid pkgid spec.
  [#13315](https://github.com/rust-lang/cargo/pull/13315)
  [#13318](https://github.com/rust-lang/cargo/pull/13318)
- cargo-package: Only filter out `target` directory if it's in the package root.
  [#12944](https://github.com/rust-lang/cargo/pull/12944)
- cargo-package: errors out when a build script doesn't exist or is outside the package root.
  [#12995](https://github.com/rust-lang/cargo/pull/12995)
- cargo-credential-1password: Add missing `--account` argument to `op signin` command.
  [#12985](https://github.com/rust-lang/cargo/pull/12985)
  [#12986](https://github.com/rust-lang/cargo/pull/12986)


### Nightly only

- 🔥 The `-Zgc` flag enables garbage collection for deleting old, unused files
  in cargo's cache. That is, downloaded source files and registry index under
  the `CARGO_HOME` directory.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#gc)
  [#12634](https://github.com/rust-lang/cargo/pull/12634)
  [#12958](https://github.com/rust-lang/cargo/pull/12958)
  [#12981](https://github.com/rust-lang/cargo/pull/12981)
  [#13055](https://github.com/rust-lang/cargo/pull/13055)
- 🔥 Added a new environment variable `CARGO_RUSTC_CURRENT_DIR`.
  This is a path that rustc is invoked from.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/environment-variables.html?highlight=CARGO_RUSTC_CURRENT_DIR#environment-variables-cargo-sets-for-crates)
  [#12996](https://github.com/rust-lang/cargo/pull/12996)
- `-Zcheck-cfg`: Include declared list of features in fingerprint for `-Zcheck-cfg`.
  [#13012](https://github.com/rust-lang/cargo/pull/13012)
- `-Zcheck-cfg`: Fix `--check-cfg` invocations with zero features.
  [#13011](https://github.com/rust-lang/cargo/pull/13011)
- `-Ztrim-paths`: reorder `--remap-path-prefix` flags for `-Zbuild-std`.
  [#13065](https://github.com/rust-lang/cargo/pull/13065)
- `-Ztrim-paths`: explicitly remap current dir by using `.`.
  [#13114](https://github.com/rust-lang/cargo/pull/13114)
- `-Ztrim-paths`: exercise with real world debugger.
  [#13091](https://github.com/rust-lang/cargo/pull/13091)
  [#13118](https://github.com/rust-lang/cargo/pull/13118)
- `-Zpublic-dependency`: Limit `exported-private-dependencies` lints to libraries.
  [#13135](https://github.com/rust-lang/cargo/pull/13135)
- `-Zpublic-dependency`: Disallow workspace-inheriting of dependency public status.
  [#13125](https://github.com/rust-lang/cargo/pull/13125)
- `-Zpublic-dependency`: Add `--public` for `cargo add`.
  [#13046](https://github.com/rust-lang/cargo/pull/13046)
- `-Zpublic-dependency`: Remove unused public-deps error handling 
  [#13036](https://github.com/rust-lang/cargo/pull/13036)
- `-Zmsrv-policy`: Prefer MSRV, rather than ignore incompatible.
  [#12950](https://github.com/rust-lang/cargo/pull/12950)
- `-Zmsrv-policy`: De-prioritize no-rust-version in MSRV resolver.
  [#13066](https://github.com/rust-lang/cargo/pull/13066)
- `-Zrustdoc-scrape-examples`: Don't filter on workspace members when scraping doc examples.
  [#13077](https://github.com/rust-lang/cargo/pull/13077)

### Documentation

- Recommends a wider selection of libsecret-compatible password managers.
  [#12993](https://github.com/rust-lang/cargo/pull/12993)
- Clarified different targets has different sets of `CARGO_CFG_*` values.
  [#13069](https://github.com/rust-lang/cargo/pull/13069)
- Clarified `[lints]` table only affects local development of the current package.
  [#12976](https://github.com/rust-lang/cargo/pull/12976)
- Clarified `cargo search` can search in alternative registries.
  [#12962](https://github.com/rust-lang/cargo/pull/12962)
- Added common CI practices for verifying `rust-version` (MSRV) field.
  [#13056](https://github.com/rust-lang/cargo/pull/13056)
- Added a link to rustc lint levels doc.
  [#12990](https://github.com/rust-lang/cargo/pull/12990)
- Added a link to the packages lint table from the related workspace table 
  [#13057](https://github.com/rust-lang/cargo/pull/13057)
- contrib: Add more resources to the contrib docs.
  [#13008](https://github.com/rust-lang/cargo/pull/13008)
- contrib: Update how that credential crates are published. 
  [#13006](https://github.com/rust-lang/cargo/pull/13006)
- contrib: remove review capacity notice.
  [#13070](https://github.com/rust-lang/cargo/pull/13070)

### Internal

- 🎉 Migrate `rustfix` crate to the `rust-lang/cargo` repository.
  [#13005](https://github.com/rust-lang/cargo/pull/13005)
  [#13042](https://github.com/rust-lang/cargo/pull/13042)
  [#13047](https://github.com/rust-lang/cargo/pull/13047)
  [#13048](https://github.com/rust-lang/cargo/pull/13048)
  [#13050](https://github.com/rust-lang/cargo/pull/13050)
- Updated to `curl-sys` 0.4.70, which corresponds to curl 8.4.0.
  [#13147](https://github.com/rust-lang/cargo/pull/13147)
- Updated to `gix-index` 0.27.1.
  [#13148](https://github.com/rust-lang/cargo/pull/13148)
- Updated to `itertools` 0.12.0.
  [#13086](https://github.com/rust-lang/cargo/pull/13086)
- Updated to `rusqlite` 0.30.0.
  [#13087](https://github.com/rust-lang/cargo/pull/13087)
- Updated to `toml_edit` 0.21.0.
  [#13088](https://github.com/rust-lang/cargo/pull/13088)
- Updated to `windows-sys` 0.52.0.
  [#13089](https://github.com/rust-lang/cargo/pull/13089)
- Updated to `tracing` 0.1.37 for being be compatible with rustc_log.
  [#13239](https://github.com/rust-lang/cargo/pull/13239)
  [#13242](https://github.com/rust-lang/cargo/pull/13242)
- Re-enable flaky gitoxide auth tests thanks to update to `gix-config`.
  [#13117](https://github.com/rust-lang/cargo/pull/13117)
  [#13129](https://github.com/rust-lang/cargo/pull/13129)
  [#13130](https://github.com/rust-lang/cargo/pull/13130)
- Dogfood Cargo `-Zlints` table feature.
  [#12178](https://github.com/rust-lang/cargo/pull/12178)
- Refactored `Cargo.toml` parsing code in preparation of extracting an official
  schema API.
  [#12954](https://github.com/rust-lang/cargo/pull/12954)
  [#12960](https://github.com/rust-lang/cargo/pull/12960)
  [#12961](https://github.com/rust-lang/cargo/pull/12961)
  [#12971](https://github.com/rust-lang/cargo/pull/12971)
  [#13000](https://github.com/rust-lang/cargo/pull/13000)
  [#13021](https://github.com/rust-lang/cargo/pull/13021)
  [#13080](https://github.com/rust-lang/cargo/pull/13080)
  [#13097](https://github.com/rust-lang/cargo/pull/13097)
  [#13123](https://github.com/rust-lang/cargo/pull/13123)
  [#13128](https://github.com/rust-lang/cargo/pull/13128)
  [#13154](https://github.com/rust-lang/cargo/pull/13154)
  [#13166](https://github.com/rust-lang/cargo/pull/13166)
- Use `IndexSummary` in `query{_vec}` functions.
  [#12970](https://github.com/rust-lang/cargo/pull/12970)
- ci: migrate renovate config 
  [#13106](https://github.com/rust-lang/cargo/pull/13106)
- ci: Always update gix packages together 
  [#13093](https://github.com/rust-lang/cargo/pull/13093)
- ci: Catch naive use of AtomicU64 early 
  [#12988](https://github.com/rust-lang/cargo/pull/12988)
- xtask-bump-check: dont check `home` against beta/stable branches 
  [#13167](https://github.com/rust-lang/cargo/pull/13167)
- cargo-test-support: Handle $message_type in JSON diagnostics 
  [#13016](https://github.com/rust-lang/cargo/pull/13016)
- cargo-test-support: Add more options to registry test support. 
  [#13085](https://github.com/rust-lang/cargo/pull/13085)
- cargo-test-support: Add features to the default Cargo.toml file 
  [#12997](https://github.com/rust-lang/cargo/pull/12997)
- cargo-test-support: Fix clippy-wrapper test race condition. 
  [#12999](https://github.com/rust-lang/cargo/pull/12999)
- test: Don't rely on mtime to test changes 
  [#13143](https://github.com/rust-lang/cargo/pull/13143)
- test: remove unnecessary packages and versions for `optionals` tests 
  [#13108](https://github.com/rust-lang/cargo/pull/13108)
- test: Remove the deleted feature `test_2018_feature` from the test.
  [#13156](https://github.com/rust-lang/cargo/pull/13156)
- test: remove jobserver env var in some tests.
  [#13072](https://github.com/rust-lang/cargo/pull/13072)
- test: Fix a rustflags test using a wrong buildfile name 
  [#12987](https://github.com/rust-lang/cargo/pull/12987)
- test: Fix some test output validation. 
  [#12982](https://github.com/rust-lang/cargo/pull/12982)
- test: Ignore changing_spec_relearns_crate_types on windows-gnu 
  [#12972](https://github.com/rust-lang/cargo/pull/12972)

## Cargo 1.75 (2023-12-28)
[59596f0f...rust-1.75.0](https://github.com/rust-lang/cargo/compare/59596f0f...rust-1.75.0)

### Added

- `package.version` field in `Cargo.toml` is now optional and defaults to `0.0.0`.
  Packages without the `package.version` field cannot be published.
  [#12786](https://github.com/rust-lang/cargo/pull/12786)
- Links in `--timings` and `cargo doc` outputs are clickable on supported terminals,
  controllable through `term.hyperlinks` config value.
  [#12889](https://github.com/rust-lang/cargo/pull/12889)
- Print environment variables for build script executions with `-vv`.
  [#12829](https://github.com/rust-lang/cargo/pull/12829)
- cargo-new: add new packages to [workspace.members] automatically.
  [#12779](https://github.com/rust-lang/cargo/pull/12779)
- cargo-doc: print a new `Generated` status displaying the full path.
  [#12859](https://github.com/rust-lang/cargo/pull/12859)

### Changed

- cargo-new: warn if crate name doesn't follow snake_case or kebab-case.
  [#12766](https://github.com/rust-lang/cargo/pull/12766)
- cargo-install: clarify the arg `<crate>` to install is positional.
  [#12841](https://github.com/rust-lang/cargo/pull/12841)
- cargo-install: Suggest an alternative version on MSRV failure.
  [#12798](https://github.com/rust-lang/cargo/pull/12798)
- cargo-install: reports more detailed SemVer errors.
  [#12924](https://github.com/rust-lang/cargo/pull/12924)
- cargo-install: install only once if there are crates duplicated.
  [#12868](https://github.com/rust-lang/cargo/pull/12868)
- cargo-remove: Clarify flag behavior of different dependency kinds.
  [#12823](https://github.com/rust-lang/cargo/pull/12823)
- cargo-remove: suggest the dependency to remove exists only in the other section.
  [#12865](https://github.com/rust-lang/cargo/pull/12865)
- cargo-update: Do not call it "Downgrading" when difference is only build metadata.
  [#12796](https://github.com/rust-lang/cargo/pull/12796)
- Enhanced help text to clarify `--test` flag is for Cargo targets, not test functions.
  [#12915](https://github.com/rust-lang/cargo/pull/12915)
- Included package name/version in build script warnings.
  [#12799](https://github.com/rust-lang/cargo/pull/12799)
- Provide next steps for bad -Z flag.
  [#12857](https://github.com/rust-lang/cargo/pull/12857)
- Suggest `cargo search` when `cargo-<command>` cannot be found.
  [#12840](https://github.com/rust-lang/cargo/pull/12840)
- Do not allow empty feature name.
  [#12928](https://github.com/rust-lang/cargo/pull/12928)
- Added unsupported short flag suggestion for `--target` and `--exclude` flags.
  [#12805](https://github.com/rust-lang/cargo/pull/12805)
- Added unsupported short flag suggestion for `--out-dir` flag.
  [#12755](https://github.com/rust-lang/cargo/pull/12755)
- Added unsupported lowercase `-z` flag suggestion for `-Z` flag.
  [#12788](https://github.com/rust-lang/cargo/pull/12788)
- Added better suggestion for unsupported `--path` flag.
  [#12811](https://github.com/rust-lang/cargo/pull/12811)
- Added detailed message when target directory path is invalid.
  [#12820](https://github.com/rust-lang/cargo/pull/12820)

### Fixed

- Fixed corruption when cargo was killed while writing to files.
  [#12744](https://github.com/rust-lang/cargo/pull/12744)
- cargo-add: Preserve more comments 
  [#12838](https://github.com/rust-lang/cargo/pull/12838)
- cargo-fix: preserve jobserver file descriptors on rustc invocation.
  [#12951](https://github.com/rust-lang/cargo/pull/12951)
- cargo-remove: Preserve feature comments 
  [#12837](https://github.com/rust-lang/cargo/pull/12837)
- Removed unnecessary backslash in timings HTML report when error happens.
  [#12934](https://github.com/rust-lang/cargo/pull/12934)
- Fixed error message that invalid a feature name can contain `-`.
  [#12939](https://github.com/rust-lang/cargo/pull/12939)
- When there's a version of a dependency in the lockfile,
  Cargo would use that "exact" version, including the build metadata.
  [#12772](https://github.com/rust-lang/cargo/pull/12772)

### Nightly only

- Added `Edition2024` unstable feature.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#edition-2024)
  [#12771](https://github.com/rust-lang/cargo/pull/12771)
- 🔥 The `-Ztrim-paths` feature adds a new profile setting to control how paths
  are sanitized in the resulting binary.
  ([RFC 3127](https://github.com/rust-lang/rfcs/blob/master/text/3127-trim-paths.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#profile-trim-paths-option))
  [#12625](https://github.com/rust-lang/cargo/pull/12625)
  [#12900](https://github.com/rust-lang/cargo/pull/12900)
  [#12908](https://github.com/rust-lang/cargo/pull/12908)
- `-Zcheck-cfg`: Adjusted for new rustc syntax and behavior.
  [#12845](https://github.com/rust-lang/cargo/pull/12845)
- `-Zcheck-cfg`: Remove outdated option to `-Zcheck-cfg` warnings.
  [#12884](https://github.com/rust-lang/cargo/pull/12884)
- `public-dependency`: Support `public` dependency configuration with workspace deps.
  [#12817](https://github.com/rust-lang/cargo/pull/12817)

### Documentation

- profile: add missing `strip` info.
  [#12754](https://github.com/rust-lang/cargo/pull/12754)
- features: a note about the new limit on number of features.
  [#12913](https://github.com/rust-lang/cargo/pull/12913)
- crates-io: Add doc comment for `NewCrate` struct.
  [#12782](https://github.com/rust-lang/cargo/pull/12782)
- resolver: Highlight commands to answer dep resolution questions.
  [#12903](https://github.com/rust-lang/cargo/pull/12903)
- cargo-bench: `--bench` is passed in unconditionally to bench harnesses.
  [#12850](https://github.com/rust-lang/cargo/pull/12850)
- cargo-login: mention args after `--` in manpage.
  [#12832](https://github.com/rust-lang/cargo/pull/12832)
- cargo-vendor: clarify config to use vendored source is printed to stdout 
  [#12893](https://github.com/rust-lang/cargo/pull/12893)
- manifest: update to SPDX 2.3 license expression and 3.20 license list.
  [#12827](https://github.com/rust-lang/cargo/pull/12827)
- contrib: Policy on manifest editing 
  [#12836](https://github.com/rust-lang/cargo/pull/12836)
- contrib: use `AND` search terms in mdbook search and fixed broken links.
  [#12812](https://github.com/rust-lang/cargo/pull/12812)
  [#12813](https://github.com/rust-lang/cargo/pull/12813)
  [#12814](https://github.com/rust-lang/cargo/pull/12814)
- contrib: Describe how to add a new package 
  [#12878](https://github.com/rust-lang/cargo/pull/12878)
- contrib: Removed review capacity notice.
  [#12842](https://github.com/rust-lang/cargo/pull/12842)

### Internal

- Updated to `itertools` 0.11.0.
  [#12759](https://github.com/rust-lang/cargo/pull/12759)
- Updated to `cargo_metadata` 0.18.0.
  [#12758](https://github.com/rust-lang/cargo/pull/12758)
- Updated to `curl-sys` 0.4.68, which corresponds to curl 8.4.0.
  [#12808](https://github.com/rust-lang/cargo/pull/12808)
- Updated to `toml` 0.8.2.
  [#12760](https://github.com/rust-lang/cargo/pull/12760)
- Updated to `toml_edit` 0.20.2.
  [#12761](https://github.com/rust-lang/cargo/pull/12761)
- Updated to `gix` to 0.55.2 
  [#12906](https://github.com/rust-lang/cargo/pull/12906)
- Disabled the `custom_target::custom_bin_target` test on windows-gnu.
  [#12763](https://github.com/rust-lang/cargo/pull/12763)
- Refactored `Cargo.toml` parsing code in preparation of extracting an official
  schema API.
  [#12768](https://github.com/rust-lang/cargo/pull/12768)
  [#12881](https://github.com/rust-lang/cargo/pull/12881)
  [#12902](https://github.com/rust-lang/cargo/pull/12902)
  [#12911](https://github.com/rust-lang/cargo/pull/12911)
  [#12948](https://github.com/rust-lang/cargo/pull/12948)
- Split out SemVer logic to its own module.
  [#12926](https://github.com/rust-lang/cargo/pull/12926)
  [#12940](https://github.com/rust-lang/cargo/pull/12940)
- source: Prepare for new `PackageIDSpec` syntax
  [#12938](https://github.com/rust-lang/cargo/pull/12938)
- resolver: Consolidate logic in `VersionPreferences` 
  [#12930](https://github.com/rust-lang/cargo/pull/12930)
- Make the `SourceId::precise` field an Enum.
  [#12849](https://github.com/rust-lang/cargo/pull/12849)
- shell: Write at once rather than in fragments.
  [#12880](https://github.com/rust-lang/cargo/pull/12880)
- Move up looking at index summary enum 
  [#12749](https://github.com/rust-lang/cargo/pull/12749)
  [#12923](https://github.com/rust-lang/cargo/pull/12923)
- Generate redirection HTML pages in CI for Cargo Contributor Guide.
  [#12846](https://github.com/rust-lang/cargo/pull/12846)
- Add new package cache lock modes.
  [#12706](https://github.com/rust-lang/cargo/pull/12706)
- Add regression test for issue 6915: features and transitive dev deps.
  [#12907](https://github.com/rust-lang/cargo/pull/12907)
- Auto-labeling when PR review state changes.
  [#12856](https://github.com/rust-lang/cargo/pull/12856)
- credential: include license files in all published crates.
  [#12953](https://github.com/rust-lang/cargo/pull/12953)
- credential: Filter `cargo-credential-*` dependencies by OS.
  [#12949](https://github.com/rust-lang/cargo/pull/12949)
- ci: bump cargo-semver-checks to 0.24.0
  [#12795](https://github.com/rust-lang/cargo/pull/12795)
- ci: set and verify all MSRVs for Cargo's crates automatically.
  [#12767](https://github.com/rust-lang/cargo/pull/12767)
  [#12654](https://github.com/rust-lang/cargo/pull/12654)
- ci: use separate concurrency group for publishing Cargo Contributor Book.
  [#12834](https://github.com/rust-lang/cargo/pull/12834)
  [#12835](https://github.com/rust-lang/cargo/pull/12835)
- ci: update `actions/checkout` action to v4
  [#12762](https://github.com/rust-lang/cargo/pull/12762)
- cargo-search: improved the margin calculation for the output.
  [#12890](https://github.com/rust-lang/cargo/pull/12890)

## Cargo 1.74 (2023-11-16)
[80eca0e5...rust-1.74.0](https://github.com/rust-lang/cargo/compare/80eca0e5...rust-1.74.0)

### Added

- 🎉 The `[lints]` table has been stabilized, allowing you to configure reporting levels for rustc and other tool lints in `Cargo.toml`.
  ([RFC 3389](https://github.com/rust-lang/rfcs/blob/master/text/3389-manifest-lint.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/manifest.html#the-lints-section))
  [#12584](https://github.com/rust-lang/cargo/pull/12584)
  [#12648](https://github.com/rust-lang/cargo/pull/12648)
- 🎉 The unstable features `credential-process` and `registry-auth` have been stabilized.
  These features consolidate the way to authenticate with private registries.
  ([RFC 2730](https://github.com/rust-lang/rfcs/blob/master/text/2730-cargo-token-from-process.md))
  ([RFC 3139](https://github.com/rust-lang/rfcs/blob/master/text/3139-cargo-alternative-registry-auth.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/registry-authentication.html))
  [#12590](https://github.com/rust-lang/cargo/pull/12590)
  [#12622](https://github.com/rust-lang/cargo/pull/12622)
  [#12623](https://github.com/rust-lang/cargo/pull/12623)
  [#12626](https://github.com/rust-lang/cargo/pull/12626)
  [#12641](https://github.com/rust-lang/cargo/pull/12641)
  [#12644](https://github.com/rust-lang/cargo/pull/12644)
  [#12649](https://github.com/rust-lang/cargo/pull/12649)
  [#12671](https://github.com/rust-lang/cargo/pull/12671)
  [#12709](https://github.com/rust-lang/cargo/pull/12709)  
  Notable changes:
  - Introducing a new protocol for both external and built-in providers to store and retrieve credentials for registry authentication.
  - Adding the `auth-required` field in the registry index's `config.json`, enabling authenticated sparse index, crate downloads, and search API.
  - For using alternative registries with authentication, a credential provider must be configured to avoid unknowingly storing unencrypted credentials on disk. 
  - These settings can be configured in `[registry]` and `[registries]` tables.
- 🎉 `--keep-going` flag has been stabilized and is now available in each build command
  (except `bench` and `test`, which have `--no-fail-fast` instead).
  ([docs](https://doc.rust-lang.org/cargo/commands/cargo-build.html#option-cargo-build---keep-going))
  [#12568](https://github.com/rust-lang/cargo/pull/12568)
- Added `--dry-run` flag and summary line at the end for `cargo clean`.
  [#12638](https://github.com/rust-lang/cargo/pull/12638)
- Added a short alias `-n` for cli option `--dry-run`. 
  [#12660](https://github.com/rust-lang/cargo/pull/12660)
- Added support for `target.'cfg(..)'.linker`.
  [#12535](https://github.com/rust-lang/cargo/pull/12535)
- Allowed incomplete versions when they are unambiguous for flags like `--package`.
  [#12591](https://github.com/rust-lang/cargo/pull/12591)
  [#12614](https://github.com/rust-lang/cargo/pull/12614)
  [#12806](https://github.com/rust-lang/cargo/pull/12806)

### Changed

- ❗️ Changed how arrays in configuration are merged.
  The order was unspecified and now follows how other configuration types work for consistency.
  [summary](https://blog.rust-lang.org/inside-rust/2023/08/24/cargo-config-merging.html)
  [#12515](https://github.com/rust-lang/cargo/pull/12515)
- ❗️ cargo-clean: error out if `--doc` is mixed with `-p`.
  [#12637](https://github.com/rust-lang/cargo/pull/12637)
- ❗ cargo-new / cargo-init no longer exclude `Cargo.lock` in VCS ignore files for libraries.
  [#12382](https://github.com/rust-lang/cargo/pull/12382)
- cargo-update: silently deprecate `--aggressive` in favor of the new `--recursive`.
  [#12544](https://github.com/rust-lang/cargo/pull/12544)
- cargo-update: `-p/--package` can be used as a positional argument.
  [#12545](https://github.com/rust-lang/cargo/pull/12545)
  [#12586](https://github.com/rust-lang/cargo/pull/12586)
- cargo-install: suggest `--git` when the package name looks like a URL.
  [#12575](https://github.com/rust-lang/cargo/pull/12575)
- cargo-add: summarize the feature list when it's too long.
  [#12662](https://github.com/rust-lang/cargo/pull/12662)
  [#12702](https://github.com/rust-lang/cargo/pull/12702)
- Shell completion for `--target` uses rustup but falls back to rustc.
  [#12606](https://github.com/rust-lang/cargo/pull/12606)
- Help users know possible `--target` values.
  [#12607](https://github.com/rust-lang/cargo/pull/12607)
- Enhanced "registry index not found" error message.
  [#12732](https://github.com/rust-lang/cargo/pull/12732)
- Enhanced CLI help message of `--explain`. 
  [#12592](https://github.com/rust-lang/cargo/pull/12592)
- Enhanced deserialization errors of untagged enums with `serde-untagged`.
  [#12574](https://github.com/rust-lang/cargo/pull/12574)
  [#12581](https://github.com/rust-lang/cargo/pull/12581)
- Enhanced the error when mismatching prerelease version candidates.
  [#12659](https://github.com/rust-lang/cargo/pull/12659)
- Enhanced the suggestion on ambiguous Package ID spec.
  [#12685](https://github.com/rust-lang/cargo/pull/12685)
- Enhanced TOML parse errors to show the context.
  [#12556](https://github.com/rust-lang/cargo/pull/12556)
- Enhanced filesystem error by adding wrappers around `std::fs::metadata`.
  [#12636](https://github.com/rust-lang/cargo/pull/12636)
- Enhanced resolver version mismatch warning.
  [#12573](https://github.com/rust-lang/cargo/pull/12573)
- Use clap to suggest alternative argument for unsupported arguments.
  [#12529](https://github.com/rust-lang/cargo/pull/12529)
  [#12693](https://github.com/rust-lang/cargo/pull/12693)
  [#12723](https://github.com/rust-lang/cargo/pull/12723)
- Removed redundant information from cargo new/init `--help` output.
  [#12594](https://github.com/rust-lang/cargo/pull/12594)
- Console output and styling tweaks.
  [#12578](https://github.com/rust-lang/cargo/pull/12578)
  [#12655](https://github.com/rust-lang/cargo/pull/12655)
  [#12593](https://github.com/rust-lang/cargo/pull/12593)

### Fixed

- Use full target spec for `cargo rustc --print --target`.
  [#12743](https://github.com/rust-lang/cargo/pull/12743)
- Copy PDBs also for EFI targets.
  [#12688](https://github.com/rust-lang/cargo/pull/12688)
- Fixed resolver behavior being independent of package order.
  [#12602](https://github.com/rust-lang/cargo/pull/12602)
- Fixed unnecessary clean up of `profile.release.package."*"` for `cargo remove`.
  [#12624](https://github.com/rust-lang/cargo/pull/12624)

### Nightly only

- `-Zasymmetric-token`: Created dedicated unstable flag for asymmetric-token support.
  [#12551](https://github.com/rust-lang/cargo/pull/12551)
- `-Zasymmetric-token`: Improved logout message for asymmetric tokens.
  [#12587](https://github.com/rust-lang/cargo/pull/12587)
- `-Zmsrv-policy`: **Very** preliminary MSRV resolver support.
  [#12560](https://github.com/rust-lang/cargo/pull/12560)
- `-Zscript`: Hack in code fence support.
  [#12681](https://github.com/rust-lang/cargo/pull/12681)
- `-Zbindeps`: Support dependencies from registries.
  [#12421](https://github.com/rust-lang/cargo/pull/12421)

### Documentation

- ❗ Policy change: Checking `Cargo.lock` into version control is now the default choice,
  even for libraries. Lockfile and CI integration documentations are also expanded.
  [Policy docs](https://doc.rust-lang.org/nightly/cargo/faq.html#why-have-cargolock-in-version-control),
  [Lockfile docs](https://doc.rust-lang.org/nightly/cargo/guide/cargo-toml-vs-cargo-lock.html),
  [CI docs](https://doc.rust-lang.org/nightly/cargo/guide/continuous-integration.html),
  [#12382](https://github.com/rust-lang/cargo/pull/12382)
  [#12630](https://github.com/rust-lang/cargo/pull/12630)
- SemVer: Update documentation about removing optional dependencies.
  [#12687](https://github.com/rust-lang/cargo/pull/12687)
- Contrib: Add process for security responses.
  [#12487](https://github.com/rust-lang/cargo/pull/12487)
- cargo-publish: warn about upload timeout.
  [#12733](https://github.com/rust-lang/cargo/pull/12733)
- mdbook: use *AND* search when having multiple terms.
  [#12548](https://github.com/rust-lang/cargo/pull/12548)
- Established publish best practices 
  [#12745](https://github.com/rust-lang/cargo/pull/12745)
- Clarify caret requirements.
  [#12679](https://github.com/rust-lang/cargo/pull/12679)
- Clarify how `version` works for `git` dependencies.
  [#12270](https://github.com/rust-lang/cargo/pull/12270)
- Clarify and differentiate defaults for split-debuginfo.
  [#12680](https://github.com/rust-lang/cargo/pull/12680)
- Added missing `strip` entries in `dev` and `release` profiles.
  [#12748](https://github.com/rust-lang/cargo/pull/12748)

### Internal

- Updated to `curl-sys` 0.4.66, which corresponds to curl 8.3.0.
  [#12718](https://github.com/rust-lang/cargo/pull/12718)
- Updated to `gitoxide` 0.54.1.
  [#12731](https://github.com/rust-lang/cargo/pull/12731)
- Updated to `git2` 0.18.0, which corresponds to libgit2 1.7.1.
  [#12580](https://github.com/rust-lang/cargo/pull/12580)
- Updated to `cargo_metadata` 0.17.0.
  [#12758](https://github.com/rust-lang/cargo/pull/12610)
- Updated target-arch-aware crates to support mips r6 targets 
  [#12720](https://github.com/rust-lang/cargo/pull/12720)
- publish.py: Remove obsolete `sleep()` calls.
  [#12686](https://github.com/rust-lang/cargo/pull/12686)
- Define `{{command}}` for use in src/doc/man/includes 
  [#12570](https://github.com/rust-lang/cargo/pull/12570)
- Set tracing target `network` for networking messages.
  [#12582](https://github.com/rust-lang/cargo/pull/12582)
- cargo-test-support: Add `with_stdout_unordered`.
  [#12635](https://github.com/rust-lang/cargo/pull/12635)
- dep: Switch from `termcolor` to `anstream`.
  [#12751](https://github.com/rust-lang/cargo/pull/12751)
- Put `Source` trait under `cargo::sources`.
  [#12527](https://github.com/rust-lang/cargo/pull/12527)
- SourceId: merge `name` and `alt_registry_key` into one enum.
  [#12675](https://github.com/rust-lang/cargo/pull/12675)
- TomlManifest: fail when package_root is not a directory.
  [#12722](https://github.com/rust-lang/cargo/pull/12722)
- util: enhanced doc of `network::retry` doc.
  [#12583](https://github.com/rust-lang/cargo/pull/12583)
- refactor: Pull out cargo-add MSRV code for reuse 
  [#12553](https://github.com/rust-lang/cargo/pull/12553)
- refactor(install): Move value parsing to clap 
  [#12547](https://github.com/rust-lang/cargo/pull/12547)
- Fixed spurious errors with networking tests. 
  [#12726](https://github.com/rust-lang/cargo/pull/12726)
- Use a more compact relative-time format for `CARGO_LOG` internal logging.
  [#12542](https://github.com/rust-lang/cargo/pull/12542)
- Use newer std API for cleaner code.
  [#12559](https://github.com/rust-lang/cargo/pull/12559)
  [#12604](https://github.com/rust-lang/cargo/pull/12604)
  [#12615](https://github.com/rust-lang/cargo/pull/12615)
  [#12631](https://github.com/rust-lang/cargo/pull/12631)
- Buffer console status messages. 
  [#12727](https://github.com/rust-lang/cargo/pull/12727)
- Use enum to describe index summaries to provide a richer information when summaries are not available for resolution.
  [#12643](https://github.com/rust-lang/cargo/pull/12643)
- Use shortest path for resolving the path from the given dependency up to the root.
  [#12678](https://github.com/rust-lang/cargo/pull/12678)
- Read/write the encoded `cargo update --precise` in the same place 
  [#12629](https://github.com/rust-lang/cargo/pull/12629)
- Set MSRV for internal packages.
  [#12381](https://github.com/rust-lang/cargo/pull/12381)
- ci: Update Renovate schema 
  [#12741](https://github.com/rust-lang/cargo/pull/12741)
- ci: Ignore patch version in MSRV 
  [#12716](https://github.com/rust-lang/cargo/pull/12716)

## Cargo 1.73 (2023-10-05)
[45782b6b...rust-1.73.0](https://github.com/rust-lang/cargo/compare/45782b6b...rust-1.73.0)

### Added

- Print environment variables for `cargo run/bench/test` in extra verbose mode `-vv`.
  [#12498](https://github.com/rust-lang/cargo/pull/12498)
- Display package versions on Cargo timings graph.
  [#12420](https://github.com/rust-lang/cargo/pull/12420)

### Changed

- ❗️ Cargo now bails out when using `cargo::` in custom build scripts. This is
  a preparation for an upcoming change in build script invocations.
  [#12332](https://github.com/rust-lang/cargo/pull/12332)
- ❗️ `cargo login` no longer accept any token after the `--` syntax.
  Arguments after `--` are now reserved in the preparation of the new credential provider feature.
  This introduces a regression that overlooks the `cargo login -- <token>` support in previous versions.
  [#12499](https://github.com/rust-lang/cargo/pull/12499)
- Make Cargo `--help` easier to browse.
  [#11905](https://github.com/rust-lang/cargo/pull/11905)
- Prompt the use of `--nocapture` flag if `cargo test` process is terminated via a signal.
  [#12463](https://github.com/rust-lang/cargo/pull/12463)
- Preserve jobserver file descriptors on the rustc invocation for getting target information.
  [#12447](https://github.com/rust-lang/cargo/pull/12447)
- Clarify in `--help` that `cargo test --all-targets` excludes doctests.
  [#12422](https://github.com/rust-lang/cargo/pull/12422)
- Normalize `cargo.toml` to `Cargo.toml` on publish, and warn on other cases of `Cargo.toml`.
  [#12399](https://github.com/rust-lang/cargo/pull/12399)

### Fixed

- Only skip mtime check on `~/.cargo/{git,registry}`.
  [#12369](https://github.com/rust-lang/cargo/pull/12369)
- Fixed `cargo doc --open` crash on WSL2.
  [#12373](https://github.com/rust-lang/cargo/pull/12373)
- Fixed panic when enabling `http.debug` for certain strings.
  [#12468](https://github.com/rust-lang/cargo/pull/12468)
- Fixed `cargo remove` incorrectly removing used patches.
  [#12454](https://github.com/rust-lang/cargo/pull/12454)
- Fixed crate checksum lookup query should match on semver build metadata.
  [#11447](https://github.com/rust-lang/cargo/pull/11447)
- Fixed printing multiple warning messages for unused fields in `[registries]` table.
  [#12439](https://github.com/rust-lang/cargo/pull/12439)

### Nightly only

- 🔥 The `-Zcredential-process` has been reimplemented with a clearer way to
  communicate  with different credential providers. Several built-in providers
  are also added to Cargo.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#credential-process)
  [#12334](https://github.com/rust-lang/cargo/pull/12334)
  [#12396](https://github.com/rust-lang/cargo/pull/12396)
  [#12424](https://github.com/rust-lang/cargo/pull/12424)
  [#12440](https://github.com/rust-lang/cargo/pull/12440)
  [#12461](https://github.com/rust-lang/cargo/pull/12461)
  [#12469](https://github.com/rust-lang/cargo/pull/12469)
  [#12483](https://github.com/rust-lang/cargo/pull/12483)
  [#12499](https://github.com/rust-lang/cargo/pull/12499)
  [#12507](https://github.com/rust-lang/cargo/pull/12507)
  [#12512](https://github.com/rust-lang/cargo/pull/12512)
  [#12518](https://github.com/rust-lang/cargo/pull/12518)
  [#12521](https://github.com/rust-lang/cargo/pull/12521)
  [#12526](https://github.com/rust-lang/cargo/pull/12526)  
  Some notable changes:
  - Renamed `credential-process` to `credential-provider` in Cargo configurations.
  - New JSON protocol for communicating with external credential providers via stdin/stdout.
  - The GNOME Secert provider now dynamically loads `libsecert`.
  - The 1password provider is no longer built-in.
  - Changed the unstable key for asymmetric tokens from `registry-auth` to `credential-process`.
- ❗️ Removed `--keep-going` flag support from `cargo test` and `cargo bench`.
  [#12478](https://github.com/rust-lang/cargo/pull/12478)
  [#12492](https://github.com/rust-lang/cargo/pull/12492)
- Fixed invalid package names generated by `-Zscript`.
  [#12349](https://github.com/rust-lang/cargo/pull/12349)
- `-Zscript` now errors out on unsupported commands — `publish` and `package`.
  [#12350](https://github.com/rust-lang/cargo/pull/12350)
- Encode URL params correctly for source ID in Cargo.lock.
  [#12280](https://github.com/rust-lang/cargo/pull/12280)
- Replaced invalid `panic_unwind` std feature with `panic-unwind`.
  [#12364](https://github.com/rust-lang/cargo/pull/12364)
- `-Zlints`: doctest extraction should respect `[lints]`.
  [#12501](https://github.com/rust-lang/cargo/pull/12501)

### Documentation

- SemVer: Adding a section for changing the alignment, layout, or size of a
  well-defined type.
  [#12169](https://github.com/rust-lang/cargo/pull/12169)
- Use heading attributes to control the fragment.
  [#12339](https://github.com/rust-lang/cargo/pull/12339)
- Use "number" instead of "digit" when explaining Cargo's use of semver.
  [#12340](https://github.com/rust-lang/cargo/pull/12340)
- contrib: Add some more detail about how publishing works.
  [#12344](https://github.com/rust-lang/cargo/pull/12344)
- Clarify "Package ID" and "Source ID" in `cargo metadata` are opaque strings.
  [#12313](https://github.com/rust-lang/cargo/pull/12313)
- Clarify that `rerun-if-env-changed` doesn't monitor the environment variables
  it set for crates and build script.
  [#12482](https://github.com/rust-lang/cargo/pull/12482)
- Clarify that multiple versions that differ only in the metadata tag are
  disallowed on crates.io.
  [#12335](https://github.com/rust-lang/cargo/pull/12335)
- Clarify `lto` setting passing `-Clinker-plugin-lto`.
  [#12407](https://github.com/rust-lang/cargo/pull/12407)
- Added `profile.strip` to configuration and environment variable docs.
  [#12337](https://github.com/rust-lang/cargo/pull/12337)
  [#12408](https://github.com/rust-lang/cargo/pull/12408)
- Added docs for artifact JSON debuginfo levels.
  [#12376](https://github.com/rust-lang/cargo/pull/12376)
- Added a notice for the backward compatible `.cargo/credential` file existence.
  [#12479](https://github.com/rust-lang/cargo/pull/12479)
- Raised the awareness of `resolver = 2` used inside workspaces.
  [#12388](https://github.com/rust-lang/cargo/pull/12388)
- Replaced `master` branch by default branch in documentation.
  [#12435](https://github.com/rust-lang/cargo/pull/12435)

### Internal

- Updated to `criterion` 0.5.1.
  [#12338](https://github.com/rust-lang/cargo/pull/12338)
- Updated to `curl-sys` 0.4.65, which corresponds to curl 8.2.1.
  [#12406](https://github.com/rust-lang/cargo/pull/12406)
- Updated to `indexmap` v2.
  [#12368](https://github.com/rust-lang/cargo/pull/12368)
- Updated to `miow` 0.6.0, which drops old versions of `windows-sys`.
  [#12453](https://github.com/rust-lang/cargo/pull/12453)
- ci: automatically test new packages by using `--workspace`.
  [#12342](https://github.com/rust-lang/cargo/pull/12342)
- ci: automatically update dependencies monthly with Renovate.
  [#12341](https://github.com/rust-lang/cargo/pull/12341)
  [#12466](https://github.com/rust-lang/cargo/pull/12466)
- ci: rewrote `xtask-bump-check` for respecting semver by adopting `cargo-semver-checks`.
  [#12395](https://github.com/rust-lang/cargo/pull/12395)
  [#12513](https://github.com/rust-lang/cargo/pull/12513)
  [#12508](https://github.com/rust-lang/cargo/pull/12508)
- Rearranged and renamed test directories
  [#12397](https://github.com/rust-lang/cargo/pull/12397)
  [#12398](https://github.com/rust-lang/cargo/pull/12398)
- Migrated from `log` to `tracing`.
  [#12458](https://github.com/rust-lang/cargo/pull/12458)
  [#12488](https://github.com/rust-lang/cargo/pull/12488)
- Track `--help` output in tests.
  [#11912](https://github.com/rust-lang/cargo/pull/11912)
- Cleaned up and shared package metadata within workspace.
  [#12352](https://github.com/rust-lang/cargo/pull/12352)
- `crates-io`: expose HTTP headers and `Error` type.
  [#12310](https://github.com/rust-lang/cargo/pull/12310)
- For `cargo update`, caught CLI flags conflict between `--aggressive` and `--precise` in clap.
  [#12428](https://github.com/rust-lang/cargo/pull/12428)
- Several fixes for either making Cargo testsuite pass on nightly or in `rust-lang/rust`.
  [#12413](https://github.com/rust-lang/cargo/pull/12413)
  [#12416](https://github.com/rust-lang/cargo/pull/12416)
  [#12429](https://github.com/rust-lang/cargo/pull/12429)
  [#12450](https://github.com/rust-lang/cargo/pull/12450)
  [#12491](https://github.com/rust-lang/cargo/pull/12491)
  [#12500](https://github.com/rust-lang/cargo/pull/12500)

## Cargo 1.72 (2023-08-24)
[64fb38c9...rust-1.72.0](https://github.com/rust-lang/cargo/compare/64fb38c9...rust-1.72.0)

### Added

- ❗ Enable `-Zdoctest-in-workspace` by default. When running each documentation
  test, the working directory is set to the root directory of the package the
  test belongs to.
  [docs](https://doc.rust-lang.org/nightly/cargo/commands/cargo-test.html#working-directory-of-tests)
  [#12221](https://github.com/rust-lang/cargo/pull/12221)
  [#12288](https://github.com/rust-lang/cargo/pull/12288)
- Add support of the "default" keyword to reset previously set `build.jobs`
  parallelism back to the default.
  [#12222](https://github.com/rust-lang/cargo/pull/12222)

### Changed

- 🚨 [CVE-2023-40030](https://github.com/rust-lang/cargo/security/advisories/GHSA-wrrj-h57r-vx9p):
  Malicious dependencies can inject arbitrary JavaScript into cargo-generated timing reports.
  To mitigate this, feature name validation check is now turned into a hard error.
  The warning was added in Rust 1.49. These extended characters aren't allowed on crates.io,
  so this should only impact users of other registries, or people who don't publish to a registry.
  [#12291](https://github.com/rust-lang/cargo/pull/12291)
- Cargo now warns when an edition 2021 package is in a virtual workspace and
  `workspace.resolver` is not set. It is recommended to set the resolver
  version for workspaces explicitly.
  [#10910](https://github.com/rust-lang/cargo/pull/10910)
- Set IBM AIX shared libraries search path to `LIBPATH`.
  [#11968](https://github.com/rust-lang/cargo/pull/11968)
- Don't pass `-C debuginfo=0` to rustc as it is the default value.
  [#12022](https://github.com/rust-lang/cargo/pull/12022)
  [#12205](https://github.com/rust-lang/cargo/pull/12205)
- Added a message on reusing previous temporary path on `cargo install` failures.
  [#12231](https://github.com/rust-lang/cargo/pull/12231)
- Added a message when `rustup` override shorthand is put in a wrong position.
  [#12226](https://github.com/rust-lang/cargo/pull/12226)
- Respect scp-like URL as much as possible when fetching nested submodules.
  [#12359](https://github.com/rust-lang/cargo/pull/12359)
  [#12411](https://github.com/rust-lang/cargo/pull/12411)

### Fixed

- `cargo clean` uses `remove_dir_all` as a fallback to resolve race conditions.
  [#11442](https://github.com/rust-lang/cargo/pull/11442)
- Reduced the chance Cargo re-formats the user's `[features]` table.
  [#12191](https://github.com/rust-lang/cargo/pull/12191)
- Fixed nested Git submodules not able to fetch.
  [#12244](https://github.com/rust-lang/cargo/pull/12244)

### Nightly only

- 🔥 The `-Zscript` is an experimental feature to add unstable support for
  single-file packages in Cargo, so we can explore the design and resolve
  questions with an implementation to collect feedback on.
  ([eRFC 3424](https://github.com/rust-lang/rfcs/blob/master/text/3424-cargo-script.md))
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#script)
  [#12245](https://github.com/rust-lang/cargo/pull/12245)
  [#12255](https://github.com/rust-lang/cargo/pull/12255)
  [#12258](https://github.com/rust-lang/cargo/pull/12258)
  [#12262](https://github.com/rust-lang/cargo/pull/12262)
  [#12268](https://github.com/rust-lang/cargo/pull/12268)
  [#12269](https://github.com/rust-lang/cargo/pull/12269)
  [#12281](https://github.com/rust-lang/cargo/pull/12281)
  [#12282](https://github.com/rust-lang/cargo/pull/12282)
  [#12283](https://github.com/rust-lang/cargo/pull/12283)
  [#12284](https://github.com/rust-lang/cargo/pull/12284)
  [#12287](https://github.com/rust-lang/cargo/pull/12287)
  [#12289](https://github.com/rust-lang/cargo/pull/12289)
  [#12303](https://github.com/rust-lang/cargo/pull/12303)
  [#12305](https://github.com/rust-lang/cargo/pull/12305)
  [#12308](https://github.com/rust-lang/cargo/pull/12308)
- Automatically inherit workspace lints when running `cargo new`/`cargo init`.
  [#12174](https://github.com/rust-lang/cargo/pull/12174)
- Removed `-Zjobserver-per-rustc` again.
  [#12285](https://github.com/rust-lang/cargo/pull/12285)
- Added `.toml` file extension restriction for `-Zconfig-include`.
  [#12298](https://github.com/rust-lang/cargo/pull/12298)
- Added `-Znext-lockfile-bump` to prepare for the next lockfile bump.
  [#12279](https://github.com/rust-lang/cargo/pull/12279)
  [#12302](https://github.com/rust-lang/cargo/pull/12302)

### Documentation

- Added a description of `Cargo.lock` conflicts in the Cargo FAQ.
  [#12185](https://github.com/rust-lang/cargo/pull/12185)
- Added a small note about indexes ignoring SemVer build metadata.
  [#12206](https://github.com/rust-lang/cargo/pull/12206)
- Added doc comments for types and friends in `cargo::sources` module.
  [#12192](https://github.com/rust-lang/cargo/pull/12192)
  [#12239](https://github.com/rust-lang/cargo/pull/12239)
  [#12247](https://github.com/rust-lang/cargo/pull/12247)
- Added more documentation for `Source` download functions.
  [#12319](https://github.com/rust-lang/cargo/pull/12319)
- Added READMEs for the credential helpers.
  [#12322](https://github.com/rust-lang/cargo/pull/12322)
- Fixed version requirement example in Dependency Resolution.
  [#12267](https://github.com/rust-lang/cargo/pull/12267)
- Clarify the default behavior of cargo-install.
  [#12276](https://github.com/rust-lang/cargo/pull/12276)
- Clarify the use of "default" branch instead of `main` by default.
  [#12251](https://github.com/rust-lang/cargo/pull/12251)
- Provide guidance on version requirements.
  [#12323](https://github.com/rust-lang/cargo/pull/12323)

### Internal

- Updated to `gix` 0.45 for multi-round pack negotiations.
  [#12236](https://github.com/rust-lang/cargo/pull/12236)
- Updated to `curl-sys` 0.4.63, which corresponds to curl 8.1.2.
  [#12218](https://github.com/rust-lang/cargo/pull/12218)
- Updated to `openssl` 0.10.55.
  [#12300](https://github.com/rust-lang/cargo/pull/12300)
- Updated several dependencies.
  [#12261](https://github.com/rust-lang/cargo/pull/12261)
- Removed unused features from `windows-sys` dependency.
  [#12176](https://github.com/rust-lang/cargo/pull/12176)
- Refactored compiler invocations.
  [#12211](https://github.com/rust-lang/cargo/pull/12211)
- Refactored git and registry sources, and registry data.
  [#12203](https://github.com/rust-lang/cargo/pull/12203)
  [#12197](https://github.com/rust-lang/cargo/pull/12197)
  [#12240](https://github.com/rust-lang/cargo/pull/12240)
  [#12248](https://github.com/rust-lang/cargo/pull/12248)
- Lexicographically order `-Z` flags.
  [#12182](https://github.com/rust-lang/cargo/pull/12182)
  [#12223](https://github.com/rust-lang/cargo/pull/12223)
  [#12224](https://github.com/rust-lang/cargo/pull/12224)
- Several Cargo's own test infra improvements and speed-ups.
  [#12184](https://github.com/rust-lang/cargo/pull/12184)
  [#12188](https://github.com/rust-lang/cargo/pull/12188)
  [#12189](https://github.com/rust-lang/cargo/pull/12189)
  [#12194](https://github.com/rust-lang/cargo/pull/12194)
  [#12199](https://github.com/rust-lang/cargo/pull/12199)
- Migrated print-ban from test to clippy
  [#12246](https://github.com/rust-lang/cargo/pull/12246)
- Switched to `OnceLock` for interning uses.
  [#12217](https://github.com/rust-lang/cargo/pull/12217)
- Removed a unnecessary `.clone`.
  [#12213](https://github.com/rust-lang/cargo/pull/12213)
- Don't try to compile `cargo-credential-gnome-secret` on non-Linux platforms.
  [#12321](https://github.com/rust-lang/cargo/pull/12321)
- Use macro to remove duplication of workspace inheritable fields getters.
  [#12317](https://github.com/rust-lang/cargo/pull/12317)
- Extracted and rearranged registry API items to their own modules.
  [#12290](https://github.com/rust-lang/cargo/pull/12290)
- Show a better error when container tests fail.
  [#12264](https://github.com/rust-lang/cargo/pull/12264)

## Cargo 1.71.1 (2023-08-03)

### Fixed

- 🚨 [CVE-2023-38497](https://github.com/rust-lang/cargo/security/advisories/GHSA-j3xp-wfr4-hx87):
  Cargo 1.71.1 or later respects umask when extracting crate archives. It also
  purges the caches it tries to access if they were generated by older Cargo versions.

## Cargo 1.71 (2023-07-13)
[84b7041f...rust-1.71.0](https://github.com/rust-lang/cargo/compare/84b7041f...rust-1.71.0)

### Added

- Allowed named debuginfo options in Cargo.toml.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#debug)
  [#11958](https://github.com/rust-lang/cargo/pull/11958)
- Added `workspace_default_members` to the output of `cargo metadata`.
  [#11978](https://github.com/rust-lang/cargo/pull/11978)
- Automatically inherit workspace fields when running `cargo new`/`cargo init`.
  [#12069](https://github.com/rust-lang/cargo/pull/12069)

### Changed

- ❗ Optimized the usage under `rustup`. When Cargo detects it will run `rustc`
  pointing a rustup proxy, it'll try bypassing the proxy and use the underlying
  binary directly. There are assumptions around the interaction with rustup and
  `RUSTUP_TOOLCHAIN`. However, it's not expected to affect normal users.
  [#11917](https://github.com/rust-lang/cargo/pull/11917)
- ❗ When querying a package, Cargo tries only the original name, all hyphens,
  and all underscores to handle misspellings. Previously, Cargo tried each
  combination of hyphens and underscores, causing excessive requests to crates.io.
  [#12083](https://github.com/rust-lang/cargo/pull/12083)
- ❗ Disallow `RUSTUP_HOME` and `RUSTUP_TOOLCHAIN` in the `[env]` configuration
  table. This is considered to be not a use case Cargo would like to support,
  since it will likely cause problems or lead to confusion.
  [#12101](https://github.com/rust-lang/cargo/pull/12101)
  [#12107](https://github.com/rust-lang/cargo/pull/12107)
- Better error message when getting an empty dependency table in Cargo.toml.
  [#11997](https://github.com/rust-lang/cargo/pull/11997)
- Better error message when empty dependency was specified in Cargo.toml.
  [#12001](https://github.com/rust-lang/cargo/pull/12001)
- `--help` text is now wrapping for readability on narrow screens.
  [#12013](https://github.com/rust-lang/cargo/pull/12013)
- Tweaked the order of arguments in `--help` text to clarify role of `--bin`.
  [#12157](https://github.com/rust-lang/cargo/pull/12157)
- `rust-version` is included in `cargo publish` requests to registries.
  [#12041](https://github.com/rust-lang/cargo/pull/12041)

### Fixed

- Corrected the bug report URL for `cargo clippy --fix`.
  [#11882](https://github.com/rust-lang/cargo/pull/11882)
- Cargo now applies `[env]` to rust invocations for target info discovery.
  [#12029](https://github.com/rust-lang/cargo/pull/12029)
- Fixed tokens not redacted in http debug when using HTTP/2.
  [#12095](https://github.com/rust-lang/cargo/pull/12095)
- Fixed `-C debuginfo` not passed in some situation, leading to build cache miss.
  [#12165](https://github.com/rust-lang/cargo/pull/12165)
- Fixed the ambiguity when `cargo install` found packages with the same name.
  The ambiguity happened in a situation like a package depending on old versions
  of itself.
  [#12015](https://github.com/rust-lang/cargo/pull/12015)
- Fixed a false positive that `cargo package` checks for conflict files.
  [#12135](https://github.com/rust-lang/cargo/pull/12135)
- Fixed `dep/feat` syntax not working when co-exist with `dep:` syntax, and
  trying to enable features of an optional dependency.
  [#12130](https://github.com/rust-lang/cargo/pull/12130)
- Fixed `cargo tree` not handling the output with `-e no-proc-macro` correctly.
  [#12044](https://github.com/rust-lang/cargo/pull/12044)
- Warn instead of error in `cargo package` on empty `readme` or `license-file`
  in Cargo.toml.
  [#12036](https://github.com/rust-lang/cargo/pull/12036)
- Fixed when an HTTP proxy is in use and the Cargo executable links to a
  certain version of system libcurl, CURL connections might fail. Affected
  libcurl versions: 7.87.0, 7.88.0, 7.88.1.
  [#12234](https://github.com/rust-lang/cargo/pull/12234)
  [#12242](https://github.com/rust-lang/cargo/pull/12242)

### Nightly only

- 🔥 The `-Zgitoxide` feature now supports shallow clones and fetches for
  dependencies and registry indexes.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#gitoxide)
  [#11840](https://github.com/rust-lang/cargo/pull/11840)
- 🔥 The `-Zlints` feature enables configuring lints rules in Cargo.toml
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#lints)
  [#12148](https://github.com/rust-lang/cargo/pull/12148)
  [#12168](https://github.com/rust-lang/cargo/pull/12168)
- The `-Zbuild-std` breakage of missing features in `nightly-2023-05-04` has
  been fixed in `nightly-2023-05-05`.
  [#12088](https://github.com/rust-lang/cargo/pull/12088)
- Recompile on profile rustflags changes.
  [#11981](https://github.com/rust-lang/cargo/pull/11981)
- Added `-Zmsrv-policy` feature flag placeholder.
  [#12043](https://github.com/rust-lang/cargo/pull/12043)
- `cargo add` now considers `rust-version` when selecting packages with `-Zmsrv-policy`.
  [#12078](https://github.com/rust-lang/cargo/pull/12078)

### Documentation

- Added Cargo team charter.
  [docs](https://doc.crates.io/contrib/team.html)
  [#12010](https://github.com/rust-lang/cargo/pull/12010)
- SemVer: Adding `#[non_exhaustive]` on existing items is a breaking change.
  [#10877](https://github.com/rust-lang/cargo/pull/10877)
- SemVer: It is not a breaking change to make an unsafe function safe.
  [#12116](https://github.com/rust-lang/cargo/pull/12116)
- SemVer: changing MSRV is generally a minor change.
  [#12122](https://github.com/rust-lang/cargo/pull/12122)
- Clarify when and how to `cargo yank`.
  [#11862](https://github.com/rust-lang/cargo/pull/11862)
- Clarify that crates.io doesn't link to docs.rs right away.
  [#12146](https://github.com/rust-lang/cargo/pull/12146)
- Clarify documentation around test target setting. 
  [#12032](https://github.com/rust-lang/cargo/pull/12032)
- Specify `rust_version` in Index format.
  [#12040](https://github.com/rust-lang/cargo/pull/12040)
- Specify `msg` in owner-remove registry API response.
  [#12068](https://github.com/rust-lang/cargo/pull/12068)
- Added more documentation for artifact-dependencies. 
  [#12110](https://github.com/rust-lang/cargo/pull/12110)
- Added doc comments for `Source` and build script for cargo-the-library.
  [#12133](https://github.com/rust-lang/cargo/pull/12133)
  [#12153](https://github.com/rust-lang/cargo/pull/12153)
  [#12159](https://github.com/rust-lang/cargo/pull/12159)
- Several typo and broken link fixes.
  [#12018](https://github.com/rust-lang/cargo/pull/12018)
  [#12020](https://github.com/rust-lang/cargo/pull/12020)
  [#12049](https://github.com/rust-lang/cargo/pull/12049)
  [#12067](https://github.com/rust-lang/cargo/pull/12067)
  [#12073](https://github.com/rust-lang/cargo/pull/12073)
  [#12143](https://github.com/rust-lang/cargo/pull/12143)
- home: clarify the behavior on each platform
  [#12047](https://github.com/rust-lang/cargo/pull/12047)

### Internal

- Updated to `linux-raw-sys` 0.3.2 
  [#11998](https://github.com/rust-lang/cargo/pull/11998)
- Updated to `git2` 0.17.1, which corresponds to libgit2 1.6.4.
  [#12096](https://github.com/rust-lang/cargo/pull/12096)
- Updated to `windows-sys` 0.48.0 
  [#12021](https://github.com/rust-lang/cargo/pull/12021)
- Updated to `libc` 0.2.144 
  [#12014](https://github.com/rust-lang/cargo/pull/12014)
  [#12098](https://github.com/rust-lang/cargo/pull/12098)
- Updated to `openssl-src` 111.25.3+1.1.1t 
  [#12005](https://github.com/rust-lang/cargo/pull/12005)
- Updated to `home` 0.5.5
  [#12037](https://github.com/rust-lang/cargo/pull/12037)
- Enabled feature `Win32_System_Console` feature since it is used.
  [#12016](https://github.com/rust-lang/cargo/pull/12016)
- Cargo is now a Cargo workspace. We dogfood ourselves finally!
  [#11851](https://github.com/rust-lang/cargo/pull/11851)
  [#11994](https://github.com/rust-lang/cargo/pull/11994)
  [#11996](https://github.com/rust-lang/cargo/pull/11996)
  [#12024](https://github.com/rust-lang/cargo/pull/12024)
  [#12025](https://github.com/rust-lang/cargo/pull/12025)
  [#12057](https://github.com/rust-lang/cargo/pull/12057)
- 🔥 A new, straightforward issue labels system for Cargo contributors.
  [docs](https://doc.crates.io/contrib/issues.html)
  [#11995](https://github.com/rust-lang/cargo/pull/11995)
  [#12002](https://github.com/rust-lang/cargo/pull/12002)
  [#12003](https://github.com/rust-lang/cargo/pull/12003)
- Allow win/mac credential managers to build on all platforms.
  [#11993](https://github.com/rust-lang/cargo/pull/11993)
  [#12027](https://github.com/rust-lang/cargo/pull/12027)
- Use `openssl` only on non-Windows platforms.
  [#11979](https://github.com/rust-lang/cargo/pull/11979)
- Use restricted Damerau-Levenshtein algorithm to provide typo suggestions.
  [#11963](https://github.com/rust-lang/cargo/pull/11963)
- Added a new xtask `cargo build-man`.
  [#12048](https://github.com/rust-lang/cargo/pull/12048)
- Added a new xtask `cargo stale-label`.
  [#12051](https://github.com/rust-lang/cargo/pull/12051)
- Added a new xtask `cargo unpublished`.
  [#12039](https://github.com/rust-lang/cargo/pull/12039)
  [#12045](https://github.com/rust-lang/cargo/pull/12045)
  [#12085](https://github.com/rust-lang/cargo/pull/12085)
- CI: check if any version bump needed for member crates.
  [#12126](https://github.com/rust-lang/cargo/pull/12126)
- Fixed some test infra issues.
  [#11976](https://github.com/rust-lang/cargo/pull/11976)
  [#12026](https://github.com/rust-lang/cargo/pull/12026)
  [#12055](https://github.com/rust-lang/cargo/pull/12055)
  [#12117](https://github.com/rust-lang/cargo/pull/12117)

## Cargo 1.70 (2023-06-01)
[9880b408...rust-1.70.0](https://github.com/rust-lang/cargo/compare/9880b408...rust-1.70.0)

### Added

- 🎉 Added `cargo logout` command for removing an API token from the registry locally.
  [docs](https://doc.rust-lang.org/nightly/cargo/commands/cargo-logout.html)
  [#11919](https://github.com/rust-lang/cargo/pull/11919)
  [#11950](https://github.com/rust-lang/cargo/pull/11950)
- Added `--ignore-rust-version` flag to `cargo install`.
  [#11859](https://github.com/rust-lang/cargo/pull/11859)
- The `CARGO_PKG_README` environment variable is now set to the path to the
  README file when compiling a crate.
  [#11645](https://github.com/rust-lang/cargo/pull/11645)
- Cargo now displays richer information of Cargo target failed to compile.
  [#11636](https://github.com/rust-lang/cargo/pull/11636)

### Changed

- 🎉 The `sparse` protocol is now the default protocol for crates.io!
  ([RFC 2789](https://github.com/rust-lang/rfcs/blob/master/text/2789-sparse-index.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/registries.html#registry-protocols))
  [#11791](https://github.com/rust-lang/cargo/pull/11791)
  [#11783](https://github.com/rust-lang/cargo/pull/11783)
- ❗ `cargo login` and `cargo logout` now uses the registry specified in
  `registry.default`. This was an unintentional regression.
  [#11949](https://github.com/rust-lang/cargo/pull/11949)
- `cargo update` accurately shows `Downgrading` status when downgrading dependencies.
  [#11839](https://github.com/rust-lang/cargo/pull/11839)
- Added more information to HTTP errors to help with debugging.
  [#11878](https://github.com/rust-lang/cargo/pull/11878)
- Added delays to network retries in Cargo.
  [#11881](https://github.com/rust-lang/cargo/pull/11881)
- Refined `cargo publish` message when waiting for a publish complete.
  [#11713](https://github.com/rust-lang/cargo/pull/11713)
- Better error message when `cargo install` from a git repository but found
  multiple packages.
  [#11835](https://github.com/rust-lang/cargo/pull/11835)

### Fixed

- Removed duplicates of possible values in `--charset` option of `cargo tree`.
  [#11785](https://github.com/rust-lang/cargo/pull/11785)
- Fixed `CARGO_CFG_` vars for configs defined both with and without value.
  [#11790](https://github.com/rust-lang/cargo/pull/11790)
- Broke endless loop on cyclic features in added dependency in `cargo add`.
  [#11805](https://github.com/rust-lang/cargo/pull/11805)
- Don't panic when `[patch]` involved in dependency resolution results in a conflict.
  [#11770](https://github.com/rust-lang/cargo/pull/11770)
- Fixed credential token format validation.
  [#11951](https://github.com/rust-lang/cargo/pull/11951)
- Added the missing token format validation on publish.
  [#11952](https://github.com/rust-lang/cargo/pull/11952)
- Fixed case mismatches when looking up env vars in the Config snapshot.
  [#11824](https://github.com/rust-lang/cargo/pull/11824)
- `cargo new` generates the correct `.hgignore` aligning semantics with other
  VCS ignore files.
  [#11855](https://github.com/rust-lang/cargo/pull/11855)
- Stopped doing unnecessary fuzzy registry index queries. This significantly
  reduces the amount of HTTP requests to remote registries for crates
  containing `-` or `_` in their names.
  [#11936](https://github.com/rust-lang/cargo/pull/11936)
  [#11937](https://github.com/rust-lang/cargo/pull/11937)

### Nightly only

- Added `-Zdirect-minimal-versions`. This behaves like `-Zminimal-versions` but
  only for direct dependencies.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#direct-minimal-versions))
  [#11688](https://github.com/rust-lang/cargo/pull/11688)
- Added `-Zgitoxide` which switches all `git fetch` operation in Cargo to
  use `gitoxide` crate. This is still an MVP but could improve the performance
  up to 2 times.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html##gitoxide))
  [#11448](https://github.com/rust-lang/cargo/pull/11448)
  [#11800](https://github.com/rust-lang/cargo/pull/11800)
  [#11822](https://github.com/rust-lang/cargo/pull/11822)
  [#11830](https://github.com/rust-lang/cargo/pull/11830)
- Removed `-Zjobserver-per-rustc`. Its rustc counterpart never got landed.
  [#11764](https://github.com/rust-lang/cargo/pull/11764)

### Documentation

- Cleaned-up unstable documentation.
  [#11793](https://github.com/rust-lang/cargo/pull/11793)
- Enhanced the documentation of timing report with graphs.
  [#11798](https://github.com/rust-lang/cargo/pull/11798)
- Clarified requirements about the state of the registry index after publish.
  [#11926](https://github.com/rust-lang/cargo/pull/11926)
- Clarified docs on `-C` that it appears before the command.
  [#11947](https://github.com/rust-lang/cargo/pull/11947)
- Clarified working directory behaviour for `cargo test`, `cargo bench` and
  `cargo run`.
  [#11901](https://github.com/rust-lang/cargo/pull/11901)
- Fixed the doc of `registries.name.index` configuration.
  [#11880](https://github.com/rust-lang/cargo/pull/11880)
- Notice for potential unexpected shell expansions in help text of `cargo-add`.
  [#11826](https://github.com/rust-lang/cargo/pull/11826)
- Updated external-tools JSON docs.
  [#11918](https://github.com/rust-lang/cargo/pull/11918)
- Call out the differences between the index JSON and the API or metadata. 
  [#11927](https://github.com/rust-lang/cargo/pull/11927)
- Consistently use `@` when mentioning pkgid format.
  [#11956](https://github.com/rust-lang/cargo/pull/11956)
- Enhanced Cargo Contributor Guide.
  [#11825](https://github.com/rust-lang/cargo/pull/11825)
  [#11842](https://github.com/rust-lang/cargo/pull/11842)
  [#11869](https://github.com/rust-lang/cargo/pull/11869)
  [#11876](https://github.com/rust-lang/cargo/pull/11876)
- Moved a part of Cargo Contributor Guide to Cargo API documentation.
  [docs](https://doc.rust-lang.org/nightly/nightly-rustc/cargo)
  [#11809](https://github.com/rust-lang/cargo/pull/11809)
  [#11841](https://github.com/rust-lang/cargo/pull/11841)
  [#11850](https://github.com/rust-lang/cargo/pull/11850)
  [#11870](https://github.com/rust-lang/cargo/pull/11870)
- Cargo team now arranges
  [office hours](https://github.com/rust-lang/cargo/wiki/Office-Hours)!
  [#11903](https://github.com/rust-lang/cargo/pull/11903)

### Internal

- Switched to `sha2` crate for SHA256 calculation.
  [#11795](https://github.com/rust-lang/cargo/pull/11795)
  [#11807](https://github.com/rust-lang/cargo/pull/11807)
- Switched benchsuite to the index archive.
  [#11933](https://github.com/rust-lang/cargo/pull/11933)
- Updated to `base64` 0.21.0.
  [#11796](https://github.com/rust-lang/cargo/pull/11796)
- Updated to `curl-sys` 0.4.61, which corresponds to curl 8.0.1.
  [#11871](https://github.com/rust-lang/cargo/pull/11871)
- Updated to `proptest` 1.1.0.
  [#11886](https://github.com/rust-lang/cargo/pull/11886)
- Updated to `git2` 0.17.0, which corresponds to libgit2 1.6.3.
  [#11928](https://github.com/rust-lang/cargo/pull/11928)
- Updated to `clap` 4.2.
  [#11904](https://github.com/rust-lang/cargo/pull/11904)
- Integrated `cargo-deny` in Cargo its own CI pipeline.
  [#11761](https://github.com/rust-lang/cargo/pull/11761)
- Made non-blocking IO calls more robust.
  [#11624](https://github.com/rust-lang/cargo/pull/11624)
- Dropped `derive` feature from `serde` in `cargo-platform`.
  [#11915](https://github.com/rust-lang/cargo/pull/11915)
- Replaced `std::fs::canonicalize` with a more robust `try_canonicalize`.
  [#11866](https://github.com/rust-lang/cargo/pull/11866)
- Enabled clippy warning on `disallowed_methods` for `std::env::var` and friends.
  [#11828](https://github.com/rust-lang/cargo/pull/11828)

## Cargo 1.69 (2023-04-20)
[985d561f...rust-1.69.0](https://github.com/rust-lang/cargo/compare/985d561f...rust-1.69.0)

### Added

- Cargo now suggests `cargo fix` or `cargo clippy --fix`
  when compilation warnings are auto-fixable.
  [#11558](https://github.com/rust-lang/cargo/pull/11558)
- Cargo now suggests `cargo add` if you try to install a library crate.
  [#11410](https://github.com/rust-lang/cargo/pull/11410)
- Cargo now sets the `CARGO_BIN_NAME` environment variable also for binary examples.
  [#11705](https://github.com/rust-lang/cargo/pull/11705)

### Changed

- ❗ When `default-features` is set to false of a workspace dependency,
  and an inherited dependency of a member has `default-features = true`,
  Cargo will enable default features of that dependency.
  [#11409](https://github.com/rust-lang/cargo/pull/11409)
- ❗ Deny `CARGO_HOME` in `[env]` configuration table. Cargo itself doesn't
  pick up this value, but recursive calls to cargo would, which was not intended.
  [#11644](https://github.com/rust-lang/cargo/pull/11644)
- ❗ Debuginfo for build dependencies is now off if not explicitly set. This is
  expected to improve the overall build time.
  [#11252](https://github.com/rust-lang/cargo/pull/11252)
- Cargo now emits errors on invalid alphanumeric characters in a registry token.
  [#11600](https://github.com/rust-lang/cargo/pull/11600)
- `cargo add` now checks only the order of `[dependencies]`
  without considering `[dependencies.*]`.
  [#11612](https://github.com/rust-lang/cargo/pull/11612)
- Cargo now respects the new jobserver IPC style in GNU Make 4.4, by updating
  its dependency `jobserver`.
  [#11767](https://github.com/rust-lang/cargo/pull/11767)
- `cargo install` now reports required features when no binary meets its requirements.
  [#11647](https://github.com/rust-lang/cargo/pull/11647)

### Fixed

- Uplifted `.dwp` DWARF package file next to the executable for debuggers to
  locate them.
  [#11572](https://github.com/rust-lang/cargo/pull/11572)
- Fixed build scripts triggering recompiles when a `rerun-if-changed` points to
  a directory whose mtime is not preserved by the filesystem.
  [#11613](https://github.com/rust-lang/cargo/pull/11613)
- Fixed panics when using dependencies from `[workspace.dependencies]`
  for `[patch]`. This usage is not supposed to be supported.
  [#11565](https://github.com/rust-lang/cargo/pull/11565)
  [#11630](https://github.com/rust-lang/cargo/pull/11630)
- Fixed `cargo report` saving the same future-incompat reports multiple times.
  [#11648](https://github.com/rust-lang/cargo/pull/11648)
- Fixed the incorrect inference of a directory ending with `.rs` as a file.
  [#11678](https://github.com/rust-lang/cargo/pull/11678)
- Fixed `.cargo-ok` file being truncated wrongly, preventing from using a dependency.
  [#11665](https://github.com/rust-lang/cargo/pull/11665)
  [#11724](https://github.com/rust-lang/cargo/pull/11724)

### Nightly only

- `-Zrustdoc-scrape-example` must fail with bad build script.
  [#11694](https://github.com/rust-lang/cargo/pull/11694)
- Updated 1password credential manager integration to the version 2 CLI.
  [#11692](https://github.com/rust-lang/cargo/pull/11692)
- Emit an error message for transitive artifact dependencies with targets the
  package doesn't directly interact with.
  [#11643](https://github.com/rust-lang/cargo/pull/11643)
- Added `-C` flag for changing current dir before build starts.
  [#10952](https://github.com/rust-lang/cargo/pull/10952)

### Documentation

- Clarified the difference between `CARGO_CRATE_NAME` and `CARGO_PKG_NAME`.
  [#11576](https://github.com/rust-lang/cargo/pull/11576)
- Added links to the Target section of the glossary for occurrences of target triple.
  [#11603](https://github.com/rust-lang/cargo/pull/11603)
- Described how the current resolver sometimes duplicates dependencies.
  [#11604](https://github.com/rust-lang/cargo/pull/11604)
- Added a note about verifying your email address on crates.io.
  [#11620](https://github.com/rust-lang/cargo/pull/11620)
- Mention current default value in `publish.timeout` docs.
  [#11652](https://github.com/rust-lang/cargo/pull/11652)
- More doc comments for `cargo::core::compiler` modules.
  [#11669](https://github.com/rust-lang/cargo/pull/11669)
  [#11703](https://github.com/rust-lang/cargo/pull/11703)
  [#11711](https://github.com/rust-lang/cargo/pull/11711)
  [#11758](https://github.com/rust-lang/cargo/pull/11758)
- Added more guidance on how to implement unstable features.
  [#11675](https://github.com/rust-lang/cargo/pull/11675)
- Fixed unstable chapter layout for `codegen-backend`.
  [#11676](https://github.com/rust-lang/cargo/pull/11676)
- Add a link to LTO doc.
  [#11701](https://github.com/rust-lang/cargo/pull/11701)
- Added documentation for the configuration discovery of `cargo install`
  to the man pages
  [#11763](https://github.com/rust-lang/cargo/pull/11763)
- Documented `-F` flag as an alias for `--features` in `cargo add`.
  [#11774](https://github.com/rust-lang/cargo/pull/11774)

### Internal

- Disable network SSH tests on Windows.
  [#11610](https://github.com/rust-lang/cargo/pull/11610)
- Made some blocking tests non-blocking.
  [#11650](https://github.com/rust-lang/cargo/pull/11650)
- Deny warnings in CI, not locally.
  [#11699](https://github.com/rust-lang/cargo/pull/11699)
- Re-export `cargo_new::NewProjectKind` as public.
  [#11700](https://github.com/rust-lang/cargo/pull/11700)
- Made dependencies in alphabetical order.
  [#11719](https://github.com/rust-lang/cargo/pull/11719)
- Switched some tests from `build` to `check`.
  [#11725](https://github.com/rust-lang/cargo/pull/11725)
- Consolidated how Cargo reads environments variables internally.
  [#11727](https://github.com/rust-lang/cargo/pull/11727)
  [#11754](https://github.com/rust-lang/cargo/pull/11754)
- Fixed tests with nondeterministic ordering
  [#11766](https://github.com/rust-lang/cargo/pull/11766)
- Added a test to verify the intermediate artifacts persist in the temp directory.
  [#11771](https://github.com/rust-lang/cargo/pull/11771)
- Updated cross test instructions for aarch64-apple-darwin.
  [#11663](https://github.com/rust-lang/cargo/pull/11663)
- Updated to `toml` v0.6 and `toml_edit` v0.18 for TOML manipulations.
  [#11618](https://github.com/rust-lang/cargo/pull/11618)
- Updated to `clap` v4.1.3.
  [#11619](https://github.com/rust-lang/cargo/pull/11619)
- Replaced `winapi` with `windows-sys` crate for Windows bindings.
  [#11656](https://github.com/rust-lang/cargo/pull/11656)
- Reused `url` crate for percent encoding instead of `percent-encoding`.
  [#11750](https://github.com/rust-lang/cargo/pull/11750)
- Cargo contributors can benefit from smart punctuations when writing
  documentations, e.g., `---` is auto-converted into an em dash.
  ([docs](https://rust-lang.github.io/mdBook/format/markdown.html#smart-punctuation))
  [#11646](https://github.com/rust-lang/cargo/pull/11646)
  [#11715](https://github.com/rust-lang/cargo/pull/11715)
- Cargo's CI pipeline now covers macOS on nightly.
  [#11712](https://github.com/rust-lang/cargo/pull/11712)
- Re-enabled some clippy lints in Cargo itself.
  [#11722](https://github.com/rust-lang/cargo/pull/11722)
- Enabled sparse protocol in Cargo's CI.
  [#11632](https://github.com/rust-lang/cargo/pull/11632)
- Pull requests in Cargo now get autolabelled for label `A-*` and `Command-*`.
  [#11664](https://github.com/rust-lang/cargo/pull/11664)
  [#11679](https://github.com/rust-lang/cargo/pull/11679)

## Cargo 1.68.2 (2023-03-28)
[115f3455...rust-1.68.0](https://github.com/rust-lang/cargo/compare/115f3455...rust-1.68.0)

- Updated the GitHub RSA SSH host key bundled within cargo.
  The key was [rotated by
  GitHub](https://github.blog/2023-03-23-we-updated-our-rsa-ssh-host-key/) on
  2023-03-24 after the old one leaked.
  [#11883](https://github.com/rust-lang/cargo/pull/11883)
- Added support for SSH known hosts marker `@revoked`.
  [#11635](https://github.com/rust-lang/cargo/pull/11635)
- Marked the old GitHub RSA host key as revoked. This will prevent Cargo from
  accepting the leaked key even when trusted by the system.
  [#11889](https://github.com/rust-lang/cargo/pull/11889)

## Cargo 1.68 (2023-03-09)
[f6e737b1...rust-1.68.0](https://github.com/rust-lang/cargo/compare/f6e737b1...rust-1.68.0)

### Added

- 🎉 The new "sparse" protocol has been stabilized.
  It should provide a significant performance improvement when accessing crates.io.
  ([RFC 2789](https://github.com/rust-lang/rfcs/blob/master/text/2789-sparse-index.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/registries.html#registry-protocols))
  [#11224](https://github.com/rust-lang/cargo/pull/11224)
  [#11480](https://github.com/rust-lang/cargo/pull/11480)
  [#11733](https://github.com/rust-lang/cargo/pull/11733)
  [#11756](https://github.com/rust-lang/cargo/pull/11756)
- 🎉 `home` crate is now a subcrate in `rust-lang/cargo` repository. Welcome!
  [#11359](https://github.com/rust-lang/cargo/pull/11359)
  [#11481](https://github.com/rust-lang/cargo/pull/11481)
- Long diagnostic messages now can be truncated to be more readable.
  [#11494](https://github.com/rust-lang/cargo/pull/11494)
- Shows the progress of crates.io index update even when `net.git-fetch-with-cli` enabled.
  [#11579](https://github.com/rust-lang/cargo/pull/11579)
- `cargo build --verbose` tells you more about why it recompiles.
  [#11407](https://github.com/rust-lang/cargo/pull/11407)
- Cargo's file locking mechanism now supports Solaris by using `fcntl`.
  [#11439](https://github.com/rust-lang/cargo/pull/11439)
  [#11474](https://github.com/rust-lang/cargo/pull/11474)
- Added a new SemVer compatibility rule explaining the expectations around diagnostic lints 
  [#11596](https://github.com/rust-lang/cargo/pull/11596)
- `cargo vendor` generates a different source replacement entry
  for each revision from the same git repository.
  [#10690](https://github.com/rust-lang/cargo/pull/1090)
- Cargo contributors can relabel issues via triagebot.
  [doc](https://forge.rust-lang.org/triagebot/labeling.html)
  [#11498](https://github.com/rust-lang/cargo/pull/11498)
- Cargo contributors can write tests in containers.
  [#11583](https://github.com/rust-lang/cargo/pull/11583)

### Changed

- Cargo now by default saves credentials to `.cargo/credentials.toml`.
  If `.cargo/credentials` exists, writes to it for backward compatibility reasons.
  [#11533](https://github.com/rust-lang/cargo/pull/11533)
- To prevent sensitive data from being logged, Cargo introduces a new wrapper type internally.
  [#11545](https://github.com/rust-lang/cargo/pull/11545)
- Several documentation improvements.
  [#11475](https://github.com/rust-lang/cargo/pull/11475)
  [#11504](https://github.com/rust-lang/cargo/pull/11504)
  [#11516](https://github.com/rust-lang/cargo/pull/11516)
  [#11517](https://github.com/rust-lang/cargo/pull/11517)
  [#11568](https://github.com/rust-lang/cargo/pull/11568)
  [#11586](https://github.com/rust-lang/cargo/pull/11586)
  [#11592](https://github.com/rust-lang/cargo/pull/11592)

### Fixed

- ❗ `cargo package` and `cargo publish` now respects workspace's `Cargo.lock`.
  This is an expected behavior but previously got overlooked.
  [#11477](https://github.com/rust-lang/cargo/pull/11477)
- Fixed `cargo vendor` failing on resolving git dependencies inherited from a workspace.
  [#11414](https://github.com/rust-lang/cargo/pull/11414)
- `cargo install` can now correctly install root package when `workspace.default-members` is specified.
  [#11067](https://github.com/rust-lang/cargo/pull/11067)
- Fixed panic on target specific dependency errors.
  [#11541](https://github.com/rust-lang/cargo/pull/11541)
- Shows `--help` if there is no man page for a subcommand.
  [#11473](https://github.com/rust-lang/cargo/pull/11473)
- Setting `target.cfg(…).rustflags` shouldn't erase `build.rustdocflags`.
  [#11323](https://github.com/rust-lang/cargo/pull/11323)
- Unsupported `profile.split-debuginfo` options are now ignored,
  which previously made Cargo fail to compile on certain platforms.
  [#11347](https://github.com/rust-lang/cargo/pull/11347)
  [#11633](https://github.com/rust-lang/cargo/pull/11633)
- Don't panic in Windows headless session with really long file names.
  [#11759](https://github.com/rust-lang/cargo/pull/11759)

### Nightly only

- Implemented initial support of asymmetric token authentication for registries.
  ([RFC 3231](https://github.com/rust-lang/rfcs/blob/master/text/3231-cargo-asymmetric-tokens.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#registry-auth))
  [#10771](https://github.com/rust-lang/cargo/pull/10771)
- Do not error for `auth-required: true` without `-Z sparse-registry`
  [#11661](https://github.com/rust-lang/cargo/pull/11661)
- Supports `codegen-backend` and `rustflags` in profiles in config file.
  [#11562](https://github.com/rust-lang/cargo/pull/11562)
- Suggests `cargo clippy --fix` when warnings/errors could be fixed with clippy.
  [#11399](https://github.com/rust-lang/cargo/pull/11399)
- Fixed artifact deps not working when target field specified coexists with `optional = true`.
  [#11434](https://github.com/rust-lang/cargo/pull/11434)
- Make Cargo distinguish `Unit`s with and without artifact targets.
  [#11478](https://github.com/rust-lang/cargo/pull/11478)
- `cargo metadata` supports artifact dependencies.
  [#11550](https://github.com/rust-lang/cargo/pull/11550)
- Allows builds of some crate to fail during optional doc-scraping.
  [#11450](https://github.com/rust-lang/cargo/pull/11450)
- Add warning if potentially-scrapable examples are skipped due to dev-dependencies.
  [#11503](https://github.com/rust-lang/cargo/pull/11503)
- Don't scrape examples from library targets by default. 
  [#11499](https://github.com/rust-lang/cargo/pull/11499)
- Fixed examples of proc-macro crates being scraped for examples.
  [#11497](https://github.com/rust-lang/cargo/pull/11497)

## Cargo 1.67 (2023-01-26)
[7e484fc1...rust-1.67.0](https://github.com/rust-lang/cargo/compare/7e484fc1...rust-1.67.0)

### Added

- `cargo remove` now cleans up the referenced dependency of the root
  workspace manifest, `profile`, `patch`, and `replace` sections
  after a successful removal of a dependency.
  [#11194](https://github.com/rust-lang/cargo/pull/11194)
  [#11242](https://github.com/rust-lang/cargo/pull/11242)
  [#11351](https://github.com/rust-lang/cargo/pull/11351)
- `cargo package` and `cargo publish` now report total and compressed crate size
  after packaging.
  [#11270](https://github.com/rust-lang/cargo/pull/11270)

### Changed

- ❗ Cargo now reuses the value of `$CARGO` if it's already set in the environment,
  and forwards the value when executing external subcommands and build scripts.
  [#11285](https://github.com/rust-lang/cargo/pull/11285)
- ❗ Cargo now emits an error when running `cargo update --precise` without a `-p` flag.
  [#11349](https://github.com/rust-lang/cargo/pull/11349)
- ❗ Cargo now emits an error if there are multiple registries in the configuration with the same index URL.
  [#10592](https://github.com/rust-lang/cargo/pull/10592)
- Cargo now is aware of compression ratio when extracting crate files.
  This relaxes the hard size limit introduced in 1.64.0 to mitigate zip bomb attack.
  [#11337](https://github.com/rust-lang/cargo/pull/11337)
- Cargo now errors out when `cargo fix` on a git repo with uncommitted changes.
  [#11400](https://github.com/rust-lang/cargo/pull/11400)
- Cargo now warns when `cargo tree -i <spec>` cannot find any package.
  [#11377](https://github.com/rust-lang/cargo/pull/11377)
- Cargo now warns when running `cargo new/init` and `PATH` env separator
  is in the project path.
  [#11318](https://github.com/rust-lang/cargo/pull/11318)
- Better error messages when multiple packages were found and 
  `cargo add/remove` gets confused.
  [#11186](https://github.com/rust-lang/cargo/pull/11186)
  [#11375](https://github.com/rust-lang/cargo/pull/11375)
- A better error message when `cargo init` but existing ignore files aren't UTF-8.
  [#11321](https://github.com/rust-lang/cargo/pull/11321)
- A better error message for `cargo install .`.
  [#11401](https://github.com/rust-lang/cargo/pull/11401)
- A better warning when the same file path found in multiple build targets.
  [#11299](https://github.com/rust-lang/cargo/pull/11299)
- Updated the internal HTTP library libcurl with various fixes and updates.
  [#11307](https://github.com/rust-lang/cargo/pull/11307)
  [#11326](https://github.com/rust-lang/cargo/pull/11326)

### Fixed

- Fixed `cargo clean` for removing fingerprints and build script
  artifacts of only the requested package
  [#10621](https://github.com/rust-lang/cargo/pull/10621)
- Fixed `cargo install --index` not working when config `registry.default` is set.
  [#11302](https://github.com/rust-lang/cargo/pull/11302)
- Fixed git2 safe-directory accidentally disabled when no network configuration was found.
  [#11366](https://github.com/rust-lang/cargo/pull/11366)
- Migrate from crate `atty` to resolve potential soundness issue.
  [#11420](https://github.com/rust-lang/cargo/pull/11420)
- Cleans stale git temp files left when libgit2 indexing is interrupted.
  [#11308](https://github.com/rust-lang/cargo/pull/11308)

### Nightly only

- Suggests `cargo fix` when some compilation warnings/errors can be auto-fixed.
  [#10989](https://github.com/rust-lang/cargo/pull/10989)
  [#11368](https://github.com/rust-lang/cargo/pull/11368)
- Changed `rustdoc-scrape-examples` to be a target-level configuration.
  [#10343](https://github.com/rust-lang/cargo/pull/10343)
  [#11425](https://github.com/rust-lang/cargo/pull/11425)
  [#11430](https://github.com/rust-lang/cargo/pull/11430)
  [#11445](https://github.com/rust-lang/cargo/pull/11445)
- Propagates change of artifact bin dependency to its parent fingerprint.
  [#11353](https://github.com/rust-lang/cargo/pull/11353)
- Fixed `wait-for-publish` to work with sparse registry.
  [#11356](https://github.com/rust-lang/cargo/pull/11356)
  [#11327](https://github.com/rust-lang/cargo/pull/11327)
  [#11388](https://github.com/rust-lang/cargo/pull/11388)
- Stores the `sparse+` prefix in the `SourceId` for sparse registries 
  [#11387](https://github.com/rust-lang/cargo/pull/11387)
  [#11403](https://github.com/rust-lang/cargo/pull/11403)
- Implemented alternative registry authentication support.
  ([RFC 3139](https://github.com/rust-lang/rfcs/blob/master/text/3139-cargo-alternative-registry-auth.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#registry-auth))
  [#10592](https://github.com/rust-lang/cargo/pull/10592)
- Added documentation of config option `registries.crates-io.protocol`.
  [#11350](https://github.com/rust-lang/cargo/pull/11350)

## Cargo 1.66.1 (2023-01-10)

### Fixed
- 🚨 [CVE-2022-46176](https://github.com/rust-lang/cargo/security/advisories/GHSA-r5w3-xm58-jv6j):
  Added validation of SSH host keys for git URLs.
  See [the docs](https://doc.rust-lang.org/cargo/appendix/git-authentication.html#ssh-known-hosts) for more information on how to configure the known host keys.

## Cargo 1.66 (2022-12-15)
[08250398...rust-1.66.0](https://github.com/rust-lang/cargo/compare/08250398...rust-1.66.0)

### Added

- 🎉 Added `cargo remove` command for removing dependencies from `Cargo.toml`.
  [docs](https://doc.rust-lang.org/nightly/cargo/commands/cargo-remove.html)
  [#11059](https://github.com/rust-lang/cargo/pull/11059)
  [#11099](https://github.com/rust-lang/cargo/pull/11099)
  [#11193](https://github.com/rust-lang/cargo/pull/11193)
  [#11204](https://github.com/rust-lang/cargo/pull/11204)
  [#11227](https://github.com/rust-lang/cargo/pull/11227)
- Added support for git dependencies having git submodules with relative paths.
  [#11106](https://github.com/rust-lang/cargo/pull/11106)
- Cargo now sends requests with an `Accept-Encoding` header to registries.
  [#11292](https://github.com/rust-lang/cargo/pull/11292)
- Cargo now forwards non-UTF8 arguments to external subcommands.
  [#11118](https://github.com/rust-lang/cargo/pull/11118)

### Changed

- ❗ Disambiguate source replacements from various angles.
  [RFC-3289](https://github.com/rust-lang/rfcs/blob/master/text/3289-source_replacement_ambiguity.md)
  [#10907](https://github.com/rust-lang/cargo/pull/10907)
  - When the crates-io source is replaced, the user is required to specify which registry to use with `--registry <NAME>` when performing an API operation.
  - Publishing to source-replaced crates.io is no longer permitted using the crates.io token (`registry.token`).
  - In source replacement, the `replace-with` key can reference the name of an alternative registry in the `[registries]` table.
- ❗ `cargo publish` now blocks until it sees the published package in the index.
  [#11062](https://github.com/rust-lang/cargo/pull/11062)
  [#11210](https://github.com/rust-lang/cargo/pull/11210)
  [#11216](https://github.com/rust-lang/cargo/pull/11216)
  [#11255](https://github.com/rust-lang/cargo/pull/11255)
- Cargo now uses the clap v4 library for command-line argument parsing.
  [#11116](https://github.com/rust-lang/cargo/pull/11116)
  [#11119](https://github.com/rust-lang/cargo/pull/11119)
  [#11159](https://github.com/rust-lang/cargo/pull/11159)
  [#11190](https://github.com/rust-lang/cargo/pull/11190)
  [#11239](https://github.com/rust-lang/cargo/pull/11239)
  [#11280](https://github.com/rust-lang/cargo/pull/11280)
- Cargo now only warns on a user-defined alias shadowing an external command.
  [#11170](https://github.com/rust-lang/cargo/pull/11170)
- Several documentation improvements.
  [#10770](https://github.com/rust-lang/cargo/pull/10770)
  [#10938](https://github.com/rust-lang/cargo/pull/10938)
  [#11082](https://github.com/rust-lang/cargo/pull/11082)
  [#11093](https://github.com/rust-lang/cargo/pull/11093)
  [#11157](https://github.com/rust-lang/cargo/pull/11157)
  [#11185](https://github.com/rust-lang/cargo/pull/11185)
  [#11207](https://github.com/rust-lang/cargo/pull/11207)
  [#11219](https://github.com/rust-lang/cargo/pull/11219)
  [#11240](https://github.com/rust-lang/cargo/pull/11240)
  [#11241](https://github.com/rust-lang/cargo/pull/11241)
  [#11282](https://github.com/rust-lang/cargo/pull/11282)

### Fixed

- ❗ Config file loaded via `cargo --config <file>` now takes priority over
  environment variables. This is a documented behaviour but the old
  implementation accidentally got it wrong.
  [#11077](https://github.com/rust-lang/cargo/pull/11077)
- ❗ Cargo collects rustflags in `target.cfg(…).rustflags` more correctly
  and warns if that's not enough for convergence.
  [#11114](https://github.com/rust-lang/cargo/pull/11114)
- Final artifacts not removed by linker should be removed before a compilation gets started.
  [#11122](https://github.com/rust-lang/cargo/pull/11122)
- `cargo add` now reports unknown features in a more discoverable manner.
  [#11098](https://github.com/rust-lang/cargo/pull/11098)
- Cargo now reports command aliasing failure with more error contexts.
  [#11087](https://github.com/rust-lang/cargo/pull/11087)
- A better error message when `cargo login` prompt receives empty input.
  [#11145](https://github.com/rust-lang/cargo/pull/11145)
- A better error message for fields with wrong types
  where workspace inheritance is supported.
  [#11113](https://github.com/rust-lang/cargo/pull/11113)
- A better error message when mixing feature syntax `dep:` with `/`.
  [#11172](https://github.com/rust-lang/cargo/pull/11172)
- A better error message when publishing but `package.publish` is `false`
  in the manifest.
  [#11280](https://github.com/rust-lang/cargo/pull/11280)

### Nightly only

- Added new config option `publish.timeout` behind `-Zpublish-timeout`.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#publish-timeout)
  [#11230](https://github.com/rust-lang/cargo/pull/11230)
- Added retry support to sparse registries.
  [#11069](https://github.com/rust-lang/cargo/pull/11069)
- Fixed sparse registry lockfile urls containing `registry+sparse+`.
  [#11177](https://github.com/rust-lang/cargo/pull/11177)
- Add new config option `registries.crates-io.protocol`
  for controlling crates.io protocol.
  [#11215](https://github.com/rust-lang/cargo/pull/11215)
- Removed `sparse+` prefix for index.crates.io.
  [#11247](https://github.com/rust-lang/cargo/pull/11247)
- Fixed publishing with a dependency on a sparse registry.
  [#11268](https://github.com/rust-lang/cargo/pull/11268)
- Fixed confusing error messages when using `-Zsparse-registry`.
  [#11283](https://github.com/rust-lang/cargo/pull/11283)
- Fixed 410 gone response handling for sparse registries.
  [#11286](https://github.com/rust-lang/cargo/pull/11286)

## Cargo 1.65 (2022-11-03)
[4fd148c4...rust-1.65.0](https://github.com/rust-lang/cargo/compare/4fd148c4...rust-1.65.0)

### Added

- External subcommands can now inherit jobserver file descriptors from Cargo.
  [#10511](https://github.com/rust-lang/cargo/pull/10511)
- Added an API documentation for private items in cargo-the-library. See
  <https://doc.rust-lang.org/nightly/nightly-rustc/cargo>.
  [#11019](https://github.com/rust-lang/cargo/pull/11019)

### Changed

- Cargo now stops adding its bin path to `PATH` if it's already there.
  [#11023](https://github.com/rust-lang/cargo/pull/11023)
- Improved the performance of Cargo build scheduling
  by sorting the queue of pending jobs.
  [#11032](https://github.com/rust-lang/cargo/pull/11032)
- Improved the performance fetching git dependencies from GitHub even
  when using a partial hash in the `rev` field.
  [#10807](https://github.com/rust-lang/cargo/pull/10807)
- Cargo now uses git2 v0.15 and libgit2-sys v0.14,
  which bring several compatibility fixes with git's new behaviors.
  [#11004](https://github.com/rust-lang/cargo/pull/11004)
- Registry index files are cached in a more granular way based on content hash.
  [#11044](https://github.com/rust-lang/cargo/pull/11044)
- Cargo now uses the standard library's `std::thread::scope` instead of the
  `crossbeam` crate for spawning scoped threads.
  [#10977](https://github.com/rust-lang/cargo/pull/10977)
- Cargo now uses the standard library's `available_parallelism` instead of the
  `num_cpus` crate for determining the default parallelism.
  [#10969](https://github.com/rust-lang/cargo/pull/10969)
- Cargo now guides you how to solve it when seeing an error message of
  `rust-version` requirement not satisfied.
  [#10891](https://github.com/rust-lang/cargo/pull/10891)
- Cargo now tells you more about possible causes and how to fix it
  when a subcommand cannot be found.
  [#10924](https://github.com/rust-lang/cargo/pull/10924)
- Cargo now lists available target names when a given Cargo target cannot be found.
  [#10999](https://github.com/rust-lang/cargo/pull/10999)
- `cargo update` now warns if `--precise` is given without `--package` flag.
  This will become a hard error after a transition period.
  [#10988](https://github.com/rust-lang/cargo/pull/10988)
  [#11011](https://github.com/rust-lang/cargo/pull/11011)
- `cargo bench` and `cargo test` now report a more precise test execution error
  right after a test fails.
  [#11028](https://github.com/rust-lang/cargo/pull/11028)
- `cargo add` now tells you for which version the features are added.
  [#11075](https://github.com/rust-lang/cargo/pull/11075)
- Call out that non-ASCII crate names are not supported by Rust anymore.
  [#11017](https://github.com/rust-lang/cargo/pull/11017)
- Enhanced the error message when in the manifest a field is expected to be
  an array but a string is used.
  [#10944](https://github.com/rust-lang/cargo/pull/10944)

### Fixed

- Removed the restriction on file locking supports on platforms other than Linux.
  [#10975](https://github.com/rust-lang/cargo/pull/10975)
- Fixed incorrect OS detection by bumping os_info to 3.5.0.
  [#10943](https://github.com/rust-lang/cargo/pull/10943)
- Scanning the package directory now ignores errors from broken
  but excluded symlink files.
  [#11008](https://github.com/rust-lang/cargo/pull/11008)
- Fixed deadlock when build scripts are waiting for input on stdin.
  [#11257](https://github.com/rust-lang/cargo/pull/11257)

### Nightly

- Progress indicator for sparse registries becomes more straightforward.
  [#11068](https://github.com/rust-lang/cargo/pull/11068)

## Cargo 1.64 (2022-09-22)
[a5e08c47...rust-1.64.0](https://github.com/rust-lang/cargo/compare/a5e08c47...rust-1.64.0)

### Added

- 🎉 Packages can now inherit settings from the workspace so that the settings
  can be centralized in one place. See
  [`workspace.package`](https://doc.rust-lang.org/nightly/cargo/reference/workspaces.html#the-package-table)
  and
  [`workspace.dependencies`](https://doc.rust-lang.org/nightly/cargo/reference/workspaces.html#the-dependencies-table)
  for more details on how to define these common settings.
  [#10859](https://github.com/rust-lang/cargo/pull/10859)
- Added the
  [`--crate-type`](https://doc.rust-lang.org/nightly/cargo/commands/cargo-rustc.html#option-cargo-rustc---crate-type)
  flag to `cargo rustc` to override the crate type.
  [#10838](https://github.com/rust-lang/cargo/pull/10838)
- Cargo commands can now accept multiple `--target` flags to build for
  multiple targets at once, and the
  [`build.target`](https://doc.rust-lang.org/nightly/cargo/reference/config.html#buildtarget)
  config option may now take an array of multiple targets.
  [#10766](https://github.com/rust-lang/cargo/pull/10766)
- The `--jobs` argument can now take a negative number to count backwards from
  the max CPUs.
  [#10844](https://github.com/rust-lang/cargo/pull/10844)

### Changed
- Bash completion of `cargo install --path` now supports path completion.
  [#10798](https://github.com/rust-lang/cargo/pull/10798)
- Significantly improved the performance fetching git dependencies from GitHub
  when using a hash in the `rev` field.
  [#10079](https://github.com/rust-lang/cargo/pull/10079)
- Published packages will now include the resolver setting from the workspace
  to ensure that they use the same resolver when used in isolation.
  [#10911](https://github.com/rust-lang/cargo/pull/10911)
  [#10961](https://github.com/rust-lang/cargo/pull/10961)
  [#10970](https://github.com/rust-lang/cargo/pull/10970)
- `cargo add` will now update `Cargo.lock`.
  [#10902](https://github.com/rust-lang/cargo/pull/10902)
- The path in the config output of `cargo vendor` now translates backslashes
  to forward slashes so that the settings should work across platforms.
  [#10668](https://github.com/rust-lang/cargo/pull/10668)
- The
  [`workspace.default-members`](https://doc.rust-lang.org/nightly/cargo/reference/workspaces.html#package-selection)
  setting now allows a value of `"."` in a non-virtual workspace to refer to
  the root package.
  [#10784](https://github.com/rust-lang/cargo/pull/10784)

### Fixed

- 🚨 [CVE-2022-36113](https://github.com/rust-lang/cargo/security/advisories/GHSA-rfj2-q3h3-hm5j):
  Extracting malicious crates can corrupt arbitrary files.
  [#11089](https://github.com/rust-lang/cargo/pull/11089)
  [#11088](https://github.com/rust-lang/cargo/pull/11088)
- 🚨 [CVE-2022-36114](https://github.com/rust-lang/cargo/security/advisories/GHSA-2hvr-h6gw-qrxp):
  Extracting malicious crates can fill the file system.
  [#11089](https://github.com/rust-lang/cargo/pull/11089)
  [#11088](https://github.com/rust-lang/cargo/pull/11088)
- The `os` output in `cargo --version --verbose` now supports more platforms.
  [#10802](https://github.com/rust-lang/cargo/pull/10802)
- Cached git checkouts will now be rebuilt if they are corrupted. This may
  happen when using `net.git-fetch-with-cli` and interrupting the clone
  process.
  [#10829](https://github.com/rust-lang/cargo/pull/10829)
- Fixed panic in `cargo add --offline`.
  [#10817](https://github.com/rust-lang/cargo/pull/10817)


### Nightly only
- Fixed deserialization of unstable `check-cfg` in `config.toml`.
  [#10799](https://github.com/rust-lang/cargo/pull/10799)


## Cargo 1.63 (2022-08-11)
[3f052d8e...rust-1.63.0](https://github.com/rust-lang/cargo/compare/3f052d8e...rust-1.63.0)

### Added

- 🎉 Added the `--config` CLI option to pass config options directly on the CLI.
  [#10755](https://github.com/rust-lang/cargo/pull/10755)
- The `CARGO_PKG_RUST_VERSION` environment variable is now set when compiling
  a crate if the manifest has the `rust-version` field set.
  [#10713](https://github.com/rust-lang/cargo/pull/10713)


### Changed
- A warning is emitted when encountering multiple packages with the same name
  in a git dependency. This will ignore packages with `publish=false`.
  [#10701](https://github.com/rust-lang/cargo/pull/10701)
  [#10767](https://github.com/rust-lang/cargo/pull/10767)
- Change tracking now uses the contents of a `.json` target spec file instead
  of its path. This should help avoid rebuilds if the path changes.
  [#10746](https://github.com/rust-lang/cargo/pull/10746)
- Git dependencies with a submodule configured with the `update=none` strategy
  in `.gitmodules` is now honored, and the submodule will not be fetched.
  [#10717](https://github.com/rust-lang/cargo/pull/10717)
- Crate files now use a more recent date (Jul 23, 2006 instead of Nov 29, 1973)
  for deterministic behavior.
  [#10720](https://github.com/rust-lang/cargo/pull/10720)
- The initial template used for `cargo new` now includes a slightly more
  realistic test structure that has `use super::*;` in the test module.
  [#10706](https://github.com/rust-lang/cargo/pull/10706)
- Updated the internal HTTP library libcurl with various small fixes and updates.
  [#10696](https://github.com/rust-lang/cargo/pull/10696)

### Fixed
- Fix zsh completions for `cargo add` and `cargo locate-project`
  [#10810](https://github.com/rust-lang/cargo/pull/10810)
  [#10811](https://github.com/rust-lang/cargo/pull/10811)
- Fixed `-p` being ignored with `cargo publish` in the root of a virtual
  workspace. Some additional checks were also added to generate an error if
  multiple packages were selected (previously it would pick the first one).
  [#10677](https://github.com/rust-lang/cargo/pull/10677)
- The human-readable executable name is no longer displayed for `cargo test`
  when using JSON output.
  [#10691](https://github.com/rust-lang/cargo/pull/10691)

### Nightly only

- Added `-Zcheck-cfg=output` to support build-scripts declaring their
  supported set of `cfg` values with `cargo:rustc-check-cfg`.
  [#10539](https://github.com/rust-lang/cargo/pull/10539)
- `-Z sparse-registry` now uses https://index.crates.io/ when accessing crates-io.
  [#10725](https://github.com/rust-lang/cargo/pull/10725)
- Fixed formatting of `.workspace` key in `cargo add` for workspace inheritance.
  [#10705](https://github.com/rust-lang/cargo/pull/10705)
- Sparse HTTP registry URLs must now end with a `/`.
  [#10698](https://github.com/rust-lang/cargo/pull/10698)
- Fixed issue with `cargo add` and workspace inheritance of the `default-features` key.
  [#10685](https://github.com/rust-lang/cargo/pull/10685)



## Cargo 1.62 (2022-06-30)
[1ef1e0a1...rust-1.62.0](https://github.com/rust-lang/cargo/compare/1ef1e0a1...rust-1.62.0)

### Added

- 🎉 Added the `cargo add` command for adding dependencies to `Cargo.toml` from
  the command-line.
  [docs](https://doc.rust-lang.org/nightly/cargo/commands/cargo-add.html)
  [#10472](https://github.com/rust-lang/cargo/pull/10472)
  [#10577](https://github.com/rust-lang/cargo/pull/10577)
  [#10578](https://github.com/rust-lang/cargo/pull/10578)
- Package ID specs now support `name@version` syntax in addition to the
  previous `name:version` to align with the behavior in `cargo add` and other
  tools. `cargo install` and `cargo yank` also now support this syntax so the
  version does not need to passed as a separate flag.
  [#10582](https://github.com/rust-lang/cargo/pull/10582)
  [#10650](https://github.com/rust-lang/cargo/pull/10650)
  [#10597](https://github.com/rust-lang/cargo/pull/10597)
- Added the CLI option `-F` as an alias of `--features`.
  [#10576](https://github.com/rust-lang/cargo/pull/10576)
- The `git` and `registry` directories in Cargo's home directory (usually
  `~/.cargo`) are now marked as cache directories so that they are not
  included in backups or content indexing (on Windows).
  [#10553](https://github.com/rust-lang/cargo/pull/10553)
- Added the `--version` flag to `cargo yank` to replace the `--vers` flag to
  be consistent with `cargo install`.
  [#10575](https://github.com/rust-lang/cargo/pull/10575)
- Added automatic `@` argfile support, which will use "response files" if the
  command-line to `rustc` exceeds the operating system's limit.
  [#10546](https://github.com/rust-lang/cargo/pull/10546)
- `cargo clean` now has a progress bar (if it takes longer than half a second).
  [#10236](https://github.com/rust-lang/cargo/pull/10236)

### Changed

- `cargo install` no longer generates an error if no binaries were found
  to install (such as missing required features).
  [#10508](https://github.com/rust-lang/cargo/pull/10508)
- `cargo test` now passes `--target` to `rustdoc` if the specified target is
  the same as the host target.
  [#10594](https://github.com/rust-lang/cargo/pull/10594)
- `cargo doc` now automatically passes `-Arustdoc::private-intra-doc-links`
  when documenting a binary (which automatically includes
  `--document-private-items`). The
  [`private-intra-doc-links`](https://doc.rust-lang.org/rustdoc/lints.html#private_intra_doc_links)
  lint is only relevant when *not* documenting private items, which doesn't
  apply to binaries.
  [#10142](https://github.com/rust-lang/cargo/pull/10142)
- The length of the short git hash in the `cargo --version` output is now
  fixed to 9 characters. Previously the length was inconsistent between
  different platforms.
  [#10579](https://github.com/rust-lang/cargo/pull/10579)
- Attempting to publish a package with a `Cargo.toml.orig` file will now
  result in an error. The filename would otherwise conflict with the
  automatically-generated file.
  [#10551](https://github.com/rust-lang/cargo/pull/10551)

### Fixed

- The `build.dep-info-basedir` configuration setting now properly supports the
  use of `..` in the path to refer to a parent directory.
  [#10281](https://github.com/rust-lang/cargo/pull/10281)
- Fixed regression in automatic detection of the default number of CPUs to use
  on systems using cgroups v1.
  [#10737](https://github.com/rust-lang/cargo/pull/10737)
  [#10739](https://github.com/rust-lang/cargo/pull/10739)


### Nightly only

- `cargo fetch` now works with `-Zbuild-std` to fetch the standard library's dependencies.
  [#10129](https://github.com/rust-lang/cargo/pull/10129)
- Added support for workspace inheritance.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#workspace-inheritance)
  [#10584](https://github.com/rust-lang/cargo/pull/10584)
  [#10568](https://github.com/rust-lang/cargo/pull/10568)
  [#10565](https://github.com/rust-lang/cargo/pull/10565)
  [#10564](https://github.com/rust-lang/cargo/pull/10564)
  [#10563](https://github.com/rust-lang/cargo/pull/10563)
  [#10606](https://github.com/rust-lang/cargo/pull/10606)
  [#10548](https://github.com/rust-lang/cargo/pull/10548)
  [#10538](https://github.com/rust-lang/cargo/pull/10538)
- Added `-Zcheck-cfg` which adds various forms of validating `cfg` expressions
  for unknown names and values.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#check-cfg)
  [#10486](https://github.com/rust-lang/cargo/pull/10486)
  [#10566](https://github.com/rust-lang/cargo/pull/10566)
- The `--config` CLI option no longer allows setting a registry token.
  [#10580](https://github.com/rust-lang/cargo/pull/10580)
- Fixed issues with proc-macros and `-Z rustdoc-scrape-examples`.
  [#10549](https://github.com/rust-lang/cargo/pull/10549)
  [#10533](https://github.com/rust-lang/cargo/pull/10533)


## Cargo 1.61 (2022-05-19)
[ea2a21c9...rust-1.61.0](https://github.com/rust-lang/cargo/compare/ea2a21c9...rust-1.61.0)

### Added

### Changed

- `cargo test --no-run` will now display the path to the test executables.
  [#10346](https://github.com/rust-lang/cargo/pull/10346)
- `cargo tree --duplicates` no longer reports dependencies that are shared
  between the host and the target as duplicates.
  [#10466](https://github.com/rust-lang/cargo/pull/10466)
- Updated to the 1.4.2 release of libgit2 which brings in several fixes
  [#10442](https://github.com/rust-lang/cargo/pull/10442)
  [#10479](https://github.com/rust-lang/cargo/pull/10479)
- `cargo vendor` no longer allows multiple values for `--sync`, you must pass
  multiple `--sync` flags instead.
  [#10448](https://github.com/rust-lang/cargo/pull/10448)
- Warnings are now issued for manifest keys that have mixed both underscore
  and dash variants (such as specifying both `proc_macro` and `proc-macro`)
  [#10316](https://github.com/rust-lang/cargo/pull/10316)
- Cargo now uses the standard library's `available_parallelism` instead of the
  `num_cpus` crate for determining the default parallelism.
  [#10427](https://github.com/rust-lang/cargo/pull/10427)
- `cargo search` terms are now highlighted.
  [#10425](https://github.com/rust-lang/cargo/pull/10425)

### Fixed

- Paths passed to VCS tools like `hg` are now added after `--` to avoid
  conflict with VCS flags.
  [#10483](https://github.com/rust-lang/cargo/pull/10483)
- Fixed the `http.timeout` configuration value to actually work.
  [#10456](https://github.com/rust-lang/cargo/pull/10456)
- Fixed issues with `cargo rustc --crate-type` not working in some situations.
  [#10388](https://github.com/rust-lang/cargo/pull/10388)

### Nightly only

- Added `-Z check-cfg-features` to enable compile-time checking of features
  [#10408](https://github.com/rust-lang/cargo/pull/10408)
- Added `-Z bindeps` to support binary artifact dependencies (RFC-3028)
  [#9992](https://github.com/rust-lang/cargo/pull/9992)
- `-Z multitarget` is now supported in the `build.target` config value with an array.
  [#10473](https://github.com/rust-lang/cargo/pull/10473)
- Added `--keep-going` flag which will continue compilation even if one crate
  fails to compile.
  [#10383](https://github.com/rust-lang/cargo/pull/10383)
- Start work on inheriting manifest values in a workspace.
  [#10497](https://github.com/rust-lang/cargo/pull/10497)
  [#10517](https://github.com/rust-lang/cargo/pull/10517)
- Added support for sparse HTTP registries.
  [#10470](https://github.com/rust-lang/cargo/pull/10470)
  [#10064](https://github.com/rust-lang/cargo/pull/10064)
- Fixed panic when artifact target is used for `[target.'cfg(<target>)'.dependencies]`
  [#10433](https://github.com/rust-lang/cargo/pull/10433)
- Fixed host flags to pass to build scripts (`-Z target-applies-to-host`)
  [#10395](https://github.com/rust-lang/cargo/pull/10395)
- Added `-Z check-cfg-features` support for rustdoc
  [#10428](https://github.com/rust-lang/cargo/pull/10428)


## Cargo 1.60 (2022-04-07)
[358e79fe...rust-1.60.0](https://github.com/rust-lang/cargo/compare/358e79fe...rust-1.60.0)

### Added

- 🎉 Added the `dep:` prefix in the `[features]` table to refer to an optional
  dependency. This allows creating feature names with the same name as a
  dependency, and allows for "hiding" optional dependencies so that they do
  not implicitly expose a feature name.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/features.html#optional-dependencies)
  [#10269](https://github.com/rust-lang/cargo/pull/10269)
- 🎉 Added the `dep-name?/feature-name` syntax to the `[features]` table to
  only enable the feature `feature-name` if the optional dependency `dep-name`
  is already enabled by some other feature.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/features.html#dependency-features)
  [#10269](https://github.com/rust-lang/cargo/pull/10269)
- 🎉 Added `--timings` option to generate an HTML report about build timing,
  concurrency, and CPU use.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/timings.html)
  [#10245](https://github.com/rust-lang/cargo/pull/10245)
- Added the `"v"` and `"features2"` fields to the registry index.
  The `"v"` field provides a method for compatibility with future changes to the index.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/registries.html#index-format)
  [#10269](https://github.com/rust-lang/cargo/pull/10269)
- Added bash completion for `cargo clippy`
  [#10347](https://github.com/rust-lang/cargo/pull/10347)
- Added bash completion for `cargo report`
  [#10295](https://github.com/rust-lang/cargo/pull/10295)
- Added support to build scripts for `rustc-link-arg-tests`,
  `rustc-link-arg-examples`, and `rustc-link-arg-benches`.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#outputs-of-the-build-script)
  [#10274](https://github.com/rust-lang/cargo/pull/10274)

### Changed

- Cargo now uses the clap 3 library for command-line argument parsing.
  [#10265](https://github.com/rust-lang/cargo/pull/10265)
- The `build.pipelining` config option is now deprecated, pipelining will now
  always be enabled.
  [#10258](https://github.com/rust-lang/cargo/pull/10258)
- `cargo new` will now generate a `.gitignore` which only ignores `Cargo.lock`
  in the root of the repo, instead of any directory.
  [#10379](https://github.com/rust-lang/cargo/pull/10379)
- Improved startup time of bash completion.
  [#10365](https://github.com/rust-lang/cargo/pull/10365)
- The `--features` flag is now honored when used with the `--all-features`
  flag, which allows enabling features from other packages.
  [#10337](https://github.com/rust-lang/cargo/pull/10337)
- Cargo now uses a different TOML parser. This should not introduce any
  user-visible changes. This paves the way to support format-preserving
  programmatic modification of TOML files for supporting `cargo add` and other
  future enhancements.
  [#10086](https://github.com/rust-lang/cargo/pull/10086)
- Setting a library to emit both a `dylib` and `cdylib` is now an error, as
  this combination is not supported.
  [#10243](https://github.com/rust-lang/cargo/pull/10243)
- `cargo --list` now includes the `help` command.
  [#10300](https://github.com/rust-lang/cargo/pull/10300)

### Fixed

- Fixed running `cargo doc` on examples with dev-dependencies.
  [#10341](https://github.com/rust-lang/cargo/pull/10341)
- Fixed `cargo install --path` for a path that is relative to a directory
  outside of the workspace in the current directory.
  [#10335](https://github.com/rust-lang/cargo/pull/10335)
- `cargo test TEST_FILTER` should no longer build binaries that are explicitly
  disabled with `test = false`.
  [#10305](https://github.com/rust-lang/cargo/pull/10305)
- Fixed regression with `term.verbose` without `term.quiet`, and vice versa.
  [#10429](https://github.com/rust-lang/cargo/pull/10429)
  [#10436](https://github.com/rust-lang/cargo/pull/10436)

### Nightly only

- Added `rustflags` option to a profile definition.
  [#10217](https://github.com/rust-lang/cargo/pull/10217)
- Changed `--config` to only support dotted keys.
  [#10176](https://github.com/rust-lang/cargo/pull/10176)
- Fixed profile `rustflags` not being gated in profile overrides.
  [#10411](https://github.com/rust-lang/cargo/pull/10411)
  [#10413](https://github.com/rust-lang/cargo/pull/10413)

## Cargo 1.59 (2022-02-24)
[7f08ace4...rust-1.59.0](https://github.com/rust-lang/cargo/compare/7f08ace4...rust-1.59.0)

### Added

- 🎉 The `strip` option can now be specified in a profile to specify the
  behavior for removing symbols and debug information from binaries.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#strip)
  [#10088](https://github.com/rust-lang/cargo/pull/10088)
  [#10376](https://github.com/rust-lang/cargo/pull/10376)
- 🎉 Added future incompatible reporting.
  This provides reporting for when a future change in `rustc` may cause a
  package or any of its dependencies to stop building.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/future-incompat-report.html)
  [#10165](https://github.com/rust-lang/cargo/pull/10165)
- SSH authentication on Windows now supports ssh-agent.
  [docs](https://doc.rust-lang.org/nightly/cargo/appendix/git-authentication.html#ssh-authentication)
  [#10248](https://github.com/rust-lang/cargo/pull/10248)
- Added `term.quiet` configuration option to enable the `--quiet` behavior
  from a config file.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#termquiet)
  [#10152](https://github.com/rust-lang/cargo/pull/10152)
- Added `-r` CLI option as an alias for `--release`.
  [#10133](https://github.com/rust-lang/cargo/pull/10133)

### Changed

- Scanning the package directory should now be resilient to errors, such as
  filesystem loops or access issues.
  [#10188](https://github.com/rust-lang/cargo/pull/10188)
  [#10214](https://github.com/rust-lang/cargo/pull/10214)
  [#10286](https://github.com/rust-lang/cargo/pull/10286)
- `cargo help <alias>` will now show the target of the alias.
  [#10193](https://github.com/rust-lang/cargo/pull/10193)
- Removed the deprecated `--host` CLI option.
  [#10145](https://github.com/rust-lang/cargo/pull/10145)
  [#10327](https://github.com/rust-lang/cargo/pull/10327)
- Cargo should now report its version to always be in sync with `rustc`.
  [#10178](https://github.com/rust-lang/cargo/pull/10178)
- Added EOPNOTSUPP to ignored file locking errors, which is relevant to BSD
  operating systems.
  [#10157](https://github.com/rust-lang/cargo/pull/10157)

### Fixed

- macOS: Fixed an issue where running an executable would sporadically be
  killed by the kernel (likely starting in macOS 12).
  [#10196](https://github.com/rust-lang/cargo/pull/10196)
- Fixed so that the `doc=false` setting is honored in the `[lib]` definition
  of a dependency.
  [#10201](https://github.com/rust-lang/cargo/pull/10201)
  [#10324](https://github.com/rust-lang/cargo/pull/10324)
- The `"executable"` field in the JSON option was incorrectly including the
  path to `index.html` when documenting a binary. It is now null.
  [#10171](https://github.com/rust-lang/cargo/pull/10171)
- Documenting a binary now waits for the package library to finish documenting
  before starting. This fixes some race conditions if the binary has intra-doc
  links to the library.
  [#10172](https://github.com/rust-lang/cargo/pull/10172)
- Fixed panic when displaying help text to a closed pipe.
  [#10164](https://github.com/rust-lang/cargo/pull/10164)

### Nightly only
- Added the `--crate-type` flag to `cargo rustc`.
  [#10093](https://github.com/rust-lang/cargo/pull/10093)


## Cargo 1.58 (2022-01-13)
[b2e52d7c...rust-1.58.0](https://github.com/rust-lang/cargo/compare/b2e52d7c...rust-1.58.0)

### Added

- Added `rust_version` field to package data in `cargo metadata`.
  [#9967](https://github.com/rust-lang/cargo/pull/9967)
- Added `--message-format` option to `cargo install`.
  [#10107](https://github.com/rust-lang/cargo/pull/10107)

### Changed

- A warning is now shown when an alias shadows an external command.
  [#10082](https://github.com/rust-lang/cargo/pull/10082)
- Updated curl to 7.80.0.
  [#10040](https://github.com/rust-lang/cargo/pull/10040)
  [#10106](https://github.com/rust-lang/cargo/pull/10106)

### Fixed

- Doctests now include rustc-link-args from build scripts.
  [#9916](https://github.com/rust-lang/cargo/pull/9916)
- Fixed `cargo tree` entering an infinite loop with cyclical dev-dependencies.
  Fixed an edge case where the resolver would fail to handle a cyclical dev-dependency with a feature.
  [#10103](https://github.com/rust-lang/cargo/pull/10103)
- Fixed `cargo clean -p` when the directory path contains glob characters.
  [#10072](https://github.com/rust-lang/cargo/pull/10072)
- Fixed debug builds of `cargo` which could panic when downloading a crate
  when the server has a redirect with a non-empty body.
  [#10048](https://github.com/rust-lang/cargo/pull/10048)

### Nightly only

- Make future-incompat-report output more user-friendly.
  [#9953](https://github.com/rust-lang/cargo/pull/9953)
- Added support to scrape code examples from the `examples` directory to be included in the documentation.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#scrape-examples)
  [#9525](https://github.com/rust-lang/cargo/pull/9525)
  [#10037](https://github.com/rust-lang/cargo/pull/10037)
  [#10017](https://github.com/rust-lang/cargo/pull/10017)
- Fixed `cargo report future-incompatibilities` to check stdout if it supports color.
  [#10024](https://github.com/rust-lang/cargo/pull/10024)

## Cargo 1.57 (2021-12-02)
[18751dd3...rust-1.57.0](https://github.com/rust-lang/cargo/compare/18751dd3...rust-1.57.0)

### Added

- 🎉 Added custom named profiles. This also changes the `test` and `bench`
  profiles to inherit their settings from `dev` and `release`, and Cargo will
  now only use a single profile during a given command instead of using
  different profiles for dependencies and cargo-targets.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#custom-profiles)
  [#9943](https://github.com/rust-lang/cargo/pull/9943)
- The `rev` option for a git dependency now supports git references that start
  with `refs/`. An example where this can be used is to depend on a pull
  request from a service like GitHub before it is merged.
  [#9859](https://github.com/rust-lang/cargo/pull/9859)
- Added `path_in_vcs` field to the `.cargo_vcs_info.json` file.
  [docs](https://doc.rust-lang.org/nightly/cargo/commands/cargo-package.html#cargo_vcs_infojson-format)
  [#9866](https://github.com/rust-lang/cargo/pull/9866)

### Changed

- ❗ `RUSTFLAGS` is no longer set for build scripts. This change was made in
  1.55, but the release notes did not highlight this change. Build scripts
  should use `CARGO_ENCODED_RUSTFLAGS` instead. See the
  [documentation](https://doc.rust-lang.org/nightly/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts)
  for more details.
- The `cargo version` command now includes some extra information.
  [#9968](https://github.com/rust-lang/cargo/pull/9968)
- Updated libgit2 to 1.3 which brings in a number of fixes and changes to git
  handling.
  [#9963](https://github.com/rust-lang/cargo/pull/9963)
  [#9988](https://github.com/rust-lang/cargo/pull/9988)
- Shell completions now include shorthand b/r/c/d subcommands.
  [#9951](https://github.com/rust-lang/cargo/pull/9951)
- `cargo update --precise` now allows specifying a version without semver
  metadata (stuff after `+` in the version number).
  [#9945](https://github.com/rust-lang/cargo/pull/9945)
- zsh completions now complete `--example` names.
  [#9939](https://github.com/rust-lang/cargo/pull/9939)
- The progress bar now differentiates when building unittests.
  [#9934](https://github.com/rust-lang/cargo/pull/9934)
- Some backwards-compatibility support for invalid TOML syntax has been removed.
  [#9932](https://github.com/rust-lang/cargo/pull/9932)
- Reverted the change from 1.55 that triggered an error for dependency
  specifications that did not include any fields.
  [#9911](https://github.com/rust-lang/cargo/pull/9911)

### Fixed

- Removed a log message (from `CARGO_LOG`) that may leak tokens.
  [#9873](https://github.com/rust-lang/cargo/pull/9873)
- `cargo fix` will now avoid writing fixes to the global registry cache.
  [#9938](https://github.com/rust-lang/cargo/pull/9938)
- Fixed `-Z help` CLI option when used with a shorthand alias (b/c/r/d).
  [#9933](https://github.com/rust-lang/cargo/pull/9933)


### Nightly only


## Cargo 1.56 (2021-10-21)
[cebef295...rust-1.56.0](https://github.com/rust-lang/cargo/compare/cebef295...rust-1.56.0)

### Added

- 🎉 Cargo now supports the 2021 edition.
  More information may be found in the [edition
  guide](https://doc.rust-lang.org/nightly/edition-guide/rust-2021/index.html).
  [#9800](https://github.com/rust-lang/cargo/pull/9800)
- 🎉 Added the
  [`rust-version`](https://doc.rust-lang.org/nightly/cargo/reference/manifest.html#the-rust-version-field)
  field to `Cargo.toml` to specify the minimum supported Rust version, and the
  `--ignore-rust-version` command line option to override it.
  [#9732](https://github.com/rust-lang/cargo/pull/9732)
- Added the `[env]` table to config files to specify environment variables to
  set.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#env)
  [#9411](https://github.com/rust-lang/cargo/pull/9411)
- `[patch]` tables may now be specified in config files.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#patch)
  [#9839](https://github.com/rust-lang/cargo/pull/9839)
- `cargo doc` now supports the `--example` and `--examples` flags.
  [#9808](https://github.com/rust-lang/cargo/pull/9808)
- 🎉 Build scripts can now pass additional linker arguments for binaries or all
  linkable targets. [docs](https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#outputs-of-the-build-script)
  [#9557](https://github.com/rust-lang/cargo/pull/9557)
- Added support for the `-p` flag for `cargo publish` to publish a specific
  package in a workspace. `cargo package` also now supports `-p` and
  `--workspace`.
  [#9559](https://github.com/rust-lang/cargo/pull/9559)
- Added documentation about third-party registries.
  [#9830](https://github.com/rust-lang/cargo/pull/9830)
- Added the `{sha256-checksum}` placeholder for URLs in a registry `config.json`.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/registries.html#index-format)
  [#9801](https://github.com/rust-lang/cargo/pull/9801)
- Added a warning when a dependency does not have a library.
  [#9771](https://github.com/rust-lang/cargo/pull/9771)

### Changed

- Doc tests now support the `-q` flag to show terse test output.
  [#9730](https://github.com/rust-lang/cargo/pull/9730)
- `features` used in a `[replace]` table now issues a warning, as they are ignored.
  [#9681](https://github.com/rust-lang/cargo/pull/9681)
- Changed so that only `wasm32-unknown-emscripten` executables are built
  without a hash in the filename. Previously it was all `wasm32` targets.
  Additionally, all `apple` binaries are now built with a hash in the
  filename. This allows multiple copies to be cached at once, and matches the
  behavior on other platforms (except `msvc`).
  [#9653](https://github.com/rust-lang/cargo/pull/9653)
- `cargo new` now generates an example that doesn't generate a warning with
  clippy.
  [#9796](https://github.com/rust-lang/cargo/pull/9796)
- `cargo fix --edition` now only applies edition-specific lints.
  [#9846](https://github.com/rust-lang/cargo/pull/9846)
- Improve resolver message to include dependency requirements.
  [#9827](https://github.com/rust-lang/cargo/pull/9827)
- `cargo fix` now has more debug logging available with the `CARGO_LOG`
  environment variable.
  [#9831](https://github.com/rust-lang/cargo/pull/9831)
- Changed `cargo fix --edition` to emit a warning when on the latest stable
  edition when running on stable instead of generating an error.
  [#9792](https://github.com/rust-lang/cargo/pull/9792)
- `cargo install` will now determine all of the packages to install before
  starting the installation, which should help with reporting errors without
  partially installing.
  [#9793](https://github.com/rust-lang/cargo/pull/9793)
- The resolver report for `cargo fix --edition` now includes differences for
  dev-dependencies.
  [#9803](https://github.com/rust-lang/cargo/pull/9803)
- `cargo fix` will now show better diagnostics for abnormal errors from `rustc`.
  [#9799](https://github.com/rust-lang/cargo/pull/9799)
- Entries in `cargo --list` are now deduplicated.
  [#9773](https://github.com/rust-lang/cargo/pull/9773)
- Aliases are now included in `cargo --list`.
  [#9764](https://github.com/rust-lang/cargo/pull/9764)

### Fixed

- Fixed panic with build-std of a proc-macro.
  [#9834](https://github.com/rust-lang/cargo/pull/9834)
- Fixed running `cargo` recursively from proc-macros while running `cargo fix`.
  [#9818](https://github.com/rust-lang/cargo/pull/9818)
- Return an error instead of a stack overflow for command alias loops.
  [#9791](https://github.com/rust-lang/cargo/pull/9791)
- Updated to curl 7.79.1, which will hopefully fix intermittent http2 errors.
  [#9937](https://github.com/rust-lang/cargo/pull/9937)

### Nightly only

- Added `[future-incompat-report]` config section.
  [#9774](https://github.com/rust-lang/cargo/pull/9774)
- Fixed value-after-table error with custom named profiles.
  [#9789](https://github.com/rust-lang/cargo/pull/9789)
- Added the `different-binary-name` feature to support specifying a
  non-rust-identifier for a binary name.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#different-binary-name)
  [#9627](https://github.com/rust-lang/cargo/pull/9627)
- Added a profile option to select the codegen backend.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#codegen-backend)
  [#9118](https://github.com/rust-lang/cargo/pull/9118)


## Cargo 1.55 (2021-09-09)
[aa8b0929...rust-1.55.0](https://github.com/rust-lang/cargo/compare/aa8b0929...rust-1.55.0)

### Added

- The package definition in `cargo metadata` now includes the `"default_run"`
  field from the manifest.
  [#9550](https://github.com/rust-lang/cargo/pull/9550)
- ❗ Build scripts now have access to the following environment variables:
  `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_ENCODED_RUSTFLAGS`.
  `RUSTFLAGS` is no longer set for build scripts; they should use
  `CARGO_ENCODED_RUSTFLAGS` instead.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts)
  [#9601](https://github.com/rust-lang/cargo/pull/9601)
- Added `cargo d` as an alias for `cargo doc`.
  [#9680](https://github.com/rust-lang/cargo/pull/9680)
- Added `{lib}` to the `cargo tree --format` option to display the library
  name of a package.
  [#9663](https://github.com/rust-lang/cargo/pull/9663)
- Added `members_mut` method to the `Workspace` API.
  [#9547](https://github.com/rust-lang/cargo/pull/9547)

### Changed

- If a build command does not match any targets when using the
  `--all-targets`, `--bins`, `--tests`, `--examples`, or `--benches` flags, a
  warning is now displayed to inform you that there were no matching targets.
  [#9549](https://github.com/rust-lang/cargo/pull/9549)
- The way `cargo init` detects whether or not existing source files represent
  a binary or library has been changed to respect the command-line flags
  instead of trying to guess which type it is.
  [#9522](https://github.com/rust-lang/cargo/pull/9522)
- Registry names are now displayed instead of registry URLs when possible.
  [#9632](https://github.com/rust-lang/cargo/pull/9632)
- Duplicate compiler diagnostics are no longer shown. This can often happen
  with `cargo test` which builds multiple copies of the same code in parallel.
  This also updates the warning summary to provide more context.
  [#9675](https://github.com/rust-lang/cargo/pull/9675)
- The output for warnings or errors is now improved to be leaner, cleaner, and
  show more context.
  [#9655](https://github.com/rust-lang/cargo/pull/9655)
- Network send errors are now treated as "spurious" which means they will be retried.
  [#9695](https://github.com/rust-lang/cargo/pull/9695)
- Git keys (`branch`, `tag`, `rev`) on a non-git dependency are now an error.
  Additionally, specifying both `git` and `path` is now an error.
  [#9689](https://github.com/rust-lang/cargo/pull/9689)
- Specifying a dependency without any keys is now an error.
  [#9686](https://github.com/rust-lang/cargo/pull/9686)
- The resolver now prefers to use `[patch]` table entries of dependencies when
  possible.
  [#9639](https://github.com/rust-lang/cargo/pull/9639)
- Package name typo errors in dependencies are now displayed aligned with the
  original to help make it easier to see the difference.
  [#9665](https://github.com/rust-lang/cargo/pull/9665)
- Windows platforms may now warn on environment variables that have the wrong case.
  [#9654](https://github.com/rust-lang/cargo/pull/9654)
- `features` used in a `[patch]` table now issues a warning, as they are ignored.
  [#9666](https://github.com/rust-lang/cargo/pull/9666)
- The `target` directory is now excluded from content indexing on Windows.
  [#9635](https://github.com/rust-lang/cargo/pull/9635)
- When `Cargo.toml` is not found, the error message now detects if it was
  misnamed with a lowercase `c` to suggest the correct form.
  [#9607](https://github.com/rust-lang/cargo/pull/9607)
- Building `diesel` with the new resolver displays a compatibility notice.
  [#9602](https://github.com/rust-lang/cargo/pull/9602)
- Updated the `opener` dependency, which handles opening a web browser, which
  includes several changes, such as new behavior when run on WSL, and using
  the system `xdg-open` on Linux.
  [#9583](https://github.com/rust-lang/cargo/pull/9583)
- Updated to libcurl 7.78.
  [#9809](https://github.com/rust-lang/cargo/pull/9809)
  [#9810](https://github.com/rust-lang/cargo/pull/9810)

### Fixed

- Fixed dep-info files including non-local build script paths.
  [#9596](https://github.com/rust-lang/cargo/pull/9596)
- Handle "jobs = 0" case in cargo config files
  [#9584](https://github.com/rust-lang/cargo/pull/9584)
- Implement warning for ignored trailing arguments after `--`
  [#9561](https://github.com/rust-lang/cargo/pull/9561)
- Fixed rustc/rustdoc config values to be config-relative.
  [#9566](https://github.com/rust-lang/cargo/pull/9566)
- `cargo fix` now supports rustc's suggestions with multiple spans.
  [#9567](https://github.com/rust-lang/cargo/pull/9567)
- `cargo fix` now fixes each target serially instead of in parallel to avoid
  problems with fixing the same file concurrently.
  [#9677](https://github.com/rust-lang/cargo/pull/9677)
- Changes to the target `linker` config value now trigger a rebuild.
  [#9647](https://github.com/rust-lang/cargo/pull/9647)
- Git unstaged deleted files are now ignored when using the `--allow-dirty`
  flag with `cargo publish` or `cargo package`.
  [#9645](https://github.com/rust-lang/cargo/pull/9645)

### Nightly only

- Enabled support for `cargo fix --edition` for 2021.
  [#9588](https://github.com/rust-lang/cargo/pull/9588)
- Several changes to named profiles.
  [#9685](https://github.com/rust-lang/cargo/pull/9685)
- Extended instructions on what to do when running `cargo fix --edition` on
  the 2021 edition.
  [#9694](https://github.com/rust-lang/cargo/pull/9694)
- Multiple updates to error messages using nightly features to help better
  explain the situation.
  [#9657](https://github.com/rust-lang/cargo/pull/9657)
- Adjusted the edition 2021 resolver diff report.
  [#9649](https://github.com/rust-lang/cargo/pull/9649)
- Fixed error using `cargo doc --open` with `doc.extern-map`.
  [#9531](https://github.com/rust-lang/cargo/pull/9531)
- Unified weak and namespaced features.
  [#9574](https://github.com/rust-lang/cargo/pull/9574)
- Various updates to future-incompatible reporting.
  [#9606](https://github.com/rust-lang/cargo/pull/9606)
- `[env]` environment variables are not allowed to set vars set by Cargo.
  [#9579](https://github.com/rust-lang/cargo/pull/9579)

## Cargo 1.54 (2021-07-29)
[4369396c...rust-1.54.0](https://github.com/rust-lang/cargo/compare/4369396c...rust-1.54.0)

### Added

- Fetching from a git repository (such as the crates.io index) now displays
  the network transfer rate.
  [#9395](https://github.com/rust-lang/cargo/pull/9395)
- Added `--prune` option for `cargo tree` to limit what is displayed.
  [#9520](https://github.com/rust-lang/cargo/pull/9520)
- Added `--depth` option for `cargo tree` to limit what is displayed.
  [#9499](https://github.com/rust-lang/cargo/pull/9499)
- Added `cargo tree -e no-proc-macro` to hide procedural macro dependencies.
  [#9488](https://github.com/rust-lang/cargo/pull/9488)
- Added `doc.browser` config option to set which browser to open with `cargo doc --open`.
  [#9473](https://github.com/rust-lang/cargo/pull/9473)
- Added `CARGO_TARGET_TMPDIR` environment variable set for integration tests &
  benches. This provides a temporary or "scratch" directory in the `target`
  directory for tests and benches to use.
  [#9375](https://github.com/rust-lang/cargo/pull/9375)

### Changed

- `--features` CLI flags now provide typo suggestions with the new feature resolver.
  [#9420](https://github.com/rust-lang/cargo/pull/9420)
- Cargo now uses a new parser for SemVer versions. This should behave mostly
  the same as before with some minor exceptions where invalid syntax for
  version requirements is now rejected.
  [#9508](https://github.com/rust-lang/cargo/pull/9508)
- Mtime handling of `.crate` published packages has changed slightly to avoid
  mtime values of 0. This was causing problems with lldb which refused to read
  those files.
  [#9517](https://github.com/rust-lang/cargo/pull/9517)
- Improved performance of git status check in `cargo package`.
  [#9478](https://github.com/rust-lang/cargo/pull/9478)
- `cargo new` with fossil now places the ignore settings in the new repository
  instead of using `fossil settings` to set them globally. This also includes
  several other cleanups to make it more consistent with other VCS
  configurations.
  [#9469](https://github.com/rust-lang/cargo/pull/9469)
- `rustc-cdylib-link-arg` applying transitively displays a warning that this
  was not intended, and may be an error in the future.
  [#9563](https://github.com/rust-lang/cargo/pull/9563)

### Fixed

- Fixed `package.exclude` in `Cargo.toml` using inverted exclusions
  (`!somefile`) when not in a git repository or when vendoring a dependency.
  [#9186](https://github.com/rust-lang/cargo/pull/9186)
- Dep-info files now adjust build script `rerun-if-changed` paths to be
  absolute paths.
  [#9421](https://github.com/rust-lang/cargo/pull/9421)
- Fixed a bug when with resolver = "1" non-virtual package was allowing
  unknown features.
  [#9437](https://github.com/rust-lang/cargo/pull/9437)
- Fixed an issue with the index cache mishandling versions that only
  differed in build metadata (such as `110.0.0` and `110.0.0+1.1.0f`).
  [#9476](https://github.com/rust-lang/cargo/pull/9476)
- Fixed `cargo install` with a semver metadata version.
  [#9467](https://github.com/rust-lang/cargo/pull/9467)

### Nightly only

- Added `report` subcommand, and changed `cargo
  describe-future-incompatibilitie` to `cargo report
  future-incompatibilities`.
  [#9438](https://github.com/rust-lang/cargo/pull/9438)
- Added a `[host]` table to the config files to be able to set build flags for
  host target. Also added `target-applies-to-host` to control how the
  `[target]` tables behave.
  [#9322](https://github.com/rust-lang/cargo/pull/9322)
- Added some validation to build script `rustc-link-arg-*` instructions to
  return an error if the target doesn't exist.
  [#9523](https://github.com/rust-lang/cargo/pull/9523)
- Added `cargo:rustc-link-arg-bin` instruction for build scripts.
  [#9486](https://github.com/rust-lang/cargo/pull/9486)


## Cargo 1.53 (2021-06-17)
[90691f2b...rust-1.53.0](https://github.com/rust-lang/cargo/compare/90691f2b...rust-1.53.0)

### Added

### Changed
- 🔥 Cargo now supports git repositories where the default `HEAD` branch is not
  "master". This also includes a switch to the version 3 `Cargo.lock` format
  which can handle default branches correctly.
  [#9133](https://github.com/rust-lang/cargo/pull/9133)
  [#9397](https://github.com/rust-lang/cargo/pull/9397)
  [#9384](https://github.com/rust-lang/cargo/pull/9384)
  [#9392](https://github.com/rust-lang/cargo/pull/9392)
- 🔥 macOS targets now default to `unpacked` split-debuginfo.
  [#9298](https://github.com/rust-lang/cargo/pull/9298)
- ❗ The `authors` field is no longer included in `Cargo.toml` for new
  projects.
  [#9282](https://github.com/rust-lang/cargo/pull/9282)
- `cargo update` may now work with the `--offline` flag.
  [#9279](https://github.com/rust-lang/cargo/pull/9279)
- `cargo doc` will now erase the `doc` directory when switching between
  different toolchain versions. There are shared, unversioned files (such as
  the search index) that can become broken when using different versions.
  [#8640](https://github.com/rust-lang/cargo/pull/8640)
  [#9404](https://github.com/rust-lang/cargo/pull/9404)
- Improved error messages when path dependency/workspace member is missing.
  [#9368](https://github.com/rust-lang/cargo/pull/9368)

### Fixed
- Fixed `cargo doc` detecting if the documentation needs to be rebuilt when
  changing some settings such as features.
  [#9419](https://github.com/rust-lang/cargo/pull/9419)
- `cargo doc` now deletes the output directory for the package before running
  rustdoc to clear out any stale files.
  [#9419](https://github.com/rust-lang/cargo/pull/9419)
- Fixed the `-C metadata` value to always include all information for all
  builds. Previously, in some situations, the hash only included the package
  name and version. This fixes some issues, such as incremental builds with
  split-debuginfo on macOS corrupting the incremental cache in some cases.
  [#9418](https://github.com/rust-lang/cargo/pull/9418)
- Fixed man pages not working on Windows if `man` is in `PATH`.
  [#9378](https://github.com/rust-lang/cargo/pull/9378)
- The `rustc` cache is now aware of `RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER`.
  [#9348](https://github.com/rust-lang/cargo/pull/9348)
- Track the `CARGO` environment variable in the rebuild fingerprint if the
  code uses `env!("CARGO")`.
  [#9363](https://github.com/rust-lang/cargo/pull/9363)

### Nightly only
- Fixed config includes not working.
  [#9299](https://github.com/rust-lang/cargo/pull/9299)
- Emit note when `--future-incompat-report` had nothing to report.
  [#9263](https://github.com/rust-lang/cargo/pull/9263)
- Error messages for nightly features flags (like `-Z` and `cargo-features`)
  now provides more information.
  [#9290](https://github.com/rust-lang/cargo/pull/9290)
- Added the ability to set the target for an individual package in `Cargo.toml`.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#per-package-target)
  [#9030](https://github.com/rust-lang/cargo/pull/9030)
- Fixed build-std updating the index on every build.
  [#9393](https://github.com/rust-lang/cargo/pull/9393)
- `-Z help` now displays all the `-Z` options.
  [#9369](https://github.com/rust-lang/cargo/pull/9369)
- Added `-Zallow-features` to specify which nightly features are allowed to be used.
  [#9283](https://github.com/rust-lang/cargo/pull/9283)
- Added `cargo config` subcommand.
  [#9302](https://github.com/rust-lang/cargo/pull/9302)

## Cargo 1.52 (2021-05-06)
[34170fcd...rust-1.52.0](https://github.com/rust-lang/cargo/compare/34170fcd...rust-1.52.0)

### Added
- Added the `"manifest_path"` field to JSON messages for a package.
  [#9022](https://github.com/rust-lang/cargo/pull/9022)
  [#9247](https://github.com/rust-lang/cargo/pull/9247)

### Changed
- Build scripts are now forbidden from setting `RUSTC_BOOTSTRAP` on stable.
  [#9181](https://github.com/rust-lang/cargo/pull/9181)
  [#9385](https://github.com/rust-lang/cargo/pull/9385)
- crates.io now supports SPDX 3.11 licenses.
  [#9209](https://github.com/rust-lang/cargo/pull/9209)
- An error is now reported if `CARGO_TARGET_DIR` is an empty string.
  [#8939](https://github.com/rust-lang/cargo/pull/8939)
- Doc tests now pass the `--message-format` flag into the test so that the
  "short" format can now be used for doc tests.
  [#9128](https://github.com/rust-lang/cargo/pull/9128)
- `cargo test` now prints a clearer indicator of which target is currently running.
  [#9195](https://github.com/rust-lang/cargo/pull/9195)
- The `CARGO_TARGET_<TRIPLE>` environment variable will now issue a warning if
  it is using lowercase letters.
  [#9169](https://github.com/rust-lang/cargo/pull/9169)

### Fixed
- Fixed publication of packages with metadata and resolver fields in `Cargo.toml`.
  [#9300](https://github.com/rust-lang/cargo/pull/9300)
  [#9304](https://github.com/rust-lang/cargo/pull/9304)
- Fixed logic for determining prefer-dynamic for a dylib which differed in a
  workspace vs a single package.
  [#9252](https://github.com/rust-lang/cargo/pull/9252)
- Fixed an issue where exclusive target-specific dependencies that overlapped
  across dependency kinds (like regular and build-dependencies) would
  incorrectly include the dependencies in both.
  [#9255](https://github.com/rust-lang/cargo/pull/9255)
- Fixed panic with certain styles of Package IDs when passed to the `-p` flag.
  [#9188](https://github.com/rust-lang/cargo/pull/9188)
- When running cargo with output not going to a TTY, and with the progress bar
  and color force-enabled, the output will now correctly clear the progress
  line.
  [#9231](https://github.com/rust-lang/cargo/pull/9231)
- Error instead of panic when JSON may contain non-utf8 paths.
  [#9226](https://github.com/rust-lang/cargo/pull/9226)
- Fixed a hang that can happen on broken stderr.
  [#9201](https://github.com/rust-lang/cargo/pull/9201)
- Fixed thin-local LTO not being disabled correctly when `lto=off` is set.
  [#9182](https://github.com/rust-lang/cargo/pull/9182)

### Nightly only
- The `strip` profile option now supports `true` and `false` values.
  [#9153](https://github.com/rust-lang/cargo/pull/9153)
- `cargo fix --edition` now displays a report when switching to 2021 if the
  new resolver changes features.
  [#9268](https://github.com/rust-lang/cargo/pull/9268)
- Added `[patch]` table support in `.cargo/config` files.
  [#9204](https://github.com/rust-lang/cargo/pull/9204)
- Added `cargo describe-future-incompatibilities` for generating a report on
  dependencies that contain future-incompatible warnings.
  [#8825](https://github.com/rust-lang/cargo/pull/8825)
- Added easier support for testing the 2021 edition.
  [#9184](https://github.com/rust-lang/cargo/pull/9184)
- Switch the default resolver to "2" in the 2021 edition.
  [#9184](https://github.com/rust-lang/cargo/pull/9184)
- `cargo fix --edition` now supports 2021.
  [#9184](https://github.com/rust-lang/cargo/pull/9184)
- Added `--print` flag to `cargo rustc` to pass along to `rustc` to display
  information from rustc.
  [#9002](https://github.com/rust-lang/cargo/pull/9002)
- Added `-Zdoctest-in-workspace` for changing the directory where doctests are
  *run* versus where they are *compiled*.
  [#9105](https://github.com/rust-lang/cargo/pull/9105)
- Added support for an `[env]` section in `.cargo/config.toml` to set
  environment variables when running cargo.
  [#9175](https://github.com/rust-lang/cargo/pull/9175)
- Added a schema field and `features2` field to the index.
  [#9161](https://github.com/rust-lang/cargo/pull/9161)
- Changes to JSON spec targets will now trigger a rebuild.
  [#9223](https://github.com/rust-lang/cargo/pull/9223)

## Cargo 1.51 (2021-03-25)
[75d5d8cf...rust-1.51.0](https://github.com/rust-lang/cargo/compare/75d5d8cf...rust-1.51.0)

### Added
- 🔥 Added the `split-debuginfo` profile option.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#split-debuginfo)
  [#9112](https://github.com/rust-lang/cargo/pull/9112)
- Added the `path` field to `cargo metadata` for the package dependencies list
  to show the path for "path" dependencies.
  [#8994](https://github.com/rust-lang/cargo/pull/8994)
- 🔥 Added a new feature resolver, and new CLI feature flag behavior. See the
  new [features](https://doc.rust-lang.org/nightly/cargo/reference/features.html#feature-resolver-version-2)
  and [resolver](https://doc.rust-lang.org/nightly/cargo/reference/resolver.html#feature-resolver-version-2)
  documentation for the `resolver = "2"` option. See the
  [CLI](https://doc.rust-lang.org/nightly/cargo/reference/features.html#command-line-feature-options)
  and [resolver 2 CLI](https://doc.rust-lang.org/nightly/cargo/reference/features.html#resolver-version-2-command-line-flags)
  options for the new CLI behavior. And, finally, see
  [RFC 2957](https://github.com/rust-lang/rfcs/blob/master/text/2957-cargo-features2.md)
  for a detailed look at what has changed.
  [#8997](https://github.com/rust-lang/cargo/pull/8997)

### Changed
- `cargo install --locked` now emits a warning if `Cargo.lock` is not found.
  [#9108](https://github.com/rust-lang/cargo/pull/9108)
- Unknown or ambiguous package IDs passed on the command-line now display
  suggestions for the correct package ID.
  [#9095](https://github.com/rust-lang/cargo/pull/9095)
- Slightly optimize `cargo vendor`
  [#8937](https://github.com/rust-lang/cargo/pull/8937)
  [#9131](https://github.com/rust-lang/cargo/pull/9131)
  [#9132](https://github.com/rust-lang/cargo/pull/9132)

### Fixed
- Fixed environment variables and cfg settings emitted by a build script that
  are set for `cargo test` and `cargo run` when the build script runs multiple
  times during the same build session.
  [#9122](https://github.com/rust-lang/cargo/pull/9122)
- Fixed a panic with `cargo doc` and the new feature resolver. This also
  introduces some heuristics to try to avoid path collisions with `rustdoc` by
  only documenting one variant of a package if there are multiple (such as
  multiple versions, or the same package shared for host and target
  platforms).
  [#9077](https://github.com/rust-lang/cargo/pull/9077)
- Fixed a bug in Cargo's cyclic dep graph detection that caused a stack
  overflow.
  [#9075](https://github.com/rust-lang/cargo/pull/9075)
- Fixed build script `links` environment variables (`DEP_*`) not showing up
  for testing packages in some cases.
  [#9065](https://github.com/rust-lang/cargo/pull/9065)
- Fixed features being selected in a nondeterministic way for a specific
  scenario when building an entire workspace with all targets with a
  proc-macro in the workspace with `resolver="2"`.
  [#9059](https://github.com/rust-lang/cargo/pull/9059)
- Fixed to use `http.proxy` setting in `~/.gitconfig`.
  [#8986](https://github.com/rust-lang/cargo/pull/8986)
- Fixed --feature pkg/feat for V1 resolver for non-member.
  [#9275](https://github.com/rust-lang/cargo/pull/9275)
  [#9277](https://github.com/rust-lang/cargo/pull/9277)
- Fixed panic in `cargo doc` when there are colliding output filenames in a workspace.
  [#9276](https://github.com/rust-lang/cargo/pull/9276)
  [#9277](https://github.com/rust-lang/cargo/pull/9277)
- Fixed `cargo install` from exiting with success if one of several packages
  did not install successfully.
  [#9185](https://github.com/rust-lang/cargo/pull/9185)
  [#9196](https://github.com/rust-lang/cargo/pull/9196)
- Fix panic with doc collision orphan.
  [#9142](https://github.com/rust-lang/cargo/pull/9142)
  [#9196](https://github.com/rust-lang/cargo/pull/9196)

### Nightly only
- Removed the `publish-lockfile` unstable feature, it was stabilized without
  the need for an explicit flag 1.5 years ago.
  [#9092](https://github.com/rust-lang/cargo/pull/9092)
- Added better diagnostics, help messages, and documentation for nightly
  features (such as those passed with the `-Z` flag, or specified with
  `cargo-features` in `Cargo.toml`).
  [#9092](https://github.com/rust-lang/cargo/pull/9092)
- Added support for Rust edition 2021.
  [#8922](https://github.com/rust-lang/cargo/pull/8922)
- Added support for the `rust-version` field in project metadata.
  [#8037](https://github.com/rust-lang/cargo/pull/8037)
- Added a schema field to the index.
  [#9161](https://github.com/rust-lang/cargo/pull/9161)
  [#9196](https://github.com/rust-lang/cargo/pull/9196)

## Cargo 1.50 (2021-02-11)
[8662ab42...rust-1.50.0](https://github.com/rust-lang/cargo/compare/8662ab42...rust-1.50.0)

### Added
- Added the `doc` field to `cargo metadata`, which indicates if a target is
  documented.
  [#8869](https://github.com/rust-lang/cargo/pull/8869)
- Added `RUSTC_WORKSPACE_WRAPPER`, an alternate RUSTC wrapper that only runs
  for the local workspace packages, and caches its artifacts independently of
  non-wrapped builds.
  [#8976](https://github.com/rust-lang/cargo/pull/8976)
- Added `--workspace` to `cargo update` to update only the workspace members,
  and not their dependencies. This is particularly useful if you update the
  version in `Cargo.toml` and want to update `Cargo.lock` without running any
  other commands.
  [#8725](https://github.com/rust-lang/cargo/pull/8725)

### Changed
- `.crate` files uploaded to a registry are now built with reproducible
  settings, so that the same `.crate` file created on different machines
  should be identical.
  [#8864](https://github.com/rust-lang/cargo/pull/8864)
- Git dependencies that specify more than one of `branch`, `tag`, or `rev` are
  now rejected.
  [#8984](https://github.com/rust-lang/cargo/pull/8984)
- The `rerun-if-changed` build script directive can now point to a directory,
  in which case Cargo will check if any file in that directory changes.
  [#8973](https://github.com/rust-lang/cargo/pull/8973)
- If Cargo cannot determine the username or email address, `cargo new` will no
  longer fail, and instead create an empty authors list.
  [#8912](https://github.com/rust-lang/cargo/pull/8912)
- The progress bar width has been reduced to provide more room to display the
  crates currently being built.
  [#8892](https://github.com/rust-lang/cargo/pull/8892)
- `cargo new` will now support `includeIf` directives in `.gitconfig` to match
  the correct directory when determining the username and email address.
  [#8886](https://github.com/rust-lang/cargo/pull/8886)

### Fixed
- Fixed `cargo metadata` and `cargo tree` to only download packages for the
  requested target.
  [#8987](https://github.com/rust-lang/cargo/pull/8987)
- Updated libgit2, which brings in many fixes, particularly fixing a zlib
  error that occasionally appeared on 32-bit systems.
  [#8998](https://github.com/rust-lang/cargo/pull/8998)
- Fixed stack overflow with a circular dev-dependency that uses the `links`
  field.
  [#8969](https://github.com/rust-lang/cargo/pull/8969)
- Fixed `cargo publish` failing on some filesystems, particularly 9p on WSL2.
  [#8950](https://github.com/rust-lang/cargo/pull/8950)

### Nightly only
- Allow `resolver="1"` to specify the original feature resolution behavior.
  [#8857](https://github.com/rust-lang/cargo/pull/8857)
- Added `-Z extra-link-arg` which adds the `cargo:rustc-link-arg-bins`
  and `cargo:rustc-link-arg` build script options.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#extra-link-arg)
  [#8441](https://github.com/rust-lang/cargo/pull/8441)
- Implemented external credential process support, and added `cargo logout`.
  ([RFC 2730](https://github.com/rust-lang/rfcs/blob/master/text/2730-cargo-token-from-process.md))
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#credential-process))
  [#8934](https://github.com/rust-lang/cargo/pull/8934)
- Fix panic with `-Zbuild-std` and no roots.
  [#8942](https://github.com/rust-lang/cargo/pull/8942)
- Set docs.rs as the default extern-map for crates.io
  [#8877](https://github.com/rust-lang/cargo/pull/8877)

## Cargo 1.49 (2020-12-31)
[75615f8e...rust-1.49.0](https://github.com/rust-lang/cargo/compare/75615f8e...rust-1.49.0)

### Added
- Added `homepage` and `documentation` fields to `cargo metadata`.
  [#8744](https://github.com/rust-lang/cargo/pull/8744)
- Added the `CARGO_PRIMARY_PACKAGE` environment variable which is set when
  running `rustc` if the package is one of the "root" packages selected on the
  command line.
  [#8758](https://github.com/rust-lang/cargo/pull/8758)
- Added support for Unix-style glob patterns for package and target selection
  flags on the command-line (such as `-p 'serde*'` or `--test '*'`).
  [#8752](https://github.com/rust-lang/cargo/pull/8752)

### Changed
- Computed LTO flags are now included in the filename metadata hash so that
  changes in LTO settings will independently cache build artifacts instead of
  overwriting previous ones. This prevents rebuilds in some situations such as
  switching between `cargo build` and `cargo test` in some circumstances.
  [#8755](https://github.com/rust-lang/cargo/pull/8755)
- `cargo tree` now displays `(proc-macro)` next to proc-macro packages.
  [#8765](https://github.com/rust-lang/cargo/pull/8765)
- Added a warning that the allowed characters for a feature name have been
  restricted to letters, digits, `_`, `-`, and `+` to accommodate future
  syntax changes. This is still a superset of the allowed syntax on crates.io,
  which requires ASCII. This is intended to be changed to an error in the
  future.
  [#8814](https://github.com/rust-lang/cargo/pull/8814)
- `-p` without a value will now print a list of workspace package names.
  [#8808](https://github.com/rust-lang/cargo/pull/8808)
- Add period to allowed feature name characters.
  [#8932](https://github.com/rust-lang/cargo/pull/8932)
  [#8943](https://github.com/rust-lang/cargo/pull/8943)

### Fixed
- Fixed building a library with both "dylib" and "rlib" crate types with LTO enabled.
  [#8754](https://github.com/rust-lang/cargo/pull/8754)
- Fixed paths in Cargo's dep-info files.
  [#8819](https://github.com/rust-lang/cargo/pull/8819)
- Fixed inconsistent source IDs in `cargo metadata` for git dependencies that
  explicitly specify `branch="master"`.
  [#8824](https://github.com/rust-lang/cargo/pull/8824)
- Fixed re-extracting dependencies which contained a `.cargo-ok` file.
  [#8835](https://github.com/rust-lang/cargo/pull/8835)

### Nightly only
- Fixed a panic with `cargo doc -Zfeatures=itarget` in some situations.
  [#8777](https://github.com/rust-lang/cargo/pull/8777)
- New implementation for namespaced features, using the syntax `dep:serde`.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#namespaced-features)
  [#8799](https://github.com/rust-lang/cargo/pull/8799)
- Added support for "weak" dependency features, using the syntax
  `dep_name?/feat_name`, which will enable a feature for a dependency without
  also enabling the dependency.
  [#8818](https://github.com/rust-lang/cargo/pull/8818)
- Fixed the new feature resolver downloading extra dependencies that weren't
  strictly necessary.
  [#8823](https://github.com/rust-lang/cargo/pull/8823)

## Cargo 1.48 (2020-11-19)
[51b66125...rust-1.48.0](https://github.com/rust-lang/cargo/compare/51b66125...rust-1.48.0)

### Added
- Added `term.progress` configuration option to control when and how the
  progress bar is displayed.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#termprogresswhen)
  [#8165](https://github.com/rust-lang/cargo/pull/8165)
- Added `--message-format plain` option to `cargo locate-project` to display
  the project location without JSON to make it easier to use in a script.
  [#8707](https://github.com/rust-lang/cargo/pull/8707)
- Added `--workspace` option to `cargo locate-project` to display the path to
  the workspace manifest.
  [#8712](https://github.com/rust-lang/cargo/pull/8712)
- A new contributor guide has been added for contributing to Cargo itself.
  This is published at <https://rust-lang.github.io/cargo/contrib/>.
  [#8715](https://github.com/rust-lang/cargo/pull/8715)
- Zsh `--target` completion will now complete with the built-in rustc targets.
  [#8740](https://github.com/rust-lang/cargo/pull/8740)

### Changed

### Fixed
- Fixed `cargo new` creating a fossil repository to properly ignore the `target` directory.
  [#8671](https://github.com/rust-lang/cargo/pull/8671)
- Don't show warnings about the workspace in the current directory when using `cargo install`
  of a remote package.
  [#8681](https://github.com/rust-lang/cargo/pull/8681)
- Automatically reinitialize the index when an "Object not found" error is
  encountered in the git repository.
  [#8735](https://github.com/rust-lang/cargo/pull/8735)
- Updated libgit2, which brings in several fixes for git repository handling.
  [#8778](https://github.com/rust-lang/cargo/pull/8778)
  [#8780](https://github.com/rust-lang/cargo/pull/8780)

### Nightly only
- Fixed `cargo install` so that it will ignore the `[unstable]` table in local config files.
  [#8656](https://github.com/rust-lang/cargo/pull/8656)
- Fixed nondeterministic behavior of the new feature resolver.
  [#8701](https://github.com/rust-lang/cargo/pull/8701)
- Fixed running `cargo test` on a proc-macro with the new feature resolver
  under a specific combination of circumstances.
  [#8742](https://github.com/rust-lang/cargo/pull/8742)

## Cargo 1.47 (2020-10-08)
[4f74d9b2...rust-1.47.0](https://github.com/rust-lang/cargo/compare/4f74d9b2...rust-1.47.0)

### Added
- `cargo doc` will now include the package's version in the left sidebar.
  [#8509](https://github.com/rust-lang/cargo/pull/8509)
- Added the `test` field to `cargo metadata` targets.
  [#8478](https://github.com/rust-lang/cargo/pull/8478)
- Cargo's man pages are now displayed via the `cargo help` command (such as
  `cargo help build`).
  [#8456](https://github.com/rust-lang/cargo/pull/8456)
  [#8577](https://github.com/rust-lang/cargo/pull/8577)
- Added new documentation chapters on [how dependency resolution
  works](https://doc.rust-lang.org/nightly/cargo/reference/resolver.html) and
  [SemVer
  compatibility](https://doc.rust-lang.org/nightly/cargo/reference/semver.html),
  along with suggestions on how to version your project and work with
  dependencies.
  [#8609](https://github.com/rust-lang/cargo/pull/8609)

### Changed
- The comments added to `.gitignore` when it is modified have been tweaked to
  add some spacing.
  [#8476](https://github.com/rust-lang/cargo/pull/8476)
- `cargo metadata` output should now be sorted to be deterministic.
  [#8489](https://github.com/rust-lang/cargo/pull/8489)
- By default, build scripts and proc-macros are now built with `opt-level=0`
  and the default codegen units, even in release mode.
  [#8500](https://github.com/rust-lang/cargo/pull/8500)
- `workspace.default-members` is now filtered by `workspace.exclude`.
  [#8485](https://github.com/rust-lang/cargo/pull/8485)
- `workspace.members` globs now ignore non-directory paths.
  [#8511](https://github.com/rust-lang/cargo/pull/8511)
- git zlib errors now trigger a retry.
  [#8520](https://github.com/rust-lang/cargo/pull/8520)
- "http" class git errors now trigger a retry.
  [#8553](https://github.com/rust-lang/cargo/pull/8553)
- git dependencies now override the `core.autocrlf` git configuration value to
  ensure they behave consistently across platforms, particularly when
  vendoring git dependencies on Windows.
  [#8523](https://github.com/rust-lang/cargo/pull/8523)
- If `Cargo.lock` needs to be updated, then it will be automatically
  transitioned to the new V2 format. This format removes the `[metadata]`
  table, and should be easier to merge changes in source control systems. This
  format was introduced in 1.38, and made the default for new projects in
  1.41.
  [#8554](https://github.com/rust-lang/cargo/pull/8554)
- Added preparation for support of git repositories with a non-"master"
  default branch. Actual support will arrive in a future version. This
  introduces some warnings:
  - Warn if a git dependency does not specify a branch, and the default branch
    on the repository is not "master". In the future, Cargo will fetch the
    default branch. In this scenario, the branch should be explicitly
    specified.
  - Warn if a workspace has multiple dependencies to the same git repository,
    one without a `branch` and one with `branch="master"`. Dependencies should
    all use one form or the other.
  [#8522](https://github.com/rust-lang/cargo/pull/8522)
- Warnings are now issued if a `required-features` entry lists a feature that
  does not exist.
  [#7950](https://github.com/rust-lang/cargo/pull/7950)
- Built-in aliases are now included in `cargo --list`.
  [#8542](https://github.com/rust-lang/cargo/pull/8542)
- `cargo install` with a specific version that has been yanked will now
  display an error message that it has been yanked, instead of "could not
  find".
  [#8565](https://github.com/rust-lang/cargo/pull/8565)
- `cargo publish` with a package that has the `publish` field set to a single
  registry, and no `--registry` flag has been given, will now publish to that
  registry instead of generating an error.
  [#8571](https://github.com/rust-lang/cargo/pull/8571)

### Fixed
- Fixed issue where if a project directory was moved, and one of the
  build scripts did not use the `rerun-if-changed` directive, then that
  build script was being rebuilt when it shouldn't.
  [#8497](https://github.com/rust-lang/cargo/pull/8497)
- Console colors should now work on Windows 7 and 8.
  [#8540](https://github.com/rust-lang/cargo/pull/8540)
- The `CARGO_TARGET_{triplet}_RUNNER` environment variable will now correctly
  override the config file instead of trying to merge the commands.
  [#8629](https://github.com/rust-lang/cargo/pull/8629)
- Fixed LTO with doctests.
  [#8657](https://github.com/rust-lang/cargo/pull/8657)
  [#8658](https://github.com/rust-lang/cargo/pull/8658)

### Nightly only
- Added support for `-Z terminal-width` which tells `rustc` the width of the
  terminal so that it can format diagnostics better.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#terminal-width)
  [#8427](https://github.com/rust-lang/cargo/pull/8427)
- Added ability to configure `-Z` unstable flags in config files via the
  `[unstable]` table.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html)
  [#8393](https://github.com/rust-lang/cargo/pull/8393)
- Added `-Z build-std-features` flag to set features for the standard library.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features)
  [#8490](https://github.com/rust-lang/cargo/pull/8490)

## Cargo 1.46 (2020-08-27)
[9fcb8c1d...rust-1.46.0](https://github.com/rust-lang/cargo/compare/9fcb8c1d...rust-1.46.0)

### Added
- The `dl` key in `config.json` of a registry index now supports the
  replacement markers `{prefix}` and `{lowerprefix}` to allow spreading crates
  across directories similar to how the index itself is structured.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/registries.html#index-format)
  [#8267](https://github.com/rust-lang/cargo/pull/8267)
- Added new environment variables that are set during compilation:
  - `CARGO_CRATE_NAME`: The name of the crate being built.
  - `CARGO_BIN_NAME`: The name of the executable binary (if this is a binary crate).
  - `CARGO_PKG_LICENSE`: The `license` field from the manifest.
  - `CARGO_PKG_LICENSE_FILE`: The `license-file` field from the manifest.
  [#8270](https://github.com/rust-lang/cargo/pull/8270)
  [#8325](https://github.com/rust-lang/cargo/pull/8325)
  [#8387](https://github.com/rust-lang/cargo/pull/8387)
- If the value for `readme` is not specified in `Cargo.toml`, it is now
  automatically inferred from the existence of a file named `README`,
  `README.md`, or `README.txt`. This can be suppressed by setting
  `readme = false`.
  [#8277](https://github.com/rust-lang/cargo/pull/8277)
- `cargo install` now supports the `--index` flag to install directly from an index.
  [#8344](https://github.com/rust-lang/cargo/pull/8344)
- Added the `metadata` table to the `workspace` definition in `Cargo.toml`.
  This can be used for arbitrary data similar to the `package.metadata` table.
  [#8323](https://github.com/rust-lang/cargo/pull/8323)
- Added the `--target-dir` flag to `cargo install` to set the target directory.
  [#8391](https://github.com/rust-lang/cargo/pull/8391)
- Changes to environment variables used by the
  [`env!`](https://doc.rust-lang.org/std/macro.env.html) or
  [`option_env!`](https://doc.rust-lang.org/std/macro.option_env.html) macros
  are now automatically detected to trigger a rebuild.
  [#8421](https://github.com/rust-lang/cargo/pull/8421)
- The `target` directory now includes the `CACHEDIR.TAG` file which is used by
  some tools to exclude the directory from backups.
  [#8378](https://github.com/rust-lang/cargo/pull/8378)
- Added docs about rustup's `+toolchain` syntax.
  [#8455](https://github.com/rust-lang/cargo/pull/8455)

### Changed
- A warning is now displayed if a git dependency includes a `#` fragment in
  the URL. This was potentially confusing because Cargo itself displays git
  URLs with this syntax, but it does not have any meaning outside of the
  `Cargo.lock` file, and would not work properly.
  [#8297](https://github.com/rust-lang/cargo/pull/8297)
- Various optimizations and fixes for bitcode embedding and LTO.
  [#8349](https://github.com/rust-lang/cargo/pull/8349)
- Reduced the amount of data fetched for git dependencies. If Cargo knows the
  branch or tag to fetch, it will now only fetch that branch or tag instead of
  all branches and tags.
  [#8363](https://github.com/rust-lang/cargo/pull/8363)
- Enhanced git fetch error messages.
  [#8409](https://github.com/rust-lang/cargo/pull/8409)
- `.crate` files are now generated with GNU tar format instead of UStar, which
  supports longer file names.
  [#8453](https://github.com/rust-lang/cargo/pull/8453)

### Fixed
- Fixed a rare situation where an update to `Cargo.lock` failed once, but then
  subsequent runs allowed it proceed.
  [#8274](https://github.com/rust-lang/cargo/pull/8274)
- Removed assertion that Windows dylibs must have a `.dll` extension. Some
  custom JSON spec targets may change the extension.
  [#8310](https://github.com/rust-lang/cargo/pull/8310)
- Updated libgit2, which brings in a fix for zlib errors for some remote
  git servers like googlesource.com.
  [#8320](https://github.com/rust-lang/cargo/pull/8320)
- Fixed the GitHub fast-path check for up-to-date git dependencies on
  non-master branches.
  [#8363](https://github.com/rust-lang/cargo/pull/8363)
- Fixed issue when enabling a feature with `pkg/feature` syntax, and `pkg` is
  an optional dependency, but also a dev-dependency, and the dev-dependency
  appears before the optional normal dependency in the registry summary, then
  the optional dependency would not get activated.
  [#8395](https://github.com/rust-lang/cargo/pull/8395)
- Fixed `clean -p` deleting the build directory if there is a test named
  `build`.
  [#8398](https://github.com/rust-lang/cargo/pull/8398)
- Fixed indentation of multi-line Cargo error messages.
  [#8409](https://github.com/rust-lang/cargo/pull/8409)
- Fixed issue where the automatic inclusion of the `--document-private-items`
  flag for rustdoc would override any flags passed to the `cargo rustdoc`
  command.
  [#8449](https://github.com/rust-lang/cargo/pull/8449)
- Cargo will now include a version in the hash of the fingerprint directories
  to support backwards-incompatible changes to the fingerprint structure.
  [#8473](https://github.com/rust-lang/cargo/pull/8473)
  [#8488](https://github.com/rust-lang/cargo/pull/8488)

### Nightly only
- Added `-Zrustdoc-map` feature which provides external mappings for rustdoc
  (such as https://docs.rs/ links).
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#rustdoc-map)
  [#8287](https://github.com/rust-lang/cargo/pull/8287)
- Fixed feature calculation when a proc-macro is declared in `Cargo.toml` with
  an underscore (like `proc_macro = true`).
  [#8319](https://github.com/rust-lang/cargo/pull/8319)
- Added support for setting `-Clinker` with `-Zdoctest-xcompile`.
  [#8359](https://github.com/rust-lang/cargo/pull/8359)
- Fixed setting the `strip` profile field in config files.
  [#8454](https://github.com/rust-lang/cargo/pull/8454)

## Cargo 1.45 (2020-07-16)
[ebda5065e...rust-1.45.0](https://github.com/rust-lang/cargo/compare/ebda5065...rust-1.45.0)

### Added

### Changed
- Changed official documentation to recommend `.cargo/config.toml` filenames
  (with the `.toml` extension). `.toml` extension support was added in 1.39.
  [#8121](https://github.com/rust-lang/cargo/pull/8121)
- The `registry.index` config value is no longer allowed (it has been
  deprecated for 4 years).
  [#7973](https://github.com/rust-lang/cargo/pull/7973)
- An error is generated if both `--index` and `--registry` are passed
  (previously `--index` was silently ignored).
  [#7973](https://github.com/rust-lang/cargo/pull/7973)
- The `registry.token` config value is no longer used with the `--index` flag.
  This is intended to avoid potentially leaking the crates.io token to another
  registry.
  [#7973](https://github.com/rust-lang/cargo/pull/7973)
- Added a warning if `registry.token` is used with source replacement. It is
  intended this will be an error in future versions.
  [#7973](https://github.com/rust-lang/cargo/pull/7973)
- Windows GNU targets now copy `.dll.a` import library files for DLL crate
  types to the output directory.
  [#8141](https://github.com/rust-lang/cargo/pull/8141)
- Dylibs for all dependencies are now unconditionally copied to the output
  directory. Some obscure scenarios can cause an old dylib to be referenced
  between builds, and this ensures that all the latest copies are used.
  [#8139](https://github.com/rust-lang/cargo/pull/8139)
- `package.exclude` can now match directory names. If a directory is
  specified, the entire directory will be excluded, and Cargo will not attempt
  to inspect it further. Previously Cargo would try to check every file in the
  directory which could cause problems if the directory contained unreadable
  files.
  [#8095](https://github.com/rust-lang/cargo/pull/8095)
- When packaging with `cargo publish` or `cargo package`, Cargo can use git to
  guide its decision on which files to include. Previously this git-based
  logic required a `Cargo.toml` file to exist at the root of the repository.
  This is no longer required, so Cargo will now use git-based guidance even if
  there is not a `Cargo.toml` in the root of the repository.
  [#8095](https://github.com/rust-lang/cargo/pull/8095)
- While unpacking a crate on Windows, if it fails to write a file because the
  file is a reserved Windows filename (like "aux.rs"), Cargo will display an
  extra message to explain why it failed.
  [#8136](https://github.com/rust-lang/cargo/pull/8136)
- Failures to set mtime on files are now ignored. Some filesystems did not
  support this.
  [#8185](https://github.com/rust-lang/cargo/pull/8185)
- Certain classes of git errors will now recommend enabling
  `net.git-fetch-with-cli`.
  [#8166](https://github.com/rust-lang/cargo/pull/8166)
- When doing an LTO build, Cargo will now instruct rustc not to perform
  codegen when possible. This may result in a faster build and use less disk
  space. Additionally, for non-LTO builds, Cargo will instruct rustc to not
  embed LLVM bitcode in libraries, which should decrease their size.
  [#8192](https://github.com/rust-lang/cargo/pull/8192)
  [#8226](https://github.com/rust-lang/cargo/pull/8226)
  [#8254](https://github.com/rust-lang/cargo/pull/8254)
- The implementation for `cargo clean -p` has been rewritten so that it can
  more accurately remove the files for a specific package.
  [#8210](https://github.com/rust-lang/cargo/pull/8210)
- The way Cargo computes the outputs from a build has been rewritten to be
  more complete and accurate. Newly tracked files will be displayed in JSON
  messages, and may be uplifted to the output directory in some cases. Some of
  the changes from this are:

  - `.exp` export files on Windows MSVC dynamic libraries are now tracked.
  - Proc-macros on Windows track import/export files.
  - All targets (like tests, etc.) that generate separate debug files
    (pdb/dSYM) are tracked.
  - Added .map files for wasm32-unknown-emscripten.
  - macOS dSYM directories are tracked for all dynamic libraries
    (dylib/cdylib/proc-macro) and for build scripts.

  There are a variety of other changes as a consequence of this:

  - Binary examples on Windows MSVC with a hyphen will now show up twice in
    the examples directory (`foo_bar.exe` and `foo-bar.exe`). Previously Cargo
    just renamed the file instead of hard-linking it.
  - Example libraries now follow the same rules for hyphen/underscore
    translation as normal libs (they will now use underscores).

  [#8210](https://github.com/rust-lang/cargo/pull/8210)
- Cargo attempts to scrub any secrets from the debug log for HTTP debugging.
  [#8222](https://github.com/rust-lang/cargo/pull/8222)
- Context has been added to many of Cargo's filesystem operations, so that
  error messages now provide more information, such as the path that caused
  the problem.
  [#8232](https://github.com/rust-lang/cargo/pull/8232)
- Several commands now ignore the error if stdout or stderr is closed while it
  is running. For example `cargo install --list | grep -q cargo-fuzz` would
  previously sometimes panic because `grep -q` may close stdout before the
  command finishes. Regular builds continue to fail if stdout or stderr is
  closed, matching the behavior of many other build systems.
  [#8236](https://github.com/rust-lang/cargo/pull/8236)
- If `cargo install` is given an exact version, like `--version=1.2.3`, it
  will now avoid updating the index if that version is already installed, and
  exit quickly indicating it is already installed.
  [#8022](https://github.com/rust-lang/cargo/pull/8022)
- Changes to the `[patch]` section will now attempt to automatically update
  `Cargo.lock` to the new version. It should now also provide better error
  messages for the rare cases where it is unable to automatically update.
  [#8248](https://github.com/rust-lang/cargo/pull/8248)

### Fixed
- Fixed copying Windows `.pdb` files to the output directory when the filename
  contained dashes.
  [#8123](https://github.com/rust-lang/cargo/pull/8123)
- Fixed error where Cargo would fail when scanning if a package is inside a
  git repository when any of its ancestor paths is a symlink.
  [#8186](https://github.com/rust-lang/cargo/pull/8186)
- Fixed `cargo update` with an unused `[patch]` so that it does not get
  stuck and refuse to update.
  [#8243](https://github.com/rust-lang/cargo/pull/8243)
- Fixed a situation where Cargo would hang if stderr is closed, and the
  compiler generated a large number of messages.
  [#8247](https://github.com/rust-lang/cargo/pull/8247)
- Fixed backtraces on macOS not showing filenames or line numbers. As a
  consequence of this, binary executables on apple targets do not include a
  hash in the filename in Cargo's cache. This means Cargo can only track one
  copy, so if you switch features or rustc versions, Cargo will need to
  rebuild the executable.
  [#8329](https://github.com/rust-lang/cargo/pull/8329)
  [#8335](https://github.com/rust-lang/cargo/pull/8335)
- Fixed fingerprinting when using lld on Windows with a dylib. Cargo was
  erroneously thinking the dylib was never fresh.
  [#8290](https://github.com/rust-lang/cargo/pull/8290)
  [#8335](https://github.com/rust-lang/cargo/pull/8335)

### Nightly only
- Fixed passing the full path for `--target` to `rustdoc` when using JSON spec
  targets.
  [#8094](https://github.com/rust-lang/cargo/pull/8094)
- `-Cembed-bitcode=no` renamed to `-Cbitcode-in-rlib=no`
  [#8134](https://github.com/rust-lang/cargo/pull/8134)
- Added new `resolver` field to `Cargo.toml` to opt-in to the new feature
  resolver.
  [#8129](https://github.com/rust-lang/cargo/pull/8129)
- `-Zbuild-std` no longer treats std dependencies as "local". This means that
  it won't use incremental compilation for those dependencies, removes them
  from dep-info files, and caps lints at "allow".
  [#8177](https://github.com/rust-lang/cargo/pull/8177)
- Added `-Zmultitarget` which allows multiple `--target` flags to build the
  same thing for multiple targets at once.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#multitarget)
  [#8167](https://github.com/rust-lang/cargo/pull/8167)
- Added `strip` option to the profile to remove symbols and debug information.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#profile-strip-option)
  [#8246](https://github.com/rust-lang/cargo/pull/8246)
- Fixed panic with `cargo tree --target=all -Zfeatures=all`.
  [#8269](https://github.com/rust-lang/cargo/pull/8269)

## Cargo 1.44 (2020-06-04)
[bda50510...rust-1.44.0](https://github.com/rust-lang/cargo/compare/bda50510...rust-1.44.0)

### Added
- 🔥 Added the `cargo tree` command.
  [docs](https://doc.rust-lang.org/nightly/cargo/commands/cargo-tree.html)
  [#8062](https://github.com/rust-lang/cargo/pull/8062)
- Added warnings if a package has Windows-restricted filenames (like `nul`,
  `con`, `aux`, `prn`, etc.).
  [#7959](https://github.com/rust-lang/cargo/pull/7959)
- Added a `"build-finished"` JSON message when compilation is complete so that
  tools can detect when they can stop listening for JSON messages with
  commands like `cargo run` or `cargo test`.
  [#8069](https://github.com/rust-lang/cargo/pull/8069)

### Changed
- Valid package names are now restricted to Unicode XID identifiers. This is
  mostly the same as before, except package names cannot start with a number
  or `-`.
  [#7959](https://github.com/rust-lang/cargo/pull/7959)
- `cargo new` and `init` will warn or reject additional package names
  (reserved Windows names, reserved Cargo directories, non-ASCII names,
  conflicting std names like `core`, etc.).
  [#7959](https://github.com/rust-lang/cargo/pull/7959)
- Tests are no longer hard-linked into the output directory (`target/debug/`).
  This ensures tools will have access to debug symbols and execute tests in
  the same way as Cargo. Tools should use JSON messages to discover the path
  to the executable.
  [#7965](https://github.com/rust-lang/cargo/pull/7965)
- Updating git submodules now displays an "Updating" message for each
  submodule.
  [#7989](https://github.com/rust-lang/cargo/pull/7989)
- File modification times are now preserved when extracting a `.crate` file.
  This reverses the change made in 1.40 where the mtime was not preserved.
  [#7935](https://github.com/rust-lang/cargo/pull/7935)
- Build script warnings are now displayed separately when the build script
  fails.
  [#8017](https://github.com/rust-lang/cargo/pull/8017)
- Removed the `git-checkout` subcommand.
  [#8040](https://github.com/rust-lang/cargo/pull/8040)
- The progress bar is now enabled for all unix platforms. Previously it was
  only Linux, macOS, and FreeBSD.
  [#8054](https://github.com/rust-lang/cargo/pull/8054)
- Artifacts generated by pre-release versions of `rustc` now share the same
  filenames. This means that changing nightly versions will not leave stale
  files in the build directory.
  [#8073](https://github.com/rust-lang/cargo/pull/8073)
- Invalid package names are rejected when using renamed dependencies.
  [#8090](https://github.com/rust-lang/cargo/pull/8090)
- Added a certain class of HTTP2 errors as "spurious" that will get retried.
  [#8102](https://github.com/rust-lang/cargo/pull/8102)
- Allow `cargo package --list` to succeed, even if there are other validation
  errors (such as `Cargo.lock` generation problem, or missing dependencies).
  [#8175](https://github.com/rust-lang/cargo/pull/8175)
  [#8215](https://github.com/rust-lang/cargo/pull/8215)

### Fixed
- Cargo no longer buffers excessive amounts of compiler output in memory.
  [#7838](https://github.com/rust-lang/cargo/pull/7838)
- Symbolic links in git repositories now work on Windows.
  [#7996](https://github.com/rust-lang/cargo/pull/7996)
- Fixed an issue where `profile.dev` was not loaded from a config file with
  `cargo test` when the `dev` profile was not defined in `Cargo.toml`.
  [#8012](https://github.com/rust-lang/cargo/pull/8012)
- When a binary is built as an implicit dependency of an integration test,
  it now checks `dep_name/feature_name` syntax in `required-features` correctly.
  [#8020](https://github.com/rust-lang/cargo/pull/8020)
- Fixed an issue where Cargo would not detect that an executable (such as an
  integration test) needs to be rebuilt when the previous build was
  interrupted with Ctrl-C.
  [#8087](https://github.com/rust-lang/cargo/pull/8087)
- Protect against some (unknown) situations where Cargo could panic when the
  system monotonic clock doesn't appear to be monotonic.
  [#8114](https://github.com/rust-lang/cargo/pull/8114)
- Fixed panic with `cargo clean -p` if the package has a build script.
  [#8216](https://github.com/rust-lang/cargo/pull/8216)

### Nightly only
- Fixed panic with new feature resolver and required-features.
  [#7962](https://github.com/rust-lang/cargo/pull/7962)
- Added `RUSTC_WORKSPACE_WRAPPER` environment variable, which provides a way
  to wrap `rustc` for workspace members only, and affects the filename hash so
  that artifacts produced by the wrapper are cached separately. This usage can
  be seen on nightly clippy with `cargo clippy -Zunstable-options`.
  [#7533](https://github.com/rust-lang/cargo/pull/7533)
- Added `--unit-graph` CLI option to display Cargo's internal dependency graph
  as JSON.
  [#7977](https://github.com/rust-lang/cargo/pull/7977)
- Changed `-Zbuild_dep` to `-Zhost_dep`, and added proc-macros to the feature
  decoupling logic.
  [#8003](https://github.com/rust-lang/cargo/pull/8003)
  [#8028](https://github.com/rust-lang/cargo/pull/8028)
- Fixed so that `--crate-version` is not automatically passed when the flag
  is found in `RUSTDOCFLAGS`.
  [#8014](https://github.com/rust-lang/cargo/pull/8014)
- Fixed panic with `-Zfeatures=dev_dep` and `check --profile=test`.
  [#8027](https://github.com/rust-lang/cargo/pull/8027)
- Fixed panic with `-Zfeatures=itarget` with certain host dependencies.
  [#8048](https://github.com/rust-lang/cargo/pull/8048)
- Added support for `-Cembed-bitcode=no`, which provides a performance boost
  and disk-space usage reduction for non-LTO builds.
  [#8066](https://github.com/rust-lang/cargo/pull/8066)
- `-Zpackage-features` has been extended with several changes intended to make
  it easier to select features on the command-line in a workspace.
  [#8074](https://github.com/rust-lang/cargo/pull/8074)

## Cargo 1.43 (2020-04-23)
[9d32b7b0...rust-1.43.0](https://github.com/rust-lang/cargo/compare/9d32b7b0...rust-1.43.0)

### Added
- 🔥 Profiles may now be specified in config files (and environment variables).
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/config.html#profile)
  [#7823](https://github.com/rust-lang/cargo/pull/7823)
- ❗ Added `CARGO_BIN_EXE_<name>` environment variable when building
  integration tests. This variable contains the path to any `[[bin]]` targets
  in the package. Integration tests should use the `env!` macro to determine
  the path to a binary to execute.
  [docs](https://doc.rust-lang.org/nightly/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates)
  [#7697](https://github.com/rust-lang/cargo/pull/7697)

### Changed
- `cargo install --git` now honors workspaces in a git repository. This allows
  workspace settings, like `[patch]`, `[replace]`, or `[profile]` to be used.
  [#7768](https://github.com/rust-lang/cargo/pull/7768)
- `cargo new` will now run `rustfmt` on the new files to pick up rustfmt
  settings like `tab_spaces` so that the new file matches the user's preferred
  indentation settings.
  [#7827](https://github.com/rust-lang/cargo/pull/7827)
- Environment variables printed with "very verbose" output (`-vv`) are now
  consistently sorted.
  [#7877](https://github.com/rust-lang/cargo/pull/7877)
- Debug logging for fingerprint rebuild-detection now includes more information.
  [#7888](https://github.com/rust-lang/cargo/pull/7888)
  [#7890](https://github.com/rust-lang/cargo/pull/7890)
  [#7952](https://github.com/rust-lang/cargo/pull/7952)
- Added warning during publish if the license-file doesn't exist.
  [#7905](https://github.com/rust-lang/cargo/pull/7905)
- The `license-file` file is automatically included during publish, even if it
  is not explicitly listed in the `include` list or is in a location outside
  of the root of the package.
  [#7905](https://github.com/rust-lang/cargo/pull/7905)
- `CARGO_CFG_DEBUG_ASSERTIONS` and `CARGO_CFG_PROC_MACRO` are no longer set
  when running a build script. These were inadvertently set in the past, but
  had no meaning as they were always true. Additionally, `cfg(proc-macro)`
  is no longer supported in a `target` expression.
  [#7943](https://github.com/rust-lang/cargo/pull/7943)
  [#7970](https://github.com/rust-lang/cargo/pull/7970)

### Fixed
- Global command-line flags now work with aliases (like `cargo -v b`).
  [#7837](https://github.com/rust-lang/cargo/pull/7837)
- Required-features using dependency syntax (like `renamed_dep/feat_name`) now
  handle renamed dependencies correctly.
  [#7855](https://github.com/rust-lang/cargo/pull/7855)
- Fixed a rare situation where if a build script is run multiple times during
  the same build, Cargo will now keep the results separate instead of losing
  the output of the first execution.
  [#7857](https://github.com/rust-lang/cargo/pull/7857)
- Fixed incorrect interpretation of environment variable
  `CARGO_TARGET_*_RUNNER=true` as a boolean. Also improved related env var
  error messages.
  [#7891](https://github.com/rust-lang/cargo/pull/7891)
- Updated internal libgit2 library, bringing various fixes to git support.
  [#7939](https://github.com/rust-lang/cargo/pull/7939)
- `cargo package` / `cargo publish` should no longer buffer the entire
  contents of each file in memory.
  [#7946](https://github.com/rust-lang/cargo/pull/7946)
- Ignore more invalid `Cargo.toml` files in a git dependency. Cargo currently
  walks the entire repo to find the requested package. Certain invalid
  manifests were already skipped, and now it should skip all of them.
  [#7947](https://github.com/rust-lang/cargo/pull/7947)

### Nightly only
- Added `build.out-dir` config variable to set the output directory.
  [#7810](https://github.com/rust-lang/cargo/pull/7810)
- Added `-Zjobserver-per-rustc` feature to support improved performance for
  parallel rustc.
  [#7731](https://github.com/rust-lang/cargo/pull/7731)
- Fixed filename collision with `build-std` and crates like `cc`.
  [#7860](https://github.com/rust-lang/cargo/pull/7860)
- `-Ztimings` will now save its report even if there is an error.
  [#7872](https://github.com/rust-lang/cargo/pull/7872)
- Updated `--config` command-line flag to support taking a path to a config
  file to load.
  [#7901](https://github.com/rust-lang/cargo/pull/7901)
- Added new feature resolver.
  [#7820](https://github.com/rust-lang/cargo/pull/7820)
- Rustdoc docs now automatically include the version of the package in the
  side bar (requires `-Z crate-versions` flag).
  [#7903](https://github.com/rust-lang/cargo/pull/7903)

## Cargo 1.42 (2020-03-12)
[0bf7aafe...rust-1.42.0](https://github.com/rust-lang/cargo/compare/0bf7aafe...rust-1.42.0)

### Added
- Added documentation on git authentication.
  [#7658](https://github.com/rust-lang/cargo/pull/7658)
- Bitbucket Pipeline badges are now supported on crates.io.
  [#7663](https://github.com/rust-lang/cargo/pull/7663)
- `cargo vendor` now accepts the `--versioned-dirs` option to force it to
  always include the version number in each package's directory name.
  [#7631](https://github.com/rust-lang/cargo/pull/7631)
- The `proc_macro` crate is now automatically added to the extern prelude for
  proc-macro packages. This means that `extern crate proc_macro;` is no longer
  necessary for proc-macros.
  [#7700](https://github.com/rust-lang/cargo/pull/7700)

### Changed
- Emit a warning if `debug_assertions`, `test`, `proc_macro`, or `feature=` is
  used in a `cfg()` expression.
  [#7660](https://github.com/rust-lang/cargo/pull/7660)
- Large update to the Cargo documentation, adding new chapters on Cargo
  targets, workspaces, and features.
  [#7733](https://github.com/rust-lang/cargo/pull/7733)
- Windows: `.lib` DLL import libraries are now copied next to the dll for all
  Windows MSVC targets. Previously it was only supported for
  `pc-windows-msvc`. This adds DLL support for `uwp-windows-msvc` targets.
  [#7758](https://github.com/rust-lang/cargo/pull/7758)
- The `ar` field in the `[target]` configuration is no longer read. It has
  been ignored for over 4 years.
  [#7763](https://github.com/rust-lang/cargo/pull/7763)
- Bash completion file simplified and updated for latest changes.
  [#7789](https://github.com/rust-lang/cargo/pull/7789)
- Credentials are only loaded when needed, instead of every Cargo command.
  [#7774](https://github.com/rust-lang/cargo/pull/7774)

### Fixed
- Removed `--offline` empty index check, which was a false positive in some
  cases.
  [#7655](https://github.com/rust-lang/cargo/pull/7655)
- Files and directories starting with a `.` can now be included in a package
  by adding it to the `include` list.
  [#7680](https://github.com/rust-lang/cargo/pull/7680)
- Fixed `cargo login` removing alternative registry tokens when previous
  entries existed in the credentials file.
  [#7708](https://github.com/rust-lang/cargo/pull/7708)
- Fixed `cargo vendor` from panicking when used with alternative registries.
  [#7718](https://github.com/rust-lang/cargo/pull/7718)
- Fixed incorrect explanation in the fingerprint debug log message.
  [#7749](https://github.com/rust-lang/cargo/pull/7749)
- A `[source]` that is defined multiple times will now result in an error.
  Previously it was randomly picking a source, which could cause
  non-deterministic behavior.
  [#7751](https://github.com/rust-lang/cargo/pull/7751)
- `dep_kinds` in `cargo metadata` are now de-duplicated.
  [#7756](https://github.com/rust-lang/cargo/pull/7756)
- Fixed packaging where `Cargo.lock` was listed in `.gitignore` in a
  subdirectory inside a git repository. Previously it was assuming
  `Cargo.lock` was at the root of the repo.
  [#7779](https://github.com/rust-lang/cargo/pull/7779)
- Partial file transfer errors will now cause an automatic retry.
  [#7788](https://github.com/rust-lang/cargo/pull/7788)
- Linux: Fixed panic if CPU iowait stat decreases.
  [#7803](https://github.com/rust-lang/cargo/pull/7803)
- Fixed using the wrong sysroot for detecting host compiler settings when
  `--sysroot` is passed in via `RUSTFLAGS`.
  [#7798](https://github.com/rust-lang/cargo/pull/7798)

### Nightly only
- `build-std` now uses `--extern` instead of `--sysroot` to find sysroot
  packages.
  [#7699](https://github.com/rust-lang/cargo/pull/7699)
- Added `--config` command-line option to set config settings.
  [#7649](https://github.com/rust-lang/cargo/pull/7649)
- Added `include` config setting which allows including another config file.
  [#7649](https://github.com/rust-lang/cargo/pull/7649)
- Profiles in config files now support any named profile. Previously it was
  limited to dev/release.
  [#7750](https://github.com/rust-lang/cargo/pull/7750)

## Cargo 1.41 (2020-01-30)
[5da4b4d4...rust-1.41.0](https://github.com/rust-lang/cargo/compare/5da4b4d4...rust-1.41.0)

### Added
- 🔥 Cargo now uses a new `Cargo.lock` file format. This new format should
  support easier merges in source control systems. Projects using the old
  format will continue to use the old format, only new `Cargo.lock` files will
  use the new format.
  [#7579](https://github.com/rust-lang/cargo/pull/7579)
- 🔥 `cargo install` will now upgrade already installed packages instead of
  failing.
  [#7560](https://github.com/rust-lang/cargo/pull/7560)
- 🔥 Profile overrides have been added. This allows overriding profiles for
  individual dependencies or build scripts. See [the
  documentation](https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#overrides)
  for more.
  [#7591](https://github.com/rust-lang/cargo/pull/7591)
- Added new documentation for build scripts.
  [#7565](https://github.com/rust-lang/cargo/pull/7565)
- Added documentation for Cargo's JSON output.
  [#7595](https://github.com/rust-lang/cargo/pull/7595)
- Significant expansion of config and environment variable documentation.
  [#7650](https://github.com/rust-lang/cargo/pull/7650)
- Add back support for `BROWSER` environment variable for `cargo doc --open`.
  [#7576](https://github.com/rust-lang/cargo/pull/7576)
- Added `kind` and `platform` for dependencies in `cargo metadata`.
  [#7132](https://github.com/rust-lang/cargo/pull/7132)
- The `OUT_DIR` value is now included in the `build-script-executed` JSON message.
  [#7622](https://github.com/rust-lang/cargo/pull/7622)

### Changed
- `cargo doc` will now document private items in binaries by default.
  [#7593](https://github.com/rust-lang/cargo/pull/7593)
- Subcommand typo suggestions now include aliases.
  [#7486](https://github.com/rust-lang/cargo/pull/7486)
- Tweak how the "already existing..." comment is added to `.gitignore`.
  [#7570](https://github.com/rust-lang/cargo/pull/7570)
- Ignore `cargo login` text from copy/paste in token.
  [#7588](https://github.com/rust-lang/cargo/pull/7588)
- Windows: Ignore errors for locking files when not supported by the filesystem.
  [#7602](https://github.com/rust-lang/cargo/pull/7602)
- Remove `**/*.rs.bk` from `.gitignore`.
  [#7647](https://github.com/rust-lang/cargo/pull/7647)

### Fixed
- Fix unused warnings for some keys in the `build` config section.
  [#7575](https://github.com/rust-lang/cargo/pull/7575)
- Linux: Don't panic when parsing `/proc/stat`.
  [#7580](https://github.com/rust-lang/cargo/pull/7580)
- Don't show canonical path in `cargo vendor`.
  [#7629](https://github.com/rust-lang/cargo/pull/7629)

### Nightly only


## Cargo 1.40 (2019-12-19)
[1c6ec66d...5da4b4d4](https://github.com/rust-lang/cargo/compare/1c6ec66d...5da4b4d4)

### Added
- Added `http.ssl-version` config option to control the version of TLS,
  along with min/max versions.
  [#7308](https://github.com/rust-lang/cargo/pull/7308)
- 🔥 Compiler warnings are now cached on disk. If a build generates warnings,
  re-running the build will now re-display the warnings.
  [#7450](https://github.com/rust-lang/cargo/pull/7450)
- Added `--filter-platform` option to `cargo metadata` to narrow the nodes
  shown in the resolver graph to only packages included for the given target
  triple.
  [#7376](https://github.com/rust-lang/cargo/pull/7376)

### Changed
- Cargo's "platform" `cfg` parsing has been extracted into a separate crate
  named `cargo-platform`.
  [#7375](https://github.com/rust-lang/cargo/pull/7375)
- Dependencies extracted into Cargo's cache no longer preserve mtimes to
  reduce syscall overhead.
  [#7465](https://github.com/rust-lang/cargo/pull/7465)
- Windows: EXE files no longer include a metadata hash in the filename.
  This helps with debuggers correlating the filename with the PDB file.
  [#7400](https://github.com/rust-lang/cargo/pull/7400)
- Wasm32: `.wasm` files are no longer treated as an "executable", allowing
  `cargo test` and `cargo run` to work properly with the generated `.js` file.
  [#7476](https://github.com/rust-lang/cargo/pull/7476)
- crates.io now supports SPDX 3.6 licenses.
  [#7481](https://github.com/rust-lang/cargo/pull/7481)
- Improved cyclic dependency error message.
  [#7470](https://github.com/rust-lang/cargo/pull/7470)
- Bare `cargo clean` no longer locks the package cache.
  [#7502](https://github.com/rust-lang/cargo/pull/7502)
- `cargo publish` now allows dev-dependencies without a version key to be
  published. A git or path-only dev-dependency will be removed from the
  package manifest before uploading.
  [#7333](https://github.com/rust-lang/cargo/pull/7333)
- `--features` and `--no-default-features` in the root of a virtual workspace
  will now generate an error instead of being ignored.
  [#7507](https://github.com/rust-lang/cargo/pull/7507)
- Generated files (like `Cargo.toml` and `Cargo.lock`) in a package archive
  now have their timestamp set to the current time instead of the epoch.
  [#7523](https://github.com/rust-lang/cargo/pull/7523)
- The `-Z` flag parser is now more strict, rejecting more invalid syntax.
  [#7531](https://github.com/rust-lang/cargo/pull/7531)

### Fixed
- Fixed an issue where if a package had an `include` field, and `Cargo.lock`
  in `.gitignore`, and a binary or example target, and the `Cargo.lock` exists
  in the current project, it would fail to publish complaining the
  `Cargo.lock` was dirty.
  [#7448](https://github.com/rust-lang/cargo/pull/7448)
- Fixed a panic in a particular combination of `[patch]` entries.
  [#7452](https://github.com/rust-lang/cargo/pull/7452)
- Windows: Better error message when `cargo test` or `rustc` crashes in an
  abnormal way, such as a signal or seg fault.
  [#7535](https://github.com/rust-lang/cargo/pull/7535)

### Nightly only
- The `mtime-on-use` feature may now be enabled via the
  `unstable.mtime_on_use` config option.
  [#7411](https://github.com/rust-lang/cargo/pull/7411)
- Added support for named profiles.
  [#6989](https://github.com/rust-lang/cargo/pull/6989)
- Added `-Zpanic-abort-tests` to allow building and running tests with the
  "abort" panic strategy.
  [#7460](https://github.com/rust-lang/cargo/pull/7460)
- Changed `build-std` to use `--sysroot`.
  [#7421](https://github.com/rust-lang/cargo/pull/7421)
- Various fixes and enhancements to `-Ztimings`.
  [#7395](https://github.com/rust-lang/cargo/pull/7395)
  [#7398](https://github.com/rust-lang/cargo/pull/7398)
  [#7397](https://github.com/rust-lang/cargo/pull/7397)
  [#7403](https://github.com/rust-lang/cargo/pull/7403)
  [#7428](https://github.com/rust-lang/cargo/pull/7428)
  [#7429](https://github.com/rust-lang/cargo/pull/7429)
- Profile overrides have renamed the syntax to be
  `[profile.dev.package.NAME]`.
  [#7504](https://github.com/rust-lang/cargo/pull/7504)
- Fixed warnings for unused profile overrides in a workspace.
  [#7536](https://github.com/rust-lang/cargo/pull/7536)

## Cargo 1.39 (2019-11-07)
[e853aa97...1c6ec66d](https://github.com/rust-lang/cargo/compare/e853aa97...1c6ec66d)

### Added
- Config files may now use the `.toml` filename extension.
  [#7295](https://github.com/rust-lang/cargo/pull/7295)
- The `--workspace` flag has been added as an alias for `--all` to help avoid
  confusion about the meaning of "all".
  [#7241](https://github.com/rust-lang/cargo/pull/7241)
- The `publish` field has been added to `cargo metadata`.
  [#7354](https://github.com/rust-lang/cargo/pull/7354)

### Changed
- Display more information if parsing the output from `rustc` fails.
  [#7236](https://github.com/rust-lang/cargo/pull/7236)
- TOML errors now show the column number.
  [#7248](https://github.com/rust-lang/cargo/pull/7248)
- `cargo vendor` no longer deletes files in the `vendor` directory that starts
  with a `.`.
  [#7242](https://github.com/rust-lang/cargo/pull/7242)
- `cargo fetch` will now show manifest warnings.
  [#7243](https://github.com/rust-lang/cargo/pull/7243)
- `cargo publish` will now check git submodules if they contain any
  uncommitted changes.
  [#7245](https://github.com/rust-lang/cargo/pull/7245)
- In a build script, `cargo:rustc-flags` now allows `-l` and `-L` flags
  without spaces.
  [#7257](https://github.com/rust-lang/cargo/pull/7257)
- When `cargo install` replaces an older version of a package it will now
  delete any installed binaries that are no longer present in the newly
  installed version.
  [#7246](https://github.com/rust-lang/cargo/pull/7246)
- A git dependency may now also specify a `version` key when published. The
  `git` value will be stripped from the uploaded crate, matching the behavior
  of `path` dependencies.
  [#7237](https://github.com/rust-lang/cargo/pull/7237)
- The behavior of workspace default-members has changed. The default-members
  now only applies when running Cargo in the root of the workspace. Previously
  it would always apply regardless of which directory Cargo is running in.
  [#7270](https://github.com/rust-lang/cargo/pull/7270)
- libgit2 updated pulling in all upstream changes.
  [#7275](https://github.com/rust-lang/cargo/pull/7275)
- Bump `home` dependency for locating home directories.
  [#7277](https://github.com/rust-lang/cargo/pull/7277)
- zsh completions have been updated.
  [#7296](https://github.com/rust-lang/cargo/pull/7296)
- SSL connect errors are now retried.
  [#7318](https://github.com/rust-lang/cargo/pull/7318)
- The jobserver has been changed to acquire N tokens (instead of N-1), and
  then immediately acquires the extra token. This was changed to accommodate
  the `cc` crate on Windows to allow it to release its implicit token.
  [#7344](https://github.com/rust-lang/cargo/pull/7344)
- The scheduling algorithm for choosing which crate to build next has been
  changed. It now chooses the crate with the greatest number of transitive
  crates waiting on it. Previously it used a maximum topological depth.
  [#7390](https://github.com/rust-lang/cargo/pull/7390)
- RUSTFLAGS are no longer incorporated in the metadata and filename hash,
  reversing the change from 1.33 that added it. This means that any change to
  RUSTFLAGS will cause a recompile, and will not affect symbol munging.
  [#7459](https://github.com/rust-lang/cargo/pull/7459)

### Fixed
- Git dependencies with submodules with shorthand SSH URLs (like
  `git@github.com/user/repo.git`) should now work.
  [#7238](https://github.com/rust-lang/cargo/pull/7238)
- Handle broken symlinks when creating `.dSYM` symlinks on macOS.
  [#7268](https://github.com/rust-lang/cargo/pull/7268)
- Fixed issues with multiple versions of the same crate in a `[patch]` table.
  [#7303](https://github.com/rust-lang/cargo/pull/7303)
- Fixed issue with custom target `.json` files where a substring of the name
  matches an unsupported crate type (like "bin").
  [#7363](https://github.com/rust-lang/cargo/issues/7363)
- Fixed issues with generating documentation for proc-macro crate types.
  [#7159](https://github.com/rust-lang/cargo/pull/7159)
- Fixed hang if Cargo panics within a build thread.
  [#7366](https://github.com/rust-lang/cargo/pull/7366)
- Fixed rebuild detection if a `build.rs` script issues different `rerun-if`
  directives between builds. Cargo was erroneously causing a rebuild after the
  change.
  [#7373](https://github.com/rust-lang/cargo/pull/7373)
- Properly handle canonical URLs for `[patch]` table entries, preventing
  the patch from working after the first time it is used.
  [#7368](https://github.com/rust-lang/cargo/pull/7368)
- Fixed an issue where integration tests were waiting for the package binary
  to finish building before starting their own build. They now may build
  concurrently.
  [#7394](https://github.com/rust-lang/cargo/pull/7394)
- Fixed accidental change in the previous release on how `--features a b` flag
  is interpreted, restoring the original behavior where this is interpreted as
  `--features a` along with the argument `b` passed to the command. To pass
  multiple features, use quotes around the features to pass multiple features
  like `--features "a b"`, or use commas, or use multiple `--features` flags.
  [#7419](https://github.com/rust-lang/cargo/pull/7419)

### Nightly only
- Basic support for building the standard library directly from Cargo has been
  added.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std))
  [#7216](https://github.com/rust-lang/cargo/pull/7216)
- Added `-Ztimings` feature to generate an HTML report on the time spent on
  individual compilation steps. This also may output completion steps on the
  console and JSON data.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#timings))
  [#7311](https://github.com/rust-lang/cargo/pull/7311)
- Added ability to cross-compile doctests.
  ([docs](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#doctest-xcompile))
  [#6892](https://github.com/rust-lang/cargo/pull/6892)

## Cargo 1.38 (2019-09-26)
[4c1fa54d...23ef9a4e](https://github.com/rust-lang/cargo/compare/4c1fa54d...23ef9a4e)

### Added
- 🔥 Cargo build pipelining has been enabled by default to leverage more idle CPU
  parallelism during builds.
  [#7143](https://github.com/rust-lang/cargo/pull/7143)
- The `--message-format` option to Cargo can now be specified multiple times and
  accepts a comma-separated list of values. In addition to the previous values
  it also now accepts `json-diagnostic-short` and
  `json-diagnostic-rendered-ansi` which configures the output coming from rustc
  in `json` message mode.
  [#7214](https://github.com/rust-lang/cargo/pull/7214)
- Cirrus CI badges are now supported on crates.io.
  [#7119](https://github.com/rust-lang/cargo/pull/7119)
- A new format for `Cargo.lock` has been introduced. This new format is
  intended to avoid source-control merge conflicts more often, and to
  generally make it safer to merge changes. This new format is *not* enabled
  at this time, though Cargo will use it if it sees it. At some point in the
  future, it is intended that this will become the default.
  [#7070](https://github.com/rust-lang/cargo/pull/7070)
- Progress bar support added for FreeBSD.
  [#7222](https://github.com/rust-lang/cargo/pull/7222)

### Changed
- The `-q` flag will no longer suppress the root error message for an error
  from Cargo itself.
  [#7116](https://github.com/rust-lang/cargo/pull/7116)
- The Cargo Book is now published with mdbook 0.3 providing a number of
  formatting fixes and improvements.
  [#7140](https://github.com/rust-lang/cargo/pull/7140)
- The `--features` command-line flag can now be specified multiple times.
  The list of features from all the flags are joined together.
  [#7084](https://github.com/rust-lang/cargo/pull/7084)
- Package include/exclude glob-vs-gitignore warnings have been removed.
  Packages may now use gitignore-style matching without producing any
  warnings.
  [#7170](https://github.com/rust-lang/cargo/pull/7170)
- Cargo now shows the command and output when parsing `rustc` output fails
  when querying `rustc` for information like `cfg` values.
  [#7185](https://github.com/rust-lang/cargo/pull/7185)
- `cargo package`/`cargo publish` now allows a symbolic link to a git
  submodule to include that submodule.
  [#6817](https://github.com/rust-lang/cargo/pull/6817)
- Improved the error message when a version requirement does not
  match any versions, but there are pre-release versions available.
  [#7191](https://github.com/rust-lang/cargo/pull/7191)

### Fixed
- Fixed using the wrong directory when updating git repositories when using
  the `git-fetch-with-cli` config option, and the `GIT_DIR` environment
  variable is set. This may happen when running cargo from git callbacks.
  [#7082](https://github.com/rust-lang/cargo/pull/7082)
- Fixed dep-info files being overwritten for targets that have separate debug
  outputs. For example, binaries on `-apple-` targets with `.dSYM` directories
  would overwrite the `.d` file.
  [#7057](https://github.com/rust-lang/cargo/pull/7057)
- Fix `[patch]` table not preserving "one major version per source" rule.
  [#7118](https://github.com/rust-lang/cargo/pull/7118)
- Ignore `--remap-path-prefix` flags for the metadata hash in the `cargo
  rustc` command. This was causing the remap settings to inadvertently affect
  symbol names.
  [#7134](https://github.com/rust-lang/cargo/pull/7134)
- Fixed cycle detection in `[patch]` dependencies.
  [#7174](https://github.com/rust-lang/cargo/pull/7174)
- Fixed `cargo new` leaving behind a symlink on Windows when `core.symlinks`
  git config is true. Also adds a number of fixes and updates from upstream
  libgit2.
  [#7176](https://github.com/rust-lang/cargo/pull/7176)
- macOS: Fixed setting the flag to mark the `target` directory to be excluded
  from backups.
  [#7192](https://github.com/rust-lang/cargo/pull/7192)
- Fixed `cargo fix` panicking under some situations involving multi-byte
  characters.
  [#7221](https://github.com/rust-lang/cargo/pull/7221)

### Nightly only
- Added `cargo fix --clippy` which will apply machine-applicable fixes from
  Clippy.
  [#7069](https://github.com/rust-lang/cargo/pull/7069)
- Added `-Z binary-dep-depinfo` flag to add change tracking for binary
  dependencies like the standard library.
  [#7137](https://github.com/rust-lang/cargo/pull/7137)
  [#7219](https://github.com/rust-lang/cargo/pull/7219)
- `cargo clippy-preview` will always run, even if no changes have been made.
  [#7157](https://github.com/rust-lang/cargo/pull/7157)
- Fixed exponential blowup when using `CARGO_BUILD_PIPELINING`.
  [#7062](https://github.com/rust-lang/cargo/pull/7062)
- Fixed passing args to clippy in `cargo clippy-preview`.
  [#7162](https://github.com/rust-lang/cargo/pull/7162)

## Cargo 1.37 (2019-08-15)
[c4fcfb72...9edd0891](https://github.com/rust-lang/cargo/compare/c4fcfb72...9edd0891)

### Added
- Added `doctest` field to `cargo metadata` to determine if a target's
  documentation is tested.
  [#6953](https://github.com/rust-lang/cargo/pull/6953)
  [#6965](https://github.com/rust-lang/cargo/pull/6965)
- 🔥 The [`cargo
  vendor`](https://doc.rust-lang.org/nightly/cargo/commands/cargo-vendor.html)
  command is now built-in to Cargo. This command may be used to create a local
  copy of the sources of all dependencies.
  [#6869](https://github.com/rust-lang/cargo/pull/6869)
- 🔥 The "publish lockfile" feature is now stable. This feature will
  automatically include the `Cargo.lock` file when a package is published if
  it contains a binary executable target. By default, Cargo will ignore
  `Cargo.lock` when installing a package. To force Cargo to use the
  `Cargo.lock` file included in the published package, use `cargo install
  --locked`. This may be useful to ensure that `cargo install` consistently
  reproduces the same result. It may also be useful when a semver-incompatible
  change is accidentally published to a dependency, providing a way to fall
  back to a version that is known to work.
  [#7026](https://github.com/rust-lang/cargo/pull/7026)
- 🔥 The `default-run` feature has been stabilized. This feature allows you to
  specify which binary executable to run by default with `cargo run` when a
  package includes multiple binaries. Set the `default-run` key in the
  `[package]` table in `Cargo.toml` to the name of the binary to use by
  default.
  [#7056](https://github.com/rust-lang/cargo/pull/7056)

### Changed
- `cargo package` now verifies that build scripts do not create empty
  directories.
  [#6973](https://github.com/rust-lang/cargo/pull/6973)
- A warning is now issued if `cargo doc` generates duplicate outputs, which
  causes files to be randomly stomped on. This may happen for a variety of
  reasons (renamed dependencies, multiple versions of the same package,
  packages with renamed libraries, etc.). This is a known bug, which needs
  more work to handle correctly.
  [#6998](https://github.com/rust-lang/cargo/pull/6998)
- Enabling a dependency's feature with `--features foo/bar` will no longer
  compile the current crate with the `foo` feature if `foo` is not an optional
  dependency.
  [#7010](https://github.com/rust-lang/cargo/pull/7010)
- If `--remap-path-prefix` is passed via RUSTFLAGS, it will no longer affect
  the filename metadata hash.
  [#6966](https://github.com/rust-lang/cargo/pull/6966)
- libgit2 has been updated to 0.28.2, which Cargo uses to access git
  repositories. This brings in hundreds of changes and fixes since it was last
  updated in November.
  [#7018](https://github.com/rust-lang/cargo/pull/7018)
- Cargo now supports absolute paths in the dep-info files generated by rustc.
  This is laying the groundwork for [tracking
  binaries](https://github.com/rust-lang/rust/pull/61727), such as libstd, for
  rebuild detection. (Note: this contains a known bug.)
  [#7030](https://github.com/rust-lang/cargo/pull/7030)

### Fixed
- Fixed how zsh completions fetch the list of commands.
  [#6956](https://github.com/rust-lang/cargo/pull/6956)
- "+ debuginfo" is no longer printed in the build summary when `debug` is set
  to 0.
  [#6971](https://github.com/rust-lang/cargo/pull/6971)
- Fixed `cargo doc` with an example configured with `doc = true` to document
  correctly.
  [#7023](https://github.com/rust-lang/cargo/pull/7023)
- Don't fail if a read-only lock cannot be acquired in CARGO_HOME. This helps
  when CARGO_HOME doesn't exist, but `--locked` is used which means CARGO_HOME
  is not needed.
  [#7149](https://github.com/rust-lang/cargo/pull/7149)
- Reverted a change in 1.35 which released jobserver tokens when Cargo blocked
  on a lock file. It caused a deadlock in some situations.
  [#7204](https://github.com/rust-lang/cargo/pull/7204)

### Nightly only
- Added [compiler message
  caching](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#cache-messages).
  The `-Z cache-messages` flag makes cargo cache the compiler output so that
  future runs can redisplay previous warnings.
  [#6933](https://github.com/rust-lang/cargo/pull/6933)
- `-Z mtime-on-use` no longer touches intermediate artifacts.
  [#7050](https://github.com/rust-lang/cargo/pull/7050)

## Cargo 1.36 (2019-07-04)
[6f3e9c36...c4fcfb72](https://github.com/rust-lang/cargo/compare/6f3e9c36...c4fcfb72)

### Added
- Added more detailed documentation on target auto-discovery.
  [#6898](https://github.com/rust-lang/cargo/pull/6898)
- 🔥 Stabilize the `--offline` flag which allows using cargo without a network
  connection.
  [#6934](https://github.com/rust-lang/cargo/pull/6934)
  [#6871](https://github.com/rust-lang/cargo/pull/6871)

### Changed
- `publish = ["crates-io"]` may be added to the manifest to restrict
  publishing to crates.io only.
  [#6838](https://github.com/rust-lang/cargo/pull/6838)
- macOS: Only include the default paths if `DYLD_FALLBACK_LIBRARY_PATH` is not
  set. Also, remove `/lib` from the default set.
  [#6856](https://github.com/rust-lang/cargo/pull/6856)
- `cargo publish` will now exit early if the login token is not available.
  [#6854](https://github.com/rust-lang/cargo/pull/6854)
- HTTP/2 stream errors are now considered "spurious" and will cause a retry.
  [#6861](https://github.com/rust-lang/cargo/pull/6861)
- Setting a feature on a dependency where that feature points to a *required*
  dependency is now an error. Previously it was a warning.
  [#6860](https://github.com/rust-lang/cargo/pull/6860)
- The `registry.index` config value now supports relative `file:` URLs.
  [#6873](https://github.com/rust-lang/cargo/pull/6873)
- macOS: The `.dSYM` directory is now symbolically linked next to example
  binaries without the metadata hash so that debuggers can find it.
  [#6891](https://github.com/rust-lang/cargo/pull/6891)
- The default `Cargo.toml` template for now projects now includes a comment
  providing a link to the documentation.
  [#6881](https://github.com/rust-lang/cargo/pull/6881)
- Some improvements to the wording of the crate download summary.
  [#6916](https://github.com/rust-lang/cargo/pull/6916)
  [#6920](https://github.com/rust-lang/cargo/pull/6920)
- ✨ Changed `RUST_LOG` environment variable to `CARGO_LOG` so that user code
  that uses the `log` crate will not display cargo's debug output.
  [#6918](https://github.com/rust-lang/cargo/pull/6918)
- `Cargo.toml` is now always included when packaging, even if it is not listed
  in `package.include`.
  [#6925](https://github.com/rust-lang/cargo/pull/6925)
- Package include/exclude values now use gitignore patterns instead of glob
  patterns. [#6924](https://github.com/rust-lang/cargo/pull/6924)
- Provide a better error message when crates.io times out. Also improve error
  messages with other HTTP response codes.
  [#6936](https://github.com/rust-lang/cargo/pull/6936)

### Performance
- Resolver performance improvements for some cases.
  [#6853](https://github.com/rust-lang/cargo/pull/6853)
- Optimized how cargo reads the index JSON files by caching the results.
  [#6880](https://github.com/rust-lang/cargo/pull/6880)
  [#6912](https://github.com/rust-lang/cargo/pull/6912)
  [#6940](https://github.com/rust-lang/cargo/pull/6940)
- Various performance improvements.
  [#6867](https://github.com/rust-lang/cargo/pull/6867)

### Fixed
- More carefully track the on-disk fingerprint information for dependencies.
  This can help in some rare cases where the build is interrupted and
  restarted. [#6832](https://github.com/rust-lang/cargo/pull/6832)
- `cargo run` now correctly passes non-UTF8 arguments to the child process.
  [#6849](https://github.com/rust-lang/cargo/pull/6849)
- Fixed bash completion to run on bash 3.2, the stock version in macOS.
  [#6905](https://github.com/rust-lang/cargo/pull/6905)
- Various fixes and improvements to zsh completion.
  [#6926](https://github.com/rust-lang/cargo/pull/6926)
  [#6929](https://github.com/rust-lang/cargo/pull/6929)
- Fix `cargo update` ignoring `-p` arguments if the `Cargo.lock` file was
  missing.
  [#6904](https://github.com/rust-lang/cargo/pull/6904)

### Nightly only
- Added [`-Z install-upgrade`
  feature](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#install-upgrade)
  to track details about installed crates and to update them if they are
  out-of-date. [#6798](https://github.com/rust-lang/cargo/pull/6798)
- Added the [`public-dependency`
  feature](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#public-dependency)
  which allows tracking public versus private dependencies.
  [#6772](https://github.com/rust-lang/cargo/pull/6772)
- Added build pipelining via the `build.pipelining` config
  option (`CARGO_BUILD_PIPELINING` env var).
  [#6883](https://github.com/rust-lang/cargo/pull/6883)
- The `publish-lockfile` feature has had some significant changes. The default
  is now `true`, the `Cargo.lock` will always be published for binary crates.
  The `Cargo.lock` is now regenerated during publishing. `cargo install` now
  ignores the `Cargo.lock` file by default, and requires `--locked` to use the
  lock file. Warnings have been added if yanked dependencies are detected.
  [#6840](https://github.com/rust-lang/cargo/pull/6840)

## Cargo 1.35 (2019-05-23)
[6789d8a0...6f3e9c36](https://github.com/rust-lang/cargo/compare/6789d8a0...6f3e9c36)

### Added
- Added the `rustc-cdylib-link-arg` key for build scripts to specify linker
  arguments for cdylib crates.
  [#6298](https://github.com/rust-lang/cargo/pull/6298)

### Changed
- When passing a test filter, such as `cargo test foo`, don't build examples
  (unless they set `test = true`).
  [#6683](https://github.com/rust-lang/cargo/pull/6683)
- Forward the `--quiet` flag from `cargo test` to the libtest harness so that
  tests are actually quiet.
  [#6358](https://github.com/rust-lang/cargo/pull/6358)
- The verification step in `cargo package` that checks if any files are
  modified is now stricter. It uses a hash of the contents instead of checking
  filesystem mtimes. It also checks *all* files in the package.
  [#6740](https://github.com/rust-lang/cargo/pull/6740)
- Jobserver tokens are now released whenever Cargo blocks on a file lock.
  [#6748](https://github.com/rust-lang/cargo/pull/6748)
- Issue a warning for a previous bug in the TOML parser that allowed multiple
  table headers with the same name.
  [#6761](https://github.com/rust-lang/cargo/pull/6761)
- Removed the `CARGO_PKG_*` environment variables from the metadata hash and
  added them to the fingerprint instead. This means that when these values
  change, stale artifacts are not left behind. Also added the "repository"
  value to the fingerprint.
  [#6785](https://github.com/rust-lang/cargo/pull/6785)
- `cargo metadata` no longer shows a `null` field for a dependency without a
  library in `resolve.nodes.deps`. The dependency is no longer shown.
  [#6534](https://github.com/rust-lang/cargo/pull/6534)
- `cargo new` will no longer include an email address in the `authors` field
  if it is set to the empty string.
  [#6802](https://github.com/rust-lang/cargo/pull/6802)
- `cargo doc --open` now works when documenting multiple packages.
  [#6803](https://github.com/rust-lang/cargo/pull/6803)
- `cargo install --path P` now loads the `.cargo/config` file from the
  directory P. [#6805](https://github.com/rust-lang/cargo/pull/6805)
- Using semver metadata in a version requirement (such as `1.0.0+1234`) now
  issues a warning that it is ignored.
  [#6806](https://github.com/rust-lang/cargo/pull/6806)
- `cargo install` now rejects certain combinations of flags where some flags
  would have been ignored.
  [#6801](https://github.com/rust-lang/cargo/pull/6801)
- Resolver performance improvements for some cases.
  [#6776](https://github.com/rust-lang/cargo/pull/6776)

### Fixed
- Fixed running separate commands (such as `cargo build` then `cargo test`)
  where the second command could use stale results from a build script.
  [#6720](https://github.com/rust-lang/cargo/pull/6720)
- Fixed `cargo fix` not working properly if a `.gitignore` file that matched
  the root package directory.
  [#6767](https://github.com/rust-lang/cargo/pull/6767)
- Fixed accidentally compiling a lib multiple times if `panic=unwind` was set
  in a profile. [#6781](https://github.com/rust-lang/cargo/pull/6781)
- Paths to JSON files in `build.target` config value are now canonicalized to
  fix building dependencies.
  [#6778](https://github.com/rust-lang/cargo/pull/6778)
- Fixed re-running a build script if its compilation was interrupted (such as
  if it is killed). [#6782](https://github.com/rust-lang/cargo/pull/6782)
- Fixed `cargo new` initializing a fossil repo.
  [#6792](https://github.com/rust-lang/cargo/pull/6792)
- Fixed supporting updating a git repo that has a force push when using the
  `git-fetch-with-cli` feature. `git-fetch-with-cli` also shows more error
  information now when it fails.
  [#6800](https://github.com/rust-lang/cargo/pull/6800)
- `--example` binaries built for the WASM target are fixed to no longer
  include a metadata hash in the filename, and are correctly emitted in the
  `compiler-artifact` JSON message.
  [#6812](https://github.com/rust-lang/cargo/pull/6812)

### Nightly only
- `cargo clippy-preview` is now a built-in cargo command.
  [#6759](https://github.com/rust-lang/cargo/pull/6759)
- The `build-override` profile setting now includes proc-macros and their
  dependencies.
  [#6811](https://github.com/rust-lang/cargo/pull/6811)
- Optional and target dependencies now work better with `-Z offline`.
  [#6814](https://github.com/rust-lang/cargo/pull/6814)

## Cargo 1.34 (2019-04-11)
[f099fe94...6789d8a0](https://github.com/rust-lang/cargo/compare/f099fe94...6789d8a0)

### Added
- 🔥 Stabilized support for [alternate
  registries](https://doc.rust-lang.org/1.34.0/cargo/reference/registries.html).
  [#6654](https://github.com/rust-lang/cargo/pull/6654)
- Added documentation on using builds.sr.ht Continuous Integration with Cargo.
  [#6565](https://github.com/rust-lang/cargo/pull/6565)
- `Cargo.lock` now includes a comment at the top that it is `@generated`.
  [#6548](https://github.com/rust-lang/cargo/pull/6548)
- Azure DevOps badges are now supported.
  [#6264](https://github.com/rust-lang/cargo/pull/6264)
- Added a warning if `--exclude` flag specifies an unknown package.
  [#6679](https://github.com/rust-lang/cargo/pull/6679)

### Changed
- `cargo test --doc --no-run` doesn't do anything, so it now displays an error
  to that effect. [#6628](https://github.com/rust-lang/cargo/pull/6628)
- Various updates to bash completion: add missing options and commands,
  support libtest completions, use rustup for `--target` completion, fallback
  to filename completion, fix editing the command line.
  [#6644](https://github.com/rust-lang/cargo/pull/6644)
- Publishing a crate with a `[patch]` section no longer generates an error.
  The `[patch]` section is removed from the manifest before publishing.
  [#6535](https://github.com/rust-lang/cargo/pull/6535)
- `build.incremental = true` config value is now treated the same as
  `CARGO_INCREMENTAL=1`, previously it was ignored.
  [#6688](https://github.com/rust-lang/cargo/pull/6688)
- Errors from a registry are now always displayed regardless of the HTTP
  response code. [#6771](https://github.com/rust-lang/cargo/pull/6771)

### Fixed
- Fixed bash completion for `cargo run --example`.
  [#6578](https://github.com/rust-lang/cargo/pull/6578)
- Fixed a race condition when using a *local* registry and running multiple
  cargo commands at the same time that build the same crate.
  [#6591](https://github.com/rust-lang/cargo/pull/6591)
- Fixed some flickering and excessive updates of the progress bar.
  [#6615](https://github.com/rust-lang/cargo/pull/6615)
- Fixed a hang when using a git credential helper that returns incorrect
  credentials. [#6681](https://github.com/rust-lang/cargo/pull/6681)
- Fixed resolving yanked crates with a local registry.
  [#6750](https://github.com/rust-lang/cargo/pull/6750)

### Nightly only
- Added `-Z mtime-on-use` flag to cause the mtime to be updated on the
  filesystem when a crate is used. This is intended to be able to track stale
  artifacts in the future for cleaning up unused files.
  [#6477](https://github.com/rust-lang/cargo/pull/6477)
  [#6573](https://github.com/rust-lang/cargo/pull/6573)
- Added experimental `-Z dual-proc-macros` to build proc macros for both the
  host and the target.
  [#6547](https://github.com/rust-lang/cargo/pull/6547)

## Cargo 1.33 (2019-02-28)
[8610973a...f099fe94](https://github.com/rust-lang/cargo/compare/8610973a...f099fe94)

### Added
- `compiler-artifact` JSON messages now include an `"executable"` key which
  includes the path to the executable that was built.
  [#6363](https://github.com/rust-lang/cargo/pull/6363)
- The man pages have been rewritten, and are now published with the web
  documentation. [#6405](https://github.com/rust-lang/cargo/pull/6405)
- `cargo login` now displays a confirmation after saving the token.
  [#6466](https://github.com/rust-lang/cargo/pull/6466)
- A warning is now emitted if a `[patch]` entry does not match any package.
  [#6470](https://github.com/rust-lang/cargo/pull/6470)
- `cargo metadata` now includes the `links` key for a package.
  [#6480](https://github.com/rust-lang/cargo/pull/6480)
- "Very verbose" output with `-vv` now displays the environment variables that
  cargo sets when it runs a process.
  [#6492](https://github.com/rust-lang/cargo/pull/6492)
- `--example`, `--bin`, `--bench`, or `--test` without an argument now lists
  the available targets for those options.
  [#6505](https://github.com/rust-lang/cargo/pull/6505)
- Windows: If a process fails with an extended status exit code, a
  human-readable name for the code is now displayed.
  [#6532](https://github.com/rust-lang/cargo/pull/6532)
- Added `--features`, `--no-default-features`, and `--all-features` flags to
  the `cargo package` and `cargo publish` commands to use the given features
  when verifying the package.
  [#6453](https://github.com/rust-lang/cargo/pull/6453)

### Changed
- If `cargo fix` fails to compile the fixed code, the rustc errors are now
  displayed on the console.
  [#6419](https://github.com/rust-lang/cargo/pull/6419)
- Hide the `--host` flag from `cargo login`, it is unused.
  [#6466](https://github.com/rust-lang/cargo/pull/6466)
- Build script fingerprints now include the rustc version.
  [#6473](https://github.com/rust-lang/cargo/pull/6473)
- macOS: Switched to setting `DYLD_FALLBACK_LIBRARY_PATH` instead of
  `DYLD_LIBRARY_PATH`. [#6355](https://github.com/rust-lang/cargo/pull/6355)
- `RUSTFLAGS` is now included in the metadata hash, meaning that changing
  the flags will not overwrite previously built files.
  [#6503](https://github.com/rust-lang/cargo/pull/6503)
- When updating the crate graph, unrelated yanked crates were erroneously
  removed. They are now kept at their original version if possible. This was
  causing unrelated packages to be downgraded during `cargo update -p
  somecrate`. [#5702](https://github.com/rust-lang/cargo/issues/5702)
- TOML files now support the [0.5 TOML
  syntax](https://github.com/toml-lang/toml/blob/master/CHANGELOG.md#050--2018-07-11).

### Fixed
- `cargo fix` will now ignore suggestions that modify multiple files.
  [#6402](https://github.com/rust-lang/cargo/pull/6402)
- `cargo fix` will now only fix one target at a time, to deal with targets
  which share the same source files.
  [#6434](https://github.com/rust-lang/cargo/pull/6434)
- Fixed bash completion showing the list of cargo commands.
  [#6461](https://github.com/rust-lang/cargo/issues/6461)
- `cargo init` will now avoid creating duplicate entries in `.gitignore`
  files. [#6521](https://github.com/rust-lang/cargo/pull/6521)
- Builds now attempt to detect if a file is modified in the middle of a
  compilation, allowing you to build again and pick up the new changes. This
  is done by keeping track of when the compilation *starts* not when it
  finishes. Also, [#5919](https://github.com/rust-lang/cargo/pull/5919) was
  reverted, meaning that cargo does *not* treat equal filesystem mtimes as
  requiring a rebuild. [#6484](https://github.com/rust-lang/cargo/pull/6484)

### Nightly only
- Allow using registry *names* in `[patch]` tables instead of just URLs.
  [#6456](https://github.com/rust-lang/cargo/pull/6456)
- `cargo metadata` added the `registry` key for dependencies.
  [#6500](https://github.com/rust-lang/cargo/pull/6500)
- Registry names are now restricted to the same style as
  package names (alphanumeric, `-` and `_` characters).
  [#6469](https://github.com/rust-lang/cargo/pull/6469)
- `cargo login` now displays the `/me` URL from the registry config.
  [#6466](https://github.com/rust-lang/cargo/pull/6466)
- `cargo login --registry=NAME` now supports interactive input for the token.
  [#6466](https://github.com/rust-lang/cargo/pull/6466)
- Registries may now elide the `api` key from `config.json` to indicate they
  do not support API access.
  [#6466](https://github.com/rust-lang/cargo/pull/6466)
- Fixed panic when using `--message-format=json` with metabuild.
  [#6432](https://github.com/rust-lang/cargo/pull/6432)
- Fixed detection of publishing to crates.io when using alternate registries.
  [#6525](https://github.com/rust-lang/cargo/pull/6525)

## Cargo 1.32 (2019-01-17)
[339d9f9c...8610973a](https://github.com/rust-lang/cargo/compare/339d9f9c...8610973a)

### Added
- Registries may now display warnings after a successful publish.
  [#6303](https://github.com/rust-lang/cargo/pull/6303)
- Added a [glossary](https://doc.rust-lang.org/cargo/appendix/glossary.html)
  to the documentation. [#6321](https://github.com/rust-lang/cargo/pull/6321)
- Added the alias `c` for `cargo check`.
  [#6218](https://github.com/rust-lang/cargo/pull/6218)

### Changed
- 🔥 HTTP/2 multiplexing is now enabled by default. The `http.multiplexing`
  config value may be used to disable it.
  [#6271](https://github.com/rust-lang/cargo/pull/6271)
- Use ANSI escape sequences to clear lines instead of spaces.
  [#6233](https://github.com/rust-lang/cargo/pull/6233)
- Disable git templates when checking out git dependencies, which can cause
  problems. [#6252](https://github.com/rust-lang/cargo/pull/6252)
- Include the `--update-head-ok` git flag when using the
  `net.git-fetch-with-cli` option. This can help prevent failures when
  fetching some repositories.
  [#6250](https://github.com/rust-lang/cargo/pull/6250)
- When extracting a crate during the verification step of `cargo package`, the
  filesystem mtimes are no longer set, which was failing on some rare
  filesystems. [#6257](https://github.com/rust-lang/cargo/pull/6257)
- `crate-type = ["proc-macro"]` is now treated the same as `proc-macro = true`
  in `Cargo.toml`. [#6256](https://github.com/rust-lang/cargo/pull/6256)
- An error is raised if `dependencies`, `features`, `target`, or `badges` is
  set in a virtual workspace. Warnings are displayed if `replace` or `patch`
  is used in a workspace member.
  [#6276](https://github.com/rust-lang/cargo/pull/6276)
- Improved performance of the resolver in some cases.
  [#6283](https://github.com/rust-lang/cargo/pull/6283)
  [#6366](https://github.com/rust-lang/cargo/pull/6366)
- `.rmeta` files are no longer hard-linked into the base target directory
  (`target/debug`). [#6292](https://github.com/rust-lang/cargo/pull/6292)
- A warning is issued if multiple targets are built with the same output
  filenames. [#6308](https://github.com/rust-lang/cargo/pull/6308)
- When using `cargo build` (without `--release`) benchmarks are now built
  using the "test" profile instead of "bench". This makes it easier to debug
  benchmarks, and avoids confusing behavior.
  [#6309](https://github.com/rust-lang/cargo/pull/6309)
- User aliases may now override built-in aliases (`b`, `r`, `t`, and `c`).
  [#6259](https://github.com/rust-lang/cargo/pull/6259)
- Setting `autobins=false` now disables auto-discovery of inferred targets.
  [#6329](https://github.com/rust-lang/cargo/pull/6329)
- `cargo verify-project` will now fail on stable if the project uses unstable
  features. [#6326](https://github.com/rust-lang/cargo/pull/6326)
- Platform targets with an internal `.` within the name are now allowed.
  [#6255](https://github.com/rust-lang/cargo/pull/6255)
- `cargo clean --release` now only deletes the release directory.
  [#6349](https://github.com/rust-lang/cargo/pull/6349)

### Fixed
- Avoid adding extra angle brackets in email address for `cargo new`.
  [#6243](https://github.com/rust-lang/cargo/pull/6243)
- The progress bar is disabled if the CI environment variable is set.
  [#6281](https://github.com/rust-lang/cargo/pull/6281)
- Avoid retaining all rustc output in memory.
  [#6289](https://github.com/rust-lang/cargo/pull/6289)
- If JSON parsing fails, and rustc exits nonzero, don't lose the parse failure
  message. [#6290](https://github.com/rust-lang/cargo/pull/6290)
- Fixed renaming a project directory with build scripts.
  [#6328](https://github.com/rust-lang/cargo/pull/6328)
- Fixed `cargo run --example NAME` to work correctly if the example sets
  `crate_type = ["bin"]`.
  [#6330](https://github.com/rust-lang/cargo/pull/6330)
- Fixed issue with `cargo package` git discovery being too aggressive. The
  `--allow-dirty` now completely disables the git repo checks.
  [#6280](https://github.com/rust-lang/cargo/pull/6280)
- Fixed build change tracking for `[patch]` deps which resulted in `cargo
  build` rebuilding when it shouldn't.
  [#6493](https://github.com/rust-lang/cargo/pull/6493)

### Nightly only
- Allow usernames in registry URLs.
  [#6242](https://github.com/rust-lang/cargo/pull/6242)
- Added `"compile_mode"` key to the build-plan JSON structure to be able to
  distinguish running a custom build script versus compiling the build script.
  [#6331](https://github.com/rust-lang/cargo/pull/6331)
- `--out-dir` no longer copies over build scripts.
  [#6300](https://github.com/rust-lang/cargo/pull/6300)

## Cargo 1.31 (2018-12-06)
[36d96825...339d9f9c](https://github.com/rust-lang/cargo/compare/36d96825...339d9f9c)

### Added
- 🔥 Stabilized support for the 2018 edition.
  [#5984](https://github.com/rust-lang/cargo/pull/5984)
  [#5989](https://github.com/rust-lang/cargo/pull/5989)
- 🔥 Added the ability to [rename
  dependencies](https://doc.rust-lang.org/1.31.0/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml)
  in Cargo.toml. [#6319](https://github.com/rust-lang/cargo/pull/6319)
- 🔥 Added support for HTTP/2 pipelining and multiplexing. Set the
  `http.multiplexing` config value to enable.
  [#6005](https://github.com/rust-lang/cargo/pull/6005)
- Added `http.debug` configuration value to debug HTTP connections. Use
  `CARGO_HTTP_DEBUG=true RUST_LOG=cargo::ops::registry cargo build` to display
  the debug information. [#6166](https://github.com/rust-lang/cargo/pull/6166)
- `CARGO_PKG_REPOSITORY` environment variable is set with the repository value
  from `Cargo.toml` when building .
  [#6096](https://github.com/rust-lang/cargo/pull/6096)

### Changed
- `cargo test --doc` now rejects other flags instead of ignoring them.
  [#6037](https://github.com/rust-lang/cargo/pull/6037)
- `cargo install` ignores `~/.cargo/config`.
  [#6026](https://github.com/rust-lang/cargo/pull/6026)
- `cargo version --verbose` is now the same as `cargo -vV`.
  [#6076](https://github.com/rust-lang/cargo/pull/6076)
- Comments at the top of `Cargo.lock` are now preserved.
  [#6181](https://github.com/rust-lang/cargo/pull/6181)
- When building in "very verbose" mode (`cargo build -vv`), build script
  output is prefixed with the package name and version, such as `[foo 0.0.1]`.
  [#6164](https://github.com/rust-lang/cargo/pull/6164)
- If `cargo fix --broken-code` fails to compile after fixes have been applied,
  the files are no longer reverted and are left in their broken state.
  [#6316](https://github.com/rust-lang/cargo/pull/6316)

### Fixed
- Windows: Pass Ctrl-C to the process with `cargo run`.
  [#6004](https://github.com/rust-lang/cargo/pull/6004)
- macOS: Fix bash completion.
  [#6038](https://github.com/rust-lang/cargo/pull/6038)
- Support arbitrary toolchain names when completing `+toolchain` in bash
  completion. [#6038](https://github.com/rust-lang/cargo/pull/6038)
- Fixed edge cases in the resolver, when backtracking on failed dependencies.
  [#5988](https://github.com/rust-lang/cargo/pull/5988)
- Fixed `cargo test --all-targets` running lib tests three times.
  [#6039](https://github.com/rust-lang/cargo/pull/6039)
- Fixed publishing renamed dependencies to crates.io.
  [#5993](https://github.com/rust-lang/cargo/pull/5993)
- Fixed `cargo install` on a git repo with multiple binaries.
  [#6060](https://github.com/rust-lang/cargo/pull/6060)
- Fixed deeply nested JSON emitted by rustc being lost.
  [#6081](https://github.com/rust-lang/cargo/pull/6081)
- Windows: Fix locking msys terminals to 60 characters.
  [#6122](https://github.com/rust-lang/cargo/pull/6122)
- Fixed renamed dependencies with dashes.
  [#6140](https://github.com/rust-lang/cargo/pull/6140)
- Fixed linking against the wrong dylib when the dylib existed in both
  `target/debug` and `target/debug/deps`.
  [#6167](https://github.com/rust-lang/cargo/pull/6167)
- Fixed some unnecessary recompiles when `panic=abort` is used.
  [#6170](https://github.com/rust-lang/cargo/pull/6170)

### Nightly only
- Added `--registry` flag to `cargo install`.
  [#6128](https://github.com/rust-lang/cargo/pull/6128)
- Added `registry.default` configuration value to specify the
  default registry to use if `--registry` flag is not passed.
  [#6135](https://github.com/rust-lang/cargo/pull/6135)
- Added `--registry` flag to `cargo new` and `cargo init`.
  [#6135](https://github.com/rust-lang/cargo/pull/6135)

## Cargo 1.30 (2018-10-25)
[524a578d...36d96825](https://github.com/rust-lang/cargo/compare/524a578d...36d96825)

### Added
- 🔥 Added an animated progress bar shows progress during building.
  [#5995](https://github.com/rust-lang/cargo/pull/5995/)
- Added `resolve.nodes.deps` key to `cargo metadata`, which includes more
  information about resolved dependencies, and properly handles renamed
  dependencies. [#5871](https://github.com/rust-lang/cargo/pull/5871)
- When creating a package, provide more detail with `-v` when failing to
  discover if files are dirty in a git repository. Also fix a problem with
  discovery on Windows. [#5858](https://github.com/rust-lang/cargo/pull/5858)
- Filters like `--bin`, `--test`, `--example`, `--bench`, or `--lib` can be
  used in a workspace without selecting a specific package.
  [#5873](https://github.com/rust-lang/cargo/pull/5873)
- `cargo run` can be used in a workspace without selecting a specific package.
  [#5877](https://github.com/rust-lang/cargo/pull/5877)
- `cargo doc --message-format=json` now outputs JSON messages from rustdoc.
  [#5878](https://github.com/rust-lang/cargo/pull/5878)
- Added `--message-format=short` to show one-line messages.
  [#5879](https://github.com/rust-lang/cargo/pull/5879)
- Added `.cargo_vcs_info.json` file to `.crate` packages that captures the
  current git hash. [#5886](https://github.com/rust-lang/cargo/pull/5886)
- Added `net.git-fetch-with-cli` configuration option to use the `git`
  executable to fetch repositories instead of using the built-in libgit2
  library. [#5914](https://github.com/rust-lang/cargo/pull/5914)
- Added `required-features` to `cargo metadata`.
  [#5902](https://github.com/rust-lang/cargo/pull/5902)
- `cargo uninstall` within a package will now uninstall that package.
  [#5927](https://github.com/rust-lang/cargo/pull/5927)
- Added `--allow-staged` flag to `cargo fix` to allow it to run if files are
  staged in git. [#5943](https://github.com/rust-lang/cargo/pull/5943)
- Added `net.low-speed-limit` config value, and also honor `net.timeout` for
  http operations. [#5957](https://github.com/rust-lang/cargo/pull/5957)
- Added `--edition` flag to `cargo new`.
  [#5984](https://github.com/rust-lang/cargo/pull/5984)
- Temporarily stabilized 2018 edition support for the duration of the beta.
  [#5984](https://github.com/rust-lang/cargo/pull/5984)
  [#5989](https://github.com/rust-lang/cargo/pull/5989)
- Added support for `target.'cfg(…)'.runner` config value to specify the
  run/test/bench runner for targets that use config expressions.
  [#5959](https://github.com/rust-lang/cargo/pull/5959)

### Changed
- Windows: `cargo run` will not kill child processes when the main process
  exits. [#5887](https://github.com/rust-lang/cargo/pull/5887)
- Switched to the `opener` crate to open a web browser with `cargo doc
  --open`. This should more reliably select the system-preferred browser on
  all platforms. [#5888](https://github.com/rust-lang/cargo/pull/5888)
- Equal file mtimes now cause a target to be rebuilt. Previously only if files
  were strictly *newer* than the last build would it cause a rebuild.
  [#5919](https://github.com/rust-lang/cargo/pull/5919)
- Ignore `build.target` config value when running `cargo install`.
  [#5874](https://github.com/rust-lang/cargo/pull/5874)
- Ignore `RUSTC_WRAPPER` for `cargo fix`.
  [#5983](https://github.com/rust-lang/cargo/pull/5983)
- Ignore empty `RUSTC_WRAPPER`.
  [#5985](https://github.com/rust-lang/cargo/pull/5985)

### Fixed
- Fixed error when creating a package with an edition field in `Cargo.toml`.
  [#5908](https://github.com/rust-lang/cargo/pull/5908)
- More consistently use relative paths for path dependencies in a workspace.
  [#5935](https://github.com/rust-lang/cargo/pull/5935)
- `cargo fix` now always runs, even if it was run previously.
  [#5944](https://github.com/rust-lang/cargo/pull/5944)
- Windows: Attempt to more reliably detect terminal width. msys-based
  terminals are forced to 60 characters wide.
  [#6010](https://github.com/rust-lang/cargo/pull/6010)
- Allow multiple target flags with `cargo doc --document-private-items`.
  [6022](https://github.com/rust-lang/cargo/pull/6022)

### Nightly only
- Added
  [metabuild](https://doc.rust-lang.org/1.30.0/cargo/reference/unstable.html#metabuild).
  [#5628](https://github.com/rust-lang/cargo/pull/5628)
