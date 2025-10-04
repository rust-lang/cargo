//! Support for nightly features in Cargo itself.
//!
//! This file is the version of `feature_gate.rs` in upstream Rust for Cargo
//! itself and is intended to be the avenue for which new features in Cargo are
//! gated by default and then eventually stabilized. All known stable and
//! unstable features are tracked in this file.
//!
//! If you're reading this then you're likely interested in adding a feature to
//! Cargo, and the good news is that it shouldn't be too hard! First determine
//! how the feature should be gated:
//!
//! * Error when the feature is used without the gate
//!   * Required if ignoring the feature violates the users intent in non-superficial ways
//!   * A low-effort / safe way to protect the user from being broken if the format of the feature changes in
//!     incompatible was (can be worked around)
//!   * Good for: CLI (gate: `-Zunstable-options` or `-Z` if combined with other changes), `Cargo.toml` (gate: `cargo-features`)
//! * Warn that the feature is ignored due to lack of the gate
//!   * For if you could opt-in to the unimplemented feature on Cargo today and Cargo would
//!     operate just fine
//!   * If gate is not enabled, prefer to warn if the format of the feature is incompatible
//!     (instead of error or ignore)
//!   * Good for: `Cargo.toml`, `.cargo/config.toml`, `config.json` index file (gate: `-Z`)
//! * Ignore the feature that is used without a gate
//!   * For when ignoring the feature has so little impact that annoying the user is not worth it
//!     (e.g. a config field that changes Cargo's terminal output)
//!   * For behavior changes without an interface (e.g. the resolver)
//!   * Good for: `.cargo/config.toml`, `config.json` index file (gate: `-Z`)
//!
//! For features that touch multiple parts of Cargo, multiple feature gating strategies (error,
//! warn, ignore) and mechanisms (`-Z`, `cargo-features`) may be used.
//!
//! When adding new tests for your feature, usually the tests should go into a
//! new module of the testsuite named after the feature. See
//! <https://doc.crates.io/contrib/tests/writing.html> for more information on
//! writing tests. Particularly, check out the "Testing Nightly Features"
//! section for testing unstable features. Be sure to test the feature gate itself.
//!
//! After you have added your feature, be sure to update the unstable
//! documentation at `src/doc/src/reference/unstable.md` to include a short
//! description of how to use your new feature.
//!
//! And hopefully that's it!
//!
//! ## `cargo-features`
//!
//! The steps for adding new Cargo.toml syntax are:
//!
//! 1. Add the cargo-features unstable gate. Search the code below for "look here" to
//!    find the [`features!`] macro invocation and add your feature to the list.
//!
//! 2. Update the Cargo.toml parsing code to handle your new feature.
//!
//! 3. Wherever you added the new parsing code, call
//!    `features.require(Feature::my_feature_name())?` if the new syntax is
//!    used. This will return an error if the user hasn't listed the feature
//!    in `cargo-features` or this is not the nightly channel.
//!
//! ## `-Z unstable-options`
//!
//! `-Z unstable-options` is intended to force the user to opt-in to new CLI
//! flags, options, and new subcommands.
//!
//! The steps to add a new command-line option are:
//!
//! 1. Add the option to the CLI parsing code. In the help text, be sure to
//!    include `(unstable)` to note that this is an unstable option.
//! 2. Where the CLI option is loaded, be sure to call
//!    [`CliUnstable::fail_if_stable_opt`]. This will return an error if `-Z
//!    unstable options` was not passed.
//!
//! ## `-Z` options
//!
//! New `-Z` options cover all other functionality that isn't covered with
//! `cargo-features` or `-Z unstable-options`.
//!
//! The steps to add a new `-Z` option are:
//!
//! 1. Add the option to the [`CliUnstable`] struct in the macro invocation of
//!    [`unstable_cli_options!`]. Flags can take an optional value if you want.
//! 2. Update the [`CliUnstable::add`] function to parse the flag.
//! 3. Wherever the new functionality is implemented, call
//!    [`GlobalContext::cli_unstable`] to get an instance of [`CliUnstable`]
//!    and check if the option has been enabled on the [`CliUnstable`] instance.
//!    Nightly gating is already handled, so no need to worry about that.
//!    If warning when feature is used without the gate, be sure to gracefully degrade (with a
//!    warning) when the `Cargo.toml` / `.cargo/config.toml` field usage doesn't match the
//!    schema.
//! 4. For any `Cargo.toml` fields, strip them in [`prepare_for_publish`] if the gate isn't set
//!
//! ## Stabilization
//!
//! For the stabilization process, see
//! <https://doc.crates.io/contrib/process/unstable.html#stabilization>.
//!
//! The steps for stabilizing are roughly:
//!
//! 1. Update the feature to be stable, based on the kind of feature:
//!   1. `cargo-features`: Change the feature to `stable` in the [`features!`]
//!      macro invocation below, and include the version and a URL for the
//!      documentation.
//!   2. `-Z unstable-options`: Find the call to [`fail_if_stable_opt`] and
//!      remove it. Be sure to update the man pages if necessary.
//!   3. `-Z` flag: Change the parsing code in [`CliUnstable::add`] to call
//!      `stabilized_warn` or `stabilized_err` and remove the field from
//!      [`CliUnstable`]. Remove the `(unstable)` note in the clap help text if
//!      necessary.
//! 2. Remove `masquerade_as_nightly_cargo` from any tests, and remove
//!    `cargo-features` from `Cargo.toml` test files if any. You can
//!     quickly find what needs to be removed by searching for the name
//!     of the feature, e.g. `print_im_a_teapot`
//! 3. Update the docs in unstable.md to move the section to the bottom
//!    and summarize it similar to the other entries. Update the rest of the
//!    documentation to add the new feature.
//!
//! [`GlobalContext::cli_unstable`]: crate::util::context::GlobalContext::cli_unstable
//! [`fail_if_stable_opt`]: CliUnstable::fail_if_stable_opt
//! [`features!`]: macro.features.html
//! [`unstable_cli_options!`]: macro.unstable_cli_options.html
//! [`prepare_for_publish`]: crate::util::toml::prepare_for_publish

use std::collections::BTreeSet;
use std::env;
use std::fmt::{self, Write};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Error, bail};
use cargo_util::ProcessBuilder;
use serde::{Deserialize, Serialize};

use crate::GlobalContext;
use crate::core::resolver::ResolveBehavior;
use crate::util::errors::CargoResult;
use crate::util::indented_lines;

pub const SEE_CHANNELS: &str = "See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information \
     about Rust release channels.";

/// Value of [`allow-features`](CliUnstable::allow_features)
pub type AllowFeatures = BTreeSet<String>;

/// The edition of the compiler ([RFC 2052])
///
/// The following sections will guide you how to add and stabilize an edition.
///
/// ## Adding a new edition
///
/// - Add the next edition to the enum.
/// - Update every match expression that now fails to compile.
/// - Update the [`FromStr`] impl.
/// - Update [`CLI_VALUES`] to include the new edition.
/// - Set [`LATEST_UNSTABLE`] to Some with the new edition.
/// - Update the shell completion files.
/// - Update any failing tests (hopefully there are very few).
///
/// ## Stabilization instructions
///
/// - Set [`LATEST_UNSTABLE`] to None.
/// - Set [`LATEST_STABLE`] to the new version.
/// - Update [`is_stable`] to `true`.
/// - Set [`first_version`] to the version it will be released.
/// - Update any tests that are affected.
/// - Update the man page for the `--edition` flag.
/// - Update the documentation:
///   - Update any features impacted by the edition.
///   - Update manifest.md#the-edition-field.
///   - Update the `--edition` flag (options-new.md).
///   - Rebuild man pages.
///
/// [RFC 2052]: https://rust-lang.github.io/rfcs/2052-epochs.html
/// [`FromStr`]: Edition::from_str
/// [`CLI_VALUES`]: Edition::CLI_VALUES
/// [`LATEST_UNSTABLE`]: Edition::LATEST_UNSTABLE
/// [`LATEST_STABLE`]: Edition::LATEST_STABLE
/// [`first_version`]: Edition::first_version
/// [`is_stable`]: Edition::is_stable
/// [`toml`]: crate::util::toml
/// [`features!`]: macro.features.html
#[derive(
    Default, Clone, Copy, Debug, Hash, PartialOrd, Ord, Eq, PartialEq, Serialize, Deserialize,
)]
pub enum Edition {
    /// The 2015 edition
    #[default]
    Edition2015,
    /// The 2018 edition
    Edition2018,
    /// The 2021 edition
    Edition2021,
    /// The 2024 edition
    Edition2024,
    /// The future edition (permanently unstable)
    EditionFuture,
}

impl Edition {
    /// The latest edition that is unstable.
    ///
    /// This is `None` if there is no next unstable edition.
    ///
    /// Note that this does *not* include "future" since this is primarily
    /// used for tests that need to step between stable and unstable.
    pub const LATEST_UNSTABLE: Option<Edition> = None;
    /// The latest stable edition.
    pub const LATEST_STABLE: Edition = Edition::Edition2024;
    pub const ALL: &'static [Edition] = &[
        Self::Edition2015,
        Self::Edition2018,
        Self::Edition2021,
        Self::Edition2024,
        Self::EditionFuture,
    ];
    /// Possible values allowed for the `--edition` CLI flag.
    ///
    /// This requires a static value due to the way clap works, otherwise I
    /// would have built this dynamically.
    ///
    /// This does not include `future` since we don't need to create new
    /// packages with it.
    pub const CLI_VALUES: [&'static str; 4] = ["2015", "2018", "2021", "2024"];

    /// Returns the first version that a particular edition was released on
    /// stable.
    pub(crate) fn first_version(&self) -> Option<semver::Version> {
        use Edition::*;
        match self {
            Edition2015 => None,
            Edition2018 => Some(semver::Version::new(1, 31, 0)),
            Edition2021 => Some(semver::Version::new(1, 56, 0)),
            Edition2024 => Some(semver::Version::new(1, 85, 0)),
            EditionFuture => None,
        }
    }

    /// Returns `true` if this edition is stable in this release.
    pub fn is_stable(&self) -> bool {
        use Edition::*;
        match self {
            Edition2015 => true,
            Edition2018 => true,
            Edition2021 => true,
            Edition2024 => true,
            EditionFuture => false,
        }
    }

    /// Returns the previous edition from this edition.
    ///
    /// Returns `None` for 2015.
    pub fn previous(&self) -> Option<Edition> {
        use Edition::*;
        match self {
            Edition2015 => None,
            Edition2018 => Some(Edition2015),
            Edition2021 => Some(Edition2018),
            Edition2024 => Some(Edition2021),
            EditionFuture => panic!("future does not have a previous edition"),
        }
    }

    /// Returns the next edition from this edition, returning the last edition
    /// if this is already the last one.
    pub fn saturating_next(&self) -> Edition {
        use Edition::*;
        // Nothing should treat "future" as being next.
        match self {
            Edition2015 => Edition2018,
            Edition2018 => Edition2021,
            Edition2021 => Edition2024,
            Edition2024 => Edition2024,
            EditionFuture => EditionFuture,
        }
    }

    /// Updates the given [`ProcessBuilder`] to include the appropriate flags
    /// for setting the edition.
    pub(crate) fn cmd_edition_arg(&self, cmd: &mut ProcessBuilder) {
        cmd.arg(format!("--edition={}", self));
        if !self.is_stable() {
            cmd.arg("-Z").arg("unstable-options");
        }
    }

    /// Adds the appropriate argument to generate warnings for this edition.
    pub(crate) fn force_warn_arg(&self, cmd: &mut ProcessBuilder) {
        use Edition::*;
        match self {
            Edition2015 => {}
            EditionFuture => {
                cmd.arg("--force-warn=edition_future_compatibility");
            }
            e => {
                // Note that cargo always passes this even if the
                // compatibility lint group does not exist. When a new edition
                // is introduced, but there are no migration lints, rustc does
                // not create the lint group. That's OK because rustc will
                // just generate a warning about an unknown lint which will be
                // suppressed due to cap-lints.
                cmd.arg(format!("--force-warn=rust-{e}-compatibility"));
            }
        }
    }

    /// Whether or not this edition supports the `rust_*_idioms` lint.
    ///
    /// Ideally this would not be necessary...
    pub(crate) fn supports_idiom_lint(&self) -> bool {
        use Edition::*;
        match self {
            Edition2015 => false,
            Edition2018 => true,
            Edition2021 => false,
            Edition2024 => false,
            EditionFuture => false,
        }
    }

    pub(crate) fn default_resolve_behavior(&self) -> ResolveBehavior {
        if *self >= Edition::Edition2024 {
            ResolveBehavior::V3
        } else if *self >= Edition::Edition2021 {
            ResolveBehavior::V2
        } else {
            ResolveBehavior::V1
        }
    }
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Edition::Edition2015 => f.write_str("2015"),
            Edition::Edition2018 => f.write_str("2018"),
            Edition::Edition2021 => f.write_str("2021"),
            Edition::Edition2024 => f.write_str("2024"),
            Edition::EditionFuture => f.write_str("future"),
        }
    }
}

impl FromStr for Edition {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "2015" => Ok(Edition::Edition2015),
            "2018" => Ok(Edition::Edition2018),
            "2021" => Ok(Edition::Edition2021),
            "2024" => Ok(Edition::Edition2024),
            "future" => Ok(Edition::EditionFuture),
            s if s.parse().map_or(false, |y: u16| y > 2024 && y < 2050) => bail!(
                "this version of Cargo is older than the `{}` edition, \
                 and only supports `2015`, `2018`, `2021`, and `2024` editions.",
                s
            ),
            s => bail!(
                "supported edition values are `2015`, `2018`, `2021`, or `2024`, \
                 but `{}` is unknown",
                s
            ),
        }
    }
}

/// The value for `-Zfix-edition`.
#[derive(Debug, Deserialize)]
pub enum FixEdition {
    /// `-Zfix-edition=start=$INITIAL`
    ///
    /// This mode for `cargo fix` will just run `cargo check` if the current
    /// edition is equal to this edition. If it is a different edition, then
    /// it just exits with success. This is used for crater integration which
    /// needs to set a baseline for the "before" toolchain.
    Start(Edition),
    /// `-Zfix-edition=end=$INITIAL,$NEXT`
    ///
    /// This mode for `cargo fix` will migrate to the `next` edition if the
    /// current edition is `initial`. After migration, it will update
    /// `Cargo.toml` and verify that that it works on the new edition. If the
    /// current edition is not `initial`, then it immediately exits with
    /// success since we just want to ignore those packages.
    End { initial: Edition, next: Edition },
}

impl FromStr for FixEdition {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        if let Some(start) = s.strip_prefix("start=") {
            Ok(FixEdition::Start(start.parse()?))
        } else if let Some(end) = s.strip_prefix("end=") {
            let (initial, next) = end
                .split_once(',')
                .ok_or_else(|| anyhow::format_err!("expected `initial,next`"))?;
            Ok(FixEdition::End {
                initial: initial.parse()?,
                next: next.parse()?,
            })
        } else {
            bail!("invalid `-Zfix-edition, expected start= or end=, got `{s}`");
        }
    }
}

#[derive(Debug, PartialEq)]
enum Status {
    Stable,
    Unstable,
    Removed,
}

/// A listing of stable and unstable new syntax in Cargo.toml.
///
/// This generates definitions and impls for [`Features`] and [`Feature`]
/// for each new syntax.
///
/// Note that all feature names in the macro invocation are valid Rust
/// identifiers, but the `_` character is translated to `-` when specified in
/// the `cargo-features` manifest entry in `Cargo.toml`.
///
/// See the [module-level documentation](self#new-cargotoml-syntax)
/// for the process of adding a new syntax.
macro_rules! features {
    (
        $(
            $(#[$attr:meta])*
            ($stab:ident, $feature:ident, $version:expr, $docs:expr),
        )*
    ) => (
        /// Unstable feature context for querying if a new Cargo.toml syntax
        /// is allowed to use.
        ///
        /// See the [module-level documentation](self#new-cargotoml-syntax) for the usage.
        #[derive(Default, Clone, Debug)]
        pub struct Features {
            $($feature: bool,)*
            /// The current activated features.
            activated: Vec<String>,
            /// Whether is allowed to use any unstable features.
            nightly_features_allowed: bool,
            /// Whether the source manifest is from a local package.
            is_local: bool,
        }

        impl Feature {
            $(
                $(#[$attr])*
                #[doc = concat!("\n\n\nSee <https://doc.rust-lang.org/nightly/cargo/", $docs, ">.")]
                pub const fn $feature() -> &'static Feature {
                    fn get(features: &Features) -> bool {
                        stab!($stab) == Status::Stable || features.$feature
                    }
                    const FEAT: Feature = Feature {
                        name: stringify!($feature),
                        stability: stab!($stab),
                        version: $version,
                        docs: $docs,
                        get,
                    };
                    &FEAT
                }
            )*

            /// Whether this feature is allowed to use in the given [`Features`] context.
            fn is_enabled(&self, features: &Features) -> bool {
                (self.get)(features)
            }

            pub(crate) fn name(&self) -> &str {
                self.name
            }
        }

        impl Features {
            fn status(&mut self, feature: &str) -> Option<(&mut bool, &'static Feature)> {
                if feature.contains("_") {
                    return None;
                }
                let feature = feature.replace("-", "_");
                $(
                    if feature == stringify!($feature) {
                        return Some((&mut self.$feature, Feature::$feature()));
                    }
                )*
                None
            }
        }
    )
}

macro_rules! stab {
    (stable) => {
        Status::Stable
    };
    (unstable) => {
        Status::Unstable
    };
    (removed) => {
        Status::Removed
    };
}

// "look here"
features! {
    /// A dummy feature that doesn't actually gate anything, but it's used in
    /// testing to ensure that we can enable stable features.
    (stable, test_dummy_stable, "1.0", ""),

    /// A dummy feature that gates the usage of the `im-a-teapot` manifest
    /// entry. This is basically just intended for tests.
    (unstable, test_dummy_unstable, "", "reference/unstable.html"),

    /// Downloading packages from alternative registry indexes.
    (stable, alternative_registries, "1.34", "reference/registries.html"),

    /// Using editions
    (stable, edition, "1.31", "reference/manifest.html#the-edition-field"),

    /// Renaming a package in the manifest via the `package` key.
    (stable, rename_dependency, "1.31", "reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml"),

    /// Whether a lock file is published with this crate.
    (removed, publish_lockfile, "1.37", "reference/unstable.html#publish-lockfile"),

    /// Overriding profiles for dependencies.
    (stable, profile_overrides, "1.41", "reference/profiles.html#overrides"),

    /// "default-run" manifest option.
    (stable, default_run, "1.37", "reference/manifest.html#the-default-run-field"),

    /// Declarative build scripts.
    (unstable, metabuild, "", "reference/unstable.html#metabuild"),

    /// Specifying the 'public' attribute on dependencies.
    (unstable, public_dependency, "", "reference/unstable.html#public-dependency"),

    /// Allow to specify profiles other than 'dev', 'release', 'test', etc.
    (stable, named_profiles, "1.57", "reference/profiles.html#custom-profiles"),

    /// Opt-in new-resolver behavior.
    (stable, resolver, "1.51", "reference/resolver.html#resolver-versions"),

    /// Allow to specify whether binaries should be stripped.
    (stable, strip, "1.58", "reference/profiles.html#strip-option"),

    /// Specifying a minimal 'rust-version' attribute for crates.
    (stable, rust_version, "1.56", "reference/manifest.html#the-rust-version-field"),

    /// Support for 2021 edition.
    (stable, edition2021, "1.56", "reference/manifest.html#the-edition-field"),

    /// Allow to specify per-package targets (compile kinds).
    (unstable, per_package_target, "", "reference/unstable.html#per-package-target"),

    /// Allow to specify which codegen backend should be used.
    (unstable, codegen_backend, "", "reference/unstable.html#codegen-backend"),

    /// Allow specifying different binary name apart from the crate name.
    (unstable, different_binary_name, "", "reference/unstable.html#different-binary-name"),

    /// Allow specifying rustflags directly in a profile.
    (unstable, profile_rustflags, "", "reference/unstable.html#profile-rustflags-option"),

    /// Allow workspace members to inherit fields and dependencies from a workspace.
    (stable, workspace_inheritance, "1.64", "reference/unstable.html#workspace-inheritance"),

    /// Support for 2024 edition.
    (stable, edition2024, "1.85", "reference/manifest.html#the-edition-field"),

    /// Allow setting trim-paths in a profile to control the sanitisation of file paths in build outputs.
    (unstable, trim_paths, "", "reference/unstable.html#profile-trim-paths-option"),

    /// Allow multiple packages to participate in the same API namespace
    (unstable, open_namespaces, "", "reference/unstable.html#open-namespaces"),

    /// Allow paths that resolve relatively to a base specified in the config.
    (unstable, path_bases, "", "reference/unstable.html#path-bases"),

    /// Allows use of editions that are not yet stable.
    (unstable, unstable_editions, "", "reference/unstable.html#unstable-editions"),

    /// Allows use of multiple build scripts.
    (unstable, multiple_build_scripts, "", "reference/unstable.html#multiple-build-scripts"),

    /// Allows use of panic="immediate-abort".
    (unstable, panic_immediate_abort, "", "reference/unstable.html#panic-immediate-abort"),
}

/// Status and metadata for a single unstable feature.
#[derive(Debug)]
pub struct Feature {
    /// Feature name. This is valid Rust identifier so no dash only underscore.
    name: &'static str,
    stability: Status,
    /// Version that this feature was stabilized or removed.
    version: &'static str,
    /// Link to the unstable documentation.
    docs: &'static str,
    get: fn(&Features) -> bool,
}

impl Features {
    /// Creates a new unstable features context.
    pub fn new(
        features: &[String],
        gctx: &GlobalContext,
        warnings: &mut Vec<String>,
        is_local: bool,
    ) -> CargoResult<Features> {
        let mut ret = Features::default();
        ret.nightly_features_allowed = gctx.nightly_features_allowed;
        ret.is_local = is_local;
        for feature in features {
            ret.add(feature, gctx, warnings)?;
            ret.activated.push(feature.to_string());
        }
        Ok(ret)
    }

    fn add(
        &mut self,
        feature_name: &str,
        gctx: &GlobalContext,
        warnings: &mut Vec<String>,
    ) -> CargoResult<()> {
        let nightly_features_allowed = self.nightly_features_allowed;
        let Some((slot, feature)) = self.status(feature_name) else {
            let mut msg = format!("unknown Cargo.toml feature `{feature_name}`\n\n");
            let mut append_see_docs = true;

            if feature_name.contains('_') {
                let _ = writeln!(msg, "Feature names must use '-' instead of '_'.");
                append_see_docs = false;
            } else {
                let underscore_name = feature_name.replace('-', "_");
                if CliUnstable::help()
                    .iter()
                    .any(|(option, _)| *option == underscore_name)
                {
                    let _ = writeln!(
                        msg,
                        "This feature can be enabled via -Z{feature_name} or the `[unstable]` section in config.toml."
                    );
                }
            }

            if append_see_docs {
                let _ = writeln!(
                    msg,
                    "See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html for more information."
                );
            }
            bail!(msg)
        };

        if *slot {
            bail!(
                "the cargo feature `{}` has already been activated",
                feature_name
            );
        }

        let see_docs = || {
            format!(
                "See {} for more information about using this feature.",
                cargo_docs_link(feature.docs)
            )
        };

        match feature.stability {
            Status::Stable => {
                let warning = format!(
                    "the cargo feature `{}` has been stabilized in the {} \
                         release and is no longer necessary to be listed in the \
                         manifest\n  {}",
                    feature_name,
                    feature.version,
                    see_docs()
                );
                warnings.push(warning);
            }
            Status::Unstable if !nightly_features_allowed => bail!(
                "the cargo feature `{}` requires a nightly version of \
                 Cargo, but this is the `{}` channel\n\
                 {}\n{}",
                feature_name,
                channel(),
                SEE_CHANNELS,
                see_docs()
            ),
            Status::Unstable => {
                if let Some(allow) = &gctx.cli_unstable().allow_features {
                    if !allow.contains(feature_name) {
                        bail!(
                            "the feature `{}` is not in the list of allowed features: [{}]",
                            feature_name,
                            itertools::join(allow, ", "),
                        );
                    }
                }
            }
            Status::Removed => {
                let mut msg = format!(
                    "the cargo feature `{}` has been removed in the {} release\n\n",
                    feature_name, feature.version
                );
                if self.is_local {
                    let _ = writeln!(
                        msg,
                        "Remove the feature from Cargo.toml to remove this error."
                    );
                } else {
                    let _ = writeln!(
                        msg,
                        "This package cannot be used with this version of Cargo, \
                         as the unstable feature `{}` is no longer supported.",
                        feature_name
                    );
                }
                let _ = writeln!(msg, "{}", see_docs());
                bail!(msg);
            }
        }

        *slot = true;

        Ok(())
    }

    /// Gets the current activated features.
    pub fn activated(&self) -> &[String] {
        &self.activated
    }

    /// Checks if the given feature is enabled.
    pub fn require(&self, feature: &Feature) -> CargoResult<()> {
        if feature.is_enabled(self) {
            return Ok(());
        }
        let feature_name = feature.name.replace("_", "-");
        let mut msg = format!(
            "feature `{}` is required\n\
             \n\
             The package requires the Cargo feature called `{}`, but \
             that feature is not stabilized in this version of Cargo ({}).\n\
            ",
            feature_name,
            feature_name,
            crate::version(),
        );

        if self.nightly_features_allowed {
            if self.is_local {
                let _ = writeln!(
                    msg,
                    "Consider adding `cargo-features = [\"{}\"]` \
                     to the top of Cargo.toml (above the [package] table) \
                     to tell Cargo you are opting in to use this unstable feature.",
                    feature_name
                );
            } else {
                let _ = writeln!(msg, "Consider trying a more recent nightly release.");
            }
        } else {
            let _ = writeln!(
                msg,
                "Consider trying a newer version of Cargo \
                 (this may require the nightly release)."
            );
        }
        let _ = writeln!(
            msg,
            "See https://doc.rust-lang.org/nightly/cargo/{} for more information \
             about the status of this feature.",
            feature.docs
        );

        bail!("{}", msg);
    }

    /// Whether the given feature is allowed to use in this context.
    pub fn is_enabled(&self, feature: &Feature) -> bool {
        feature.is_enabled(self)
    }
}

/// Generates `-Z` flags as fields of [`CliUnstable`].
///
/// See the [module-level documentation](self#-z-options) for details.
macro_rules! unstable_cli_options {
    (
        $(
            $(#[$meta:meta])?
            $element: ident: $ty: ty$( = ($help:literal))?,
        )*
    ) => {
        /// A parsed representation of all unstable flags that Cargo accepts.
        ///
        /// Cargo, like `rustc`, accepts a suite of `-Z` flags which are intended for
        /// gating unstable functionality to Cargo. These flags are only available on
        /// the nightly channel of Cargo.
        #[derive(Default, Debug, Deserialize)]
        #[serde(default, rename_all = "kebab-case")]
        pub struct CliUnstable {
            $(
                $(#[doc = $help])?
                $(#[$meta])?
                pub $element: $ty
            ),*
        }
        impl CliUnstable {
            /// Returns a list of `(<option-name>, <help-text>)`.
            pub fn help() -> Vec<(&'static str, Option<&'static str>)> {
                let fields = vec![$((stringify!($element), None$(.or(Some($help)))?)),*];
                fields
            }
        }

        #[cfg(test)]
        mod test {
            #[test]
            fn ensure_sorted() {
                // This will be printed out if the fields are not sorted.
                let location = std::panic::Location::caller();
                println!(
                    "\nTo fix this test, sort the features inside the macro at {}:{}\n",
                    location.file(),
                    location.line()
                );
                let mut expected = vec![$(stringify!($element)),*];
                expected[2..].sort();
                let expected = format!("{:#?}", expected);
                let actual = format!("{:#?}", vec![$(stringify!($element)),*]);
                snapbox::assert_data_eq!(actual, expected);
            }
        }
    }
}

unstable_cli_options!(
    // Permanently unstable features:
    allow_features: Option<AllowFeatures> = ("Allow *only* the listed unstable features"),
    print_im_a_teapot: bool,

    // All other unstable features.
    // Please keep this list lexicographically ordered.
    advanced_env: bool,
    asymmetric_token: bool = ("Allows authenticating with asymmetric tokens"),
    avoid_dev_deps: bool = ("Avoid installing dev-dependencies if possible"),
    binary_dep_depinfo: bool = ("Track changes to dependency artifacts"),
    bindeps: bool = ("Allow Cargo packages to depend on bin, cdylib, and staticlib crates, and use the artifacts built by those crates"),
    build_analysis: bool = ("Record and persist build metrics across runs, with commands to query past builds."),
    build_dir_new_layout: bool = ("Use the new build-dir filesystem layout"),
    #[serde(deserialize_with = "deserialize_comma_separated_list")]
    build_std: Option<Vec<String>>  = ("Enable Cargo to compile the standard library itself as part of a crate graph compilation"),
    #[serde(deserialize_with = "deserialize_comma_separated_list")]
    build_std_features: Option<Vec<String>>  = ("Configure features enabled for the standard library itself when building the standard library"),
    cargo_lints: bool = ("Enable the `[lints.cargo]` table"),
    checksum_freshness: bool = ("Use a checksum to determine if output is fresh rather than filesystem mtime"),
    codegen_backend: bool = ("Enable the `codegen-backend` option in profiles in .cargo/config.toml file"),
    config_include: bool = ("Enable the `include` key in config files"),
    direct_minimal_versions: bool = ("Resolve minimal dependency versions instead of maximum (direct dependencies only)"),
    dual_proc_macros: bool = ("Build proc-macros for both the host and the target"),
    feature_unification: bool = ("Enable new feature unification modes in workspaces"),
    features: Option<Vec<String>>,
    fix_edition: Option<FixEdition> = ("Permanently unstable edition migration helper"),
    gc: bool = ("Track cache usage and \"garbage collect\" unused files"),
    #[serde(deserialize_with = "deserialize_git_features")]
    git: Option<GitFeatures> = ("Enable support for shallow git fetch operations"),
    #[serde(deserialize_with = "deserialize_gitoxide_features")]
    gitoxide: Option<GitoxideFeatures> = ("Use gitoxide for the given git interactions, or all of them if no argument is given"),
    host_config: bool = ("Enable the `[host]` section in the .cargo/config.toml file"),
    minimal_versions: bool = ("Resolve minimal dependency versions instead of maximum"),
    msrv_policy: bool = ("Enable rust-version aware policy within cargo"),
    mtime_on_use: bool = ("Configure Cargo to update the mtime of used files"),
    next_lockfile_bump: bool,
    no_embed_metadata: bool = ("Avoid embedding metadata in library artifacts"),
    no_index_update: bool = ("Do not update the registry index even if the cache is outdated"),
    panic_abort_tests: bool = ("Enable support to run tests with -Cpanic=abort"),
    panic_immediate_abort: bool = ("Enable setting `panic = \"immediate-abort\"` in profiles"),
    profile_hint_mostly_unused: bool = ("Enable the `hint-mostly-unused` setting in profiles to mark a crate as mostly unused."),
    profile_rustflags: bool = ("Enable the `rustflags` option in profiles in .cargo/config.toml file"),
    public_dependency: bool = ("Respect a dependency's `public` field in Cargo.toml to control public/private dependencies"),
    publish_timeout: bool = ("Enable the `publish.timeout` key in .cargo/config.toml file"),
    root_dir: Option<PathBuf> = ("Set the root directory relative to which paths are printed (defaults to workspace root)"),
    rustdoc_depinfo: bool = ("Use dep-info files in rustdoc rebuild detection"),
    rustdoc_map: bool = ("Allow passing external documentation mappings to rustdoc"),
    rustdoc_scrape_examples: bool = ("Allows Rustdoc to scrape code examples from reverse-dependencies"),
    sbom: bool = ("Enable the `sbom` option in build config in .cargo/config.toml file"),
    script: bool = ("Enable support for single-file, `.rs` packages"),
    section_timings: bool = ("Enable support for extended compilation sections in --timings output"),
    separate_nightlies: bool,
    skip_rustdoc_fingerprint: bool,
    target_applies_to_host: bool = ("Enable the `target-applies-to-host` key in the .cargo/config.toml file"),
    trim_paths: bool = ("Enable the `trim-paths` option in profiles"),
    unstable_options: bool = ("Allow the usage of unstable options"),
    warnings: bool = ("Allow use of the build.warnings config key"),
);

const STABILIZED_COMPILE_PROGRESS: &str = "The progress bar is now always \
    enabled when used on an interactive console.\n\
    See https://doc.rust-lang.org/cargo/reference/config.html#termprogresswhen \
    for information on controlling the progress bar.";

const STABILIZED_OFFLINE: &str = "Offline mode is now available via the \
    --offline CLI option";

const STABILIZED_CACHE_MESSAGES: &str = "Message caching is now always enabled.";

const STABILIZED_INSTALL_UPGRADE: &str = "Packages are now always upgraded if \
    they appear out of date.\n\
    See https://doc.rust-lang.org/cargo/commands/cargo-install.html for more \
    information on how upgrading works.";

const STABILIZED_CONFIG_PROFILE: &str = "See \
    https://doc.rust-lang.org/cargo/reference/config.html#profile for more \
    information about specifying profiles in config.";

const STABILIZED_CRATE_VERSIONS: &str = "The crate version is now \
    automatically added to the documentation.";

const STABILIZED_PACKAGE_FEATURES: &str = "Enhanced feature flag behavior is now \
    available in virtual workspaces, and `member/feature-name` syntax is also \
    always available. Other extensions require setting `resolver = \"2\"` in \
    Cargo.toml.\n\
    See https://doc.rust-lang.org/nightly/cargo/reference/features.html#resolver-version-2-command-line-flags \
    for more information.";

const STABILIZED_FEATURES: &str = "The new feature resolver is now available \
    by specifying `resolver = \"2\"` in Cargo.toml.\n\
    See https://doc.rust-lang.org/nightly/cargo/reference/features.html#feature-resolver-version-2 \
    for more information.";

const STABILIZED_EXTRA_LINK_ARG: &str = "Additional linker arguments are now \
    supported without passing this flag.";

const STABILIZED_CONFIGURABLE_ENV: &str = "The [env] section is now always enabled.";

const STABILIZED_PATCH_IN_CONFIG: &str = "The patch-in-config feature is now always enabled.";

const STABILIZED_NAMED_PROFILES: &str = "The named-profiles feature is now always enabled.\n\
    See https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#custom-profiles \
    for more information";

const STABILIZED_DOCTEST_IN_WORKSPACE: &str =
    "The doctest-in-workspace feature is now always enabled.";

const STABILIZED_FUTURE_INCOMPAT_REPORT: &str =
    "The future-incompat-report feature is now always enabled.";

const STABILIZED_WEAK_DEP_FEATURES: &str = "Weak dependency features are now always available.";

const STABILISED_NAMESPACED_FEATURES: &str = "Namespaced features are now always available.";

const STABILIZED_TIMINGS: &str = "The -Ztimings option has been stabilized as --timings.";

const STABILISED_MULTITARGET: &str = "Multiple `--target` options are now always available.";

const STABILIZED_TERMINAL_WIDTH: &str =
    "The -Zterminal-width option is now always enabled for terminal output.";

const STABILISED_SPARSE_REGISTRY: &str = "The sparse protocol is now the default for crates.io";

const STABILIZED_CREDENTIAL_PROCESS: &str =
    "Authentication with a credential provider is always available.";

const STABILIZED_REGISTRY_AUTH: &str =
    "Authenticated registries are available if a credential provider is configured.";

const STABILIZED_LINTS: &str = "The `[lints]` table is now always available.";

const STABILIZED_CHECK_CFG: &str =
    "Compile-time checking of conditional (a.k.a. `-Zcheck-cfg`) is now always enabled.";

const STABILIZED_DOCTEST_XCOMPILE: &str = "Doctest cross-compiling is now always enabled.";

const STABILIZED_PACKAGE_WORKSPACE: &str =
    "Workspace packaging and publishing (a.k.a. `-Zpackage-workspace`) is now always enabled.";

const STABILIZED_BUILD_DIR: &str = "build.build-dir is now always enabled.";

fn deserialize_comma_separated_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(list) = <Option<Vec<String>>>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let v = list
        .iter()
        .flat_map(|s| s.split(','))
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    Ok(Some(v))
}

#[derive(Debug, Copy, Clone, Default, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
#[serde(default)]
pub struct GitFeatures {
    /// When cloning the index, perform a shallow clone. Maintain shallowness upon subsequent fetches.
    pub shallow_index: bool,
    /// When cloning git dependencies, perform a shallow clone and maintain shallowness on subsequent fetches.
    pub shallow_deps: bool,
}

impl GitFeatures {
    pub fn all() -> Self {
        GitFeatures {
            shallow_index: true,
            shallow_deps: true,
        }
    }

    fn expecting() -> String {
        let fields = ["`shallow-index`", "`shallow-deps`"];
        format!(
            "unstable 'git' only takes {} as valid inputs",
            fields.join(" and ")
        )
    }
}

fn deserialize_git_features<'de, D>(deserializer: D) -> Result<Option<GitFeatures>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct GitFeaturesVisitor;

    impl<'de> serde::de::Visitor<'de> for GitFeaturesVisitor {
        type Value = Option<GitFeatures>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&GitFeatures::expecting())
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v {
                Ok(Some(GitFeatures::all()))
            } else {
                Ok(None)
            }
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(parse_git(s.split(",")).map_err(serde::de::Error::custom)?)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            let git = GitFeatures::deserialize(deserializer)?;
            Ok(Some(git))
        }

        fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
        where
            V: serde::de::MapAccess<'de>,
        {
            let mvd = serde::de::value::MapAccessDeserializer::new(map);
            Ok(Some(GitFeatures::deserialize(mvd)?))
        }
    }

    deserializer.deserialize_any(GitFeaturesVisitor)
}

fn parse_git(it: impl Iterator<Item = impl AsRef<str>>) -> CargoResult<Option<GitFeatures>> {
    let mut out = GitFeatures::default();
    let GitFeatures {
        shallow_index,
        shallow_deps,
    } = &mut out;

    for e in it {
        match e.as_ref() {
            "shallow-index" => *shallow_index = true,
            "shallow-deps" => *shallow_deps = true,
            _ => {
                bail!(GitFeatures::expecting())
            }
        }
    }
    Ok(Some(out))
}

#[derive(Debug, Copy, Clone, Default, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
#[serde(default)]
pub struct GitoxideFeatures {
    /// All fetches are done with `gitoxide`, which includes git dependencies as well as the crates index.
    pub fetch: bool,
    /// Checkout git dependencies using `gitoxide` (submodules are still handled by git2 ATM, and filters
    /// like linefeed conversions are unsupported).
    pub checkout: bool,
    /// A feature flag which doesn't have any meaning except for preventing
    /// `__CARGO_USE_GITOXIDE_INSTEAD_OF_GIT2=1` builds to enable all safe `gitoxide` features.
    /// That way, `gitoxide` isn't actually used even though it's enabled.
    pub internal_use_git2: bool,
}

impl GitoxideFeatures {
    pub fn all() -> Self {
        GitoxideFeatures {
            fetch: true,
            checkout: true,
            internal_use_git2: false,
        }
    }

    /// Features we deem safe for everyday use - typically true when all tests pass with them
    /// AND they are backwards compatible.
    fn safe() -> Self {
        GitoxideFeatures {
            fetch: true,
            checkout: true,
            internal_use_git2: false,
        }
    }

    fn expecting() -> String {
        let fields = ["`fetch`", "`checkout`", "`internal-use-git2`"];
        format!(
            "unstable 'gitoxide' only takes {} as valid inputs, for shallow fetches see `-Zgit=shallow-index,shallow-deps`",
            fields.join(" and ")
        )
    }
}

fn deserialize_gitoxide_features<'de, D>(
    deserializer: D,
) -> Result<Option<GitoxideFeatures>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct GitoxideFeaturesVisitor;

    impl<'de> serde::de::Visitor<'de> for GitoxideFeaturesVisitor {
        type Value = Option<GitoxideFeatures>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&GitoxideFeatures::expecting())
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(parse_gitoxide(s.split(",")).map_err(serde::de::Error::custom)?)
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v {
                Ok(Some(GitoxideFeatures::all()))
            } else {
                Ok(None)
            }
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            let gitoxide = GitoxideFeatures::deserialize(deserializer)?;
            Ok(Some(gitoxide))
        }

        fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
        where
            V: serde::de::MapAccess<'de>,
        {
            let mvd = serde::de::value::MapAccessDeserializer::new(map);
            Ok(Some(GitoxideFeatures::deserialize(mvd)?))
        }
    }

    deserializer.deserialize_any(GitoxideFeaturesVisitor)
}

fn parse_gitoxide(
    it: impl Iterator<Item = impl AsRef<str>>,
) -> CargoResult<Option<GitoxideFeatures>> {
    let mut out = GitoxideFeatures::default();
    let GitoxideFeatures {
        fetch,
        checkout,
        internal_use_git2,
    } = &mut out;

    for e in it {
        match e.as_ref() {
            "fetch" => *fetch = true,
            "checkout" => *checkout = true,
            "internal-use-git2" => *internal_use_git2 = true,
            _ => {
                bail!(GitoxideFeatures::expecting())
            }
        }
    }
    Ok(Some(out))
}

impl CliUnstable {
    /// Parses `-Z` flags from the command line, and returns messages that warn
    /// if any flag has already been stabilized.
    pub fn parse(
        &mut self,
        flags: &[String],
        nightly_features_allowed: bool,
    ) -> CargoResult<Vec<String>> {
        if !flags.is_empty() && !nightly_features_allowed {
            bail!(
                "the `-Z` flag is only accepted on the nightly channel of Cargo, \
                 but this is the `{}` channel\n\
                 {}",
                channel(),
                SEE_CHANNELS
            );
        }
        let mut warnings = Vec::new();
        // We read flags twice, first to get allowed-features (if specified),
        // and then to read the remaining unstable flags.
        for flag in flags {
            if flag.starts_with("allow-features=") {
                self.add(flag, &mut warnings)?;
            }
        }
        for flag in flags {
            self.add(flag, &mut warnings)?;
        }

        if self.gitoxide.is_none() && cargo_use_gitoxide_instead_of_git2() {
            self.gitoxide = GitoxideFeatures::safe().into();
        }
        Ok(warnings)
    }

    fn add(&mut self, flag: &str, warnings: &mut Vec<String>) -> CargoResult<()> {
        let mut parts = flag.splitn(2, '=');
        let k = parts.next().unwrap();
        let v = parts.next();

        fn parse_bool(key: &str, value: Option<&str>) -> CargoResult<bool> {
            match value {
                None | Some("yes") => Ok(true),
                Some("no") => Ok(false),
                Some(s) => bail!("flag -Z{} expected `no` or `yes`, found: `{}`", key, s),
            }
        }

        /// Parse a comma-separated list
        fn parse_list(value: Option<&str>) -> Vec<String> {
            match value {
                None => Vec::new(),
                Some("") => Vec::new(),
                Some(v) => v.split(',').map(|s| s.to_string()).collect(),
            }
        }

        // Asserts that there is no argument to the flag.
        fn parse_empty(key: &str, value: Option<&str>) -> CargoResult<bool> {
            if let Some(v) = value {
                bail!("flag -Z{} does not take a value, found: `{}`", key, v);
            }
            Ok(true)
        }

        let mut stabilized_warn = |key: &str, version: &str, message: &str| {
            warnings.push(format!(
                "flag `-Z {}` has been stabilized in the {} release, \
                 and is no longer necessary\n{}",
                key,
                version,
                indented_lines(message)
            ));
        };

        // Use this if the behavior now requires another mechanism to enable.
        let stabilized_err = |key: &str, version: &str, message: &str| {
            Err(anyhow::format_err!(
                "flag `-Z {}` has been stabilized in the {} release\n{}",
                key,
                version,
                indented_lines(message)
            ))
        };

        if let Some(allowed) = &self.allow_features {
            if k != "allow-features" && !allowed.contains(k) {
                bail!(
                    "the feature `{}` is not in the list of allowed features: [{}]",
                    k,
                    itertools::join(allowed, ", ")
                );
            }
        }

        match k {
            // Permanently unstable features
            // Sorted alphabetically:
            "allow-features" => self.allow_features = Some(parse_list(v).into_iter().collect()),
            "print-im-a-teapot" => self.print_im_a_teapot = parse_bool(k, v)?,

            // Stabilized features
            // Sorted by version, then alphabetically:
            "compile-progress" => stabilized_warn(k, "1.30", STABILIZED_COMPILE_PROGRESS),
            "offline" => stabilized_err(k, "1.36", STABILIZED_OFFLINE)?,
            "cache-messages" => stabilized_warn(k, "1.40", STABILIZED_CACHE_MESSAGES),
            "install-upgrade" => stabilized_warn(k, "1.41", STABILIZED_INSTALL_UPGRADE),
            "config-profile" => stabilized_warn(k, "1.43", STABILIZED_CONFIG_PROFILE),
            "crate-versions" => stabilized_warn(k, "1.47", STABILIZED_CRATE_VERSIONS),
            "features" => {
                // `-Z features` has been stabilized since 1.51,
                // but `-Z features=compare` is still allowed for convenience
                // to validate that the feature resolver resolves features
                // in the same way as the dependency resolver,
                // until we feel confident to remove entirely.
                //
                // See rust-lang/cargo#11168
                let feats = parse_list(v);
                let stab_is_not_empty = feats.iter().any(|feat| {
                    matches!(
                        feat.as_str(),
                        "build_dep" | "host_dep" | "dev_dep" | "itarget" | "all"
                    )
                });
                if stab_is_not_empty || feats.is_empty() {
                    // Make this stabilized_err once -Zfeature support is removed.
                    stabilized_warn(k, "1.51", STABILIZED_FEATURES);
                }
                self.features = Some(feats);
            }
            "package-features" => stabilized_warn(k, "1.51", STABILIZED_PACKAGE_FEATURES),
            "configurable-env" => stabilized_warn(k, "1.56", STABILIZED_CONFIGURABLE_ENV),
            "extra-link-arg" => stabilized_warn(k, "1.56", STABILIZED_EXTRA_LINK_ARG),
            "patch-in-config" => stabilized_warn(k, "1.56", STABILIZED_PATCH_IN_CONFIG),
            "named-profiles" => stabilized_warn(k, "1.57", STABILIZED_NAMED_PROFILES),
            "future-incompat-report" => {
                stabilized_warn(k, "1.59.0", STABILIZED_FUTURE_INCOMPAT_REPORT)
            }
            "namespaced-features" => stabilized_warn(k, "1.60", STABILISED_NAMESPACED_FEATURES),
            "timings" => stabilized_warn(k, "1.60", STABILIZED_TIMINGS),
            "weak-dep-features" => stabilized_warn(k, "1.60", STABILIZED_WEAK_DEP_FEATURES),
            "multitarget" => stabilized_warn(k, "1.64", STABILISED_MULTITARGET),
            "sparse-registry" => stabilized_warn(k, "1.68", STABILISED_SPARSE_REGISTRY),
            "terminal-width" => stabilized_warn(k, "1.68", STABILIZED_TERMINAL_WIDTH),
            "doctest-in-workspace" => stabilized_warn(k, "1.72", STABILIZED_DOCTEST_IN_WORKSPACE),
            "credential-process" => stabilized_warn(k, "1.74", STABILIZED_CREDENTIAL_PROCESS),
            "lints" => stabilized_warn(k, "1.74", STABILIZED_LINTS),
            "registry-auth" => stabilized_warn(k, "1.74", STABILIZED_REGISTRY_AUTH),
            "check-cfg" => stabilized_warn(k, "1.80", STABILIZED_CHECK_CFG),
            "doctest-xcompile" => stabilized_warn(k, "1.89", STABILIZED_DOCTEST_XCOMPILE),
            "package-workspace" => stabilized_warn(k, "1.89", STABILIZED_PACKAGE_WORKSPACE),
            "build-dir" => stabilized_warn(k, "1.91", STABILIZED_BUILD_DIR),

            // Unstable features
            // Sorted alphabetically:
            "advanced-env" => self.advanced_env = parse_empty(k, v)?,
            "asymmetric-token" => self.asymmetric_token = parse_empty(k, v)?,
            "avoid-dev-deps" => self.avoid_dev_deps = parse_empty(k, v)?,
            "binary-dep-depinfo" => self.binary_dep_depinfo = parse_empty(k, v)?,
            "bindeps" => self.bindeps = parse_empty(k, v)?,
            "build-analysis" => self.build_analysis = parse_empty(k, v)?,
            "build-dir-new-layout" => self.build_dir_new_layout = parse_empty(k, v)?,
            "build-std" => self.build_std = Some(parse_list(v)),
            "build-std-features" => self.build_std_features = Some(parse_list(v)),
            "cargo-lints" => self.cargo_lints = parse_empty(k, v)?,
            "codegen-backend" => self.codegen_backend = parse_empty(k, v)?,
            "config-include" => self.config_include = parse_empty(k, v)?,
            "direct-minimal-versions" => self.direct_minimal_versions = parse_empty(k, v)?,
            "dual-proc-macros" => self.dual_proc_macros = parse_empty(k, v)?,
            "feature-unification" => self.feature_unification = parse_empty(k, v)?,
            "fix-edition" => {
                let fe = v
                    .ok_or_else(|| anyhow::anyhow!("-Zfix-edition expected a value"))?
                    .parse()?;
                self.fix_edition = Some(fe);
            }
            "gc" => self.gc = parse_empty(k, v)?,
            "git" => {
                self.git =
                    v.map_or_else(|| Ok(Some(GitFeatures::all())), |v| parse_git(v.split(',')))?
            }
            "gitoxide" => {
                self.gitoxide = v.map_or_else(
                    || Ok(Some(GitoxideFeatures::all())),
                    |v| parse_gitoxide(v.split(',')),
                )?
            }
            "host-config" => self.host_config = parse_empty(k, v)?,
            "next-lockfile-bump" => self.next_lockfile_bump = parse_empty(k, v)?,
            "minimal-versions" => self.minimal_versions = parse_empty(k, v)?,
            "msrv-policy" => self.msrv_policy = parse_empty(k, v)?,
            // can also be set in .cargo/config or with and ENV
            "mtime-on-use" => self.mtime_on_use = parse_empty(k, v)?,
            "no-embed-metadata" => self.no_embed_metadata = parse_empty(k, v)?,
            "no-index-update" => self.no_index_update = parse_empty(k, v)?,
            "panic-abort-tests" => self.panic_abort_tests = parse_empty(k, v)?,
            "public-dependency" => self.public_dependency = parse_empty(k, v)?,
            "profile-hint-mostly-unused" => self.profile_hint_mostly_unused = parse_empty(k, v)?,
            "profile-rustflags" => self.profile_rustflags = parse_empty(k, v)?,
            "trim-paths" => self.trim_paths = parse_empty(k, v)?,
            "publish-timeout" => self.publish_timeout = parse_empty(k, v)?,
            "root-dir" => self.root_dir = v.map(|v| v.into()),
            "rustdoc-depinfo" => self.rustdoc_depinfo = parse_empty(k, v)?,
            "rustdoc-map" => self.rustdoc_map = parse_empty(k, v)?,
            "rustdoc-scrape-examples" => self.rustdoc_scrape_examples = parse_empty(k, v)?,
            "sbom" => self.sbom = parse_empty(k, v)?,
            "section-timings" => self.section_timings = parse_empty(k, v)?,
            "separate-nightlies" => self.separate_nightlies = parse_empty(k, v)?,
            "checksum-freshness" => self.checksum_freshness = parse_empty(k, v)?,
            "skip-rustdoc-fingerprint" => self.skip_rustdoc_fingerprint = parse_empty(k, v)?,
            "script" => self.script = parse_empty(k, v)?,
            "target-applies-to-host" => self.target_applies_to_host = parse_empty(k, v)?,
            "panic-immediate-abort" => self.panic_immediate_abort = parse_empty(k, v)?,
            "unstable-options" => self.unstable_options = parse_empty(k, v)?,
            "warnings" => self.warnings = parse_empty(k, v)?,
            _ => bail!(
                "\
            unknown `-Z` flag specified: {k}\n\n\
            For available unstable features, see \
            https://doc.rust-lang.org/nightly/cargo/reference/unstable.html\n\
            If you intended to use an unstable rustc feature, try setting `RUSTFLAGS=\"-Z{k}\"`"
            ),
        }

        Ok(())
    }

    /// Generates an error if `-Z unstable-options` was not used for a new,
    /// unstable command-line flag.
    pub fn fail_if_stable_opt(&self, flag: &str, issue: u32) -> CargoResult<()> {
        self.fail_if_stable_opt_custom_z(flag, issue, "unstable-options", self.unstable_options)
    }

    pub fn fail_if_stable_opt_custom_z(
        &self,
        flag: &str,
        issue: u32,
        z_name: &str,
        enabled: bool,
    ) -> CargoResult<()> {
        if !enabled {
            let see = format!(
                "See https://github.com/rust-lang/cargo/issues/{issue} for more \
                 information about the `{flag}` flag."
            );
            // NOTE: a `config` isn't available here, check the channel directly
            let channel = channel();
            if channel == "nightly" || channel == "dev" {
                bail!(
                    "the `{flag}` flag is unstable, pass `-Z {z_name}` to enable it\n\
                     {see}"
                );
            } else {
                bail!(
                    "the `{flag}` flag is unstable, and only available on the nightly channel \
                     of Cargo, but this is the `{channel}` channel\n\
                     {SEE_CHANNELS}\n\
                     {see}"
                );
            }
        }
        Ok(())
    }

    /// Generates an error if `-Z unstable-options` was not used for a new,
    /// unstable subcommand.
    pub fn fail_if_stable_command(
        &self,
        gctx: &GlobalContext,
        command: &str,
        issue: u32,
        z_name: &str,
        enabled: bool,
    ) -> CargoResult<()> {
        if enabled {
            return Ok(());
        }
        let see = format!(
            "See https://github.com/rust-lang/cargo/issues/{} for more \
            information about the `cargo {}` command.",
            issue, command
        );
        if gctx.nightly_features_allowed {
            bail!(
                "the `cargo {command}` command is unstable, pass `-Z {z_name}` \
                 to enable it\n\
                 {see}",
            );
        } else {
            bail!(
                "the `cargo {}` command is unstable, and only available on the \
                 nightly channel of Cargo, but this is the `{}` channel\n\
                 {}\n\
                 {}",
                command,
                channel(),
                SEE_CHANNELS,
                see
            );
        }
    }
}

/// Returns the current release channel ("stable", "beta", "nightly", "dev").
pub fn channel() -> String {
    // ALLOWED: For testing cargo itself only.
    #[allow(clippy::disallowed_methods)]
    if let Ok(override_channel) = env::var("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS") {
        return override_channel;
    }
    // ALLOWED: the process of rustc bootstrapping reads this through
    // `std::env`. We should make the behavior consistent. Also, we
    // don't advertise this for bypassing nightly.
    #[allow(clippy::disallowed_methods)]
    if let Ok(staging) = env::var("RUSTC_BOOTSTRAP") {
        if staging == "1" {
            return "dev".to_string();
        }
    }
    crate::version()
        .release_channel
        .unwrap_or_else(|| String::from("dev"))
}

/// Only for testing and developing. See ["Running with gitoxide as default git backend in tests"][1].
///
/// [1]: https://doc.crates.io/contrib/tests/running.html#running-with-gitoxide-as-default-git-backend-in-tests
// ALLOWED: For testing cargo itself only.
#[allow(clippy::disallowed_methods)]
fn cargo_use_gitoxide_instead_of_git2() -> bool {
    std::env::var_os("__CARGO_USE_GITOXIDE_INSTEAD_OF_GIT2").map_or(false, |value| value == "1")
}

/// Generate a link to Cargo documentation for the current release channel
/// `path` is the URL component after `https://doc.rust-lang.org/{channel}/cargo/`
pub fn cargo_docs_link(path: &str) -> String {
    let url_channel = match channel().as_str() {
        "dev" | "nightly" => "nightly/",
        "beta" => "beta/",
        _ => "",
    };
    format!("https://doc.rust-lang.org/{url_channel}cargo/{path}")
}
