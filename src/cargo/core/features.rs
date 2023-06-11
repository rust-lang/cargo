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
//! * New syntax in Cargo.toml should use `cargo-features`.
//! * New CLI options should use `-Z unstable-options`.
//! * New functionality that may not have an interface, or the interface has
//!   not yet been designed, or for more complex features that affect multiple
//!   parts of Cargo should use a new `-Z` flag.
//!
//! See below for more details.
//!
//! When adding new tests for your feature, usually the tests should go into a
//! new module of the testsuite. See
//! <https://doc.crates.io/contrib/tests/writing.html> for more information on
//! writing tests. Particularly, check out the "Testing Nightly Features"
//! section for testing unstable features.
//!
//! After you have added your feature, be sure to update the unstable
//! documentation at `src/doc/src/reference/unstable.md` to include a short
//! description of how to use your new feature.
//!
//! And hopefully that's it!
//!
//! ## New Cargo.toml syntax
//!
//! The steps for adding new Cargo.toml syntax are:
//!
//! 1. Add the cargo-features unstable gate. Search below for "look here" to
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
//! 1. Add the option to the [`CliUnstable`] struct below. Flags can take an
//!    optional value if you want.
//! 2. Update the [`CliUnstable::add`] function to parse the flag.
//! 3. Wherever the new functionality is implemented, call
//!    [`Config::cli_unstable`] to get an instance of [`CliUnstable`]
//!    and check if the option has been enabled on the [`CliUnstable`] instance.
//!    Nightly gating is already handled, so no need to worry about that.
//!
//! ### `-Z` vs `cargo-features`
//!
//! In some cases there might be some changes that `cargo-features` is unable
//! to sufficiently encompass. An example would be a syntax change in
//! `Cargo.toml` that also impacts the index or resolver. The resolver doesn't
//! know about `cargo-features`, so it needs a `-Z` flag to enable the
//! experimental functionality.
//!
//! In those cases, you usually should introduce both a `-Z` flag (to enable
//! the changes outside of the manifest) and a `cargo-features` entry (to
//! enable the new syntax in `Cargo.toml`). The `cargo-features` entry ensures
//! that any experimental syntax that gets uploaded to crates.io is clearly
//! intended for nightly-only builds. Otherwise, users accessing those crates
//! may get confusing errors, particularly if the syntax changes during the
//! development cycle, and the user tries to access it with a stable release.
//!
//! ### `-Z` with external files
//!
//! Some files, such as `config.toml` config files, or the `config.json` index
//! file, are used in a global location which can make interaction with stable
//! releases problematic. In general, before the feature is stabilized, stable
//! Cargo should behave roughly similar to how it behaved *before* the
//! unstable feature was introduced. If Cargo would normally have ignored or
//! warned about the introduction of something, then it probably should
//! continue to do so.
//!
//! For example, Cargo generally ignores (or warns) about `config.toml`
//! entries it doesn't know about. This allows a limited degree of
//! forwards-compatibility with future versions of Cargo that add new entries.
//!
//! Whether or not to warn on stable may need to be decided on a case-by-case
//! basis. For example, you may want to avoid generating a warning for options
//! that are not critical to Cargo's operation in order to reduce the
//! annoyance of constant warnings. However, ignoring some options may prevent
//! proper operation, so a warning may be valuable for a user trying to
//! diagnose why it isn't working correctly.
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
//! [`Config::cli_unstable`]: crate::util::config::Config::cli_unstable
//! [`fail_if_stable_opt`]: CliUnstable::fail_if_stable_opt
//! [`features!`]: macro.features.html

use std::collections::BTreeSet;
use std::env;
use std::fmt::{self, Write};
use std::str::FromStr;

use anyhow::{bail, Error};
use cargo_util::ProcessBuilder;
use serde::{Deserialize, Serialize};

use crate::core::resolver::ResolveBehavior;
use crate::util::errors::CargoResult;
use crate::util::{indented_lines, iter_join};
use crate::Config;

pub const HIDDEN: &str = "";
pub const SEE_CHANNELS: &str =
    "See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information \
     about Rust release channels.";

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
/// - Add an unstable feature to the [`features!`] macro invocation below for the new edition.
/// - Gate on that new feature in [`TomlManifest::to_real_manifest`].
/// - Update the shell completion files.
/// - Update any failing tests (hopefully there are very few).
/// - Update unstable.md to add a new section for this new edition (see [this example]).
///
/// ## Stabilization instructions
///
/// - Set [`LATEST_UNSTABLE`] to None.
/// - Set [`LATEST_STABLE`] to the new version.
/// - Update [`is_stable`] to `true`.
/// - Set the editionNNNN feature to stable in the [`features!`] macro invocation below.
/// - Update any tests that are affected.
/// - Update the man page for the `--edition` flag.
/// - Update unstable.md to move the edition section to the bottom.
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
/// [this example]: https://github.com/rust-lang/cargo/blob/3ebb5f15a940810f250b68821149387af583a79e/src/doc/src/reference/unstable.md?plain=1#L1238-L1264
/// [`is_stable`]: Edition::is_stable
/// [`TomlManifest::to_real_manifest`]: crate::util::toml::TomlManifest::to_real_manifest
/// [`features!`]: macro.features.html
#[derive(Clone, Copy, Debug, Hash, PartialOrd, Ord, Eq, PartialEq, Serialize, Deserialize)]
pub enum Edition {
    /// The 2015 edition
    Edition2015,
    /// The 2018 edition
    Edition2018,
    /// The 2021 edition
    Edition2021,
}

impl Edition {
    /// The latest edition that is unstable.
    ///
    /// This is `None` if there is no next unstable edition.
    pub const LATEST_UNSTABLE: Option<Edition> = None;
    /// The latest stable edition.
    pub const LATEST_STABLE: Edition = Edition::Edition2021;
    /// Possible values allowed for the `--edition` CLI flag.
    ///
    /// This requires a static value due to the way clap works, otherwise I
    /// would have built this dynamically.
    pub const CLI_VALUES: [&'static str; 3] = ["2015", "2018", "2021"];

    /// Returns the first version that a particular edition was released on
    /// stable.
    pub(crate) fn first_version(&self) -> Option<semver::Version> {
        use Edition::*;
        match self {
            Edition2015 => None,
            Edition2018 => Some(semver::Version::new(1, 31, 0)),
            Edition2021 => Some(semver::Version::new(1, 56, 0)),
        }
    }

    /// Returns `true` if this edition is stable in this release.
    pub fn is_stable(&self) -> bool {
        use Edition::*;
        match self {
            Edition2015 => true,
            Edition2018 => true,
            Edition2021 => true,
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
        }
    }

    /// Returns the next edition from this edition, returning the last edition
    /// if this is already the last one.
    pub fn saturating_next(&self) -> Edition {
        use Edition::*;
        match self {
            Edition2015 => Edition2018,
            Edition2018 => Edition2021,
            Edition2021 => Edition2021,
        }
    }

    /// Updates the given [`ProcessBuilder`] to include the appropriate flags
    /// for setting the edition.
    pub(crate) fn cmd_edition_arg(&self, cmd: &mut ProcessBuilder) {
        if *self != Edition::Edition2015 {
            cmd.arg(format!("--edition={}", self));
        }
        if !self.is_stable() {
            cmd.arg("-Z").arg("unstable-options");
        }
    }

    /// Whether or not this edition supports the `rust_*_compatibility` lint.
    ///
    /// Ideally this would not be necessary, but editions may not have any
    /// lints, and thus `rustc` doesn't recognize it. Perhaps `rustc` could
    /// create an empty group instead?
    pub(crate) fn supports_compat_lint(&self) -> bool {
        use Edition::*;
        match self {
            Edition2015 => false,
            Edition2018 => true,
            Edition2021 => true,
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
        }
    }

    pub(crate) fn default_resolve_behavior(&self) -> ResolveBehavior {
        if *self >= Edition::Edition2021 {
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
            s if s.parse().map_or(false, |y: u16| y > 2021 && y < 2050) => bail!(
                "this version of Cargo is older than the `{}` edition, \
                 and only supports `2015`, `2018`, and `2021` editions.",
                s
            ),
            s => bail!(
                "supported edition values are `2015`, `2018`, or `2021`, \
                 but `{}` is unknown",
                s
            ),
        }
    }
}

#[derive(PartialEq)]
enum Status {
    Stable,
    Unstable,
    Removed,
}

macro_rules! features {
    (
        $(($stab:ident, $feature:ident, $version:expr, $docs:expr),)*
    ) => (
        #[derive(Default, Clone, Debug)]
        pub struct Features {
            $($feature: bool,)*
            activated: Vec<String>,
            nightly_features_allowed: bool,
            is_local: bool,
        }

        impl Feature {
            $(
                pub fn $feature() -> &'static Feature {
                    fn get(features: &Features) -> bool {
                        stab!($stab) == Status::Stable || features.$feature
                    }
                    static FEAT: Feature = Feature {
                        name: stringify!($feature),
                        stability: stab!($stab),
                        version: $version,
                        docs: $docs,
                        get,
                    };
                    &FEAT
                }
            )*

            fn is_enabled(&self, features: &Features) -> bool {
                (self.get)(features)
            }
        }

        impl Features {
            fn status(&mut self, feature: &str) -> Option<(&mut bool, &'static Feature)> {
                if feature.contains("_") {
                    return None
                }
                let feature = feature.replace("-", "_");
                $(
                    if feature == stringify!($feature) {
                        return Some((&mut self.$feature, Feature::$feature()))
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

// A listing of all features in Cargo.
//
// "look here"
//
// This is the macro that lists all stable and unstable features in Cargo.
// You'll want to add to this macro whenever you add a feature to Cargo, also
// following the directions above.
//
// Note that all feature names here are valid Rust identifiers, but the `_`
// character is translated to `-` when specified in the `cargo-features`
// manifest entry in `Cargo.toml`.
features! {
    // A dummy feature that doesn't actually gate anything, but it's used in
    // testing to ensure that we can enable stable features.
    (stable, test_dummy_stable, "1.0", ""),

    // A dummy feature that gates the usage of the `im-a-teapot` manifest
    // entry. This is basically just intended for tests.
    (unstable, test_dummy_unstable, "", "reference/unstable.html"),

    // Downloading packages from alternative registry indexes.
    (stable, alternative_registries, "1.34", "reference/registries.html"),

    // Using editions
    (stable, edition, "1.31", "reference/manifest.html#the-edition-field"),

    // Renaming a package in the manifest via the `package` key
    (stable, rename_dependency, "1.31", "reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml"),

    // Whether a lock file is published with this crate
    (removed, publish_lockfile, "1.37", "reference/unstable.html#publish-lockfile"),

    // Overriding profiles for dependencies.
    (stable, profile_overrides, "1.41", "reference/profiles.html#overrides"),

    // "default-run" manifest option,
    (stable, default_run, "1.37", "reference/manifest.html#the-default-run-field"),

    // Declarative build scripts.
    (unstable, metabuild, "", "reference/unstable.html#metabuild"),

    // Specifying the 'public' attribute on dependencies
    (unstable, public_dependency, "", "reference/unstable.html#public-dependency"),

    // Allow to specify profiles other than 'dev', 'release', 'test', etc.
    (stable, named_profiles, "1.57", "reference/profiles.html#custom-profiles"),

    // Opt-in new-resolver behavior.
    (stable, resolver, "1.51", "reference/resolver.html#resolver-versions"),

    // Allow to specify whether binaries should be stripped.
    (stable, strip, "1.58", "reference/profiles.html#strip-option"),

    // Specifying a minimal 'rust-version' attribute for crates
    (stable, rust_version, "1.56", "reference/manifest.html#the-rust-version-field"),

    // Support for 2021 edition.
    (stable, edition2021, "1.56", "reference/manifest.html#the-edition-field"),

    // Allow to specify per-package targets (compile kinds)
    (unstable, per_package_target, "", "reference/unstable.html#per-package-target"),

    // Allow to specify which codegen backend should be used.
    (unstable, codegen_backend, "", "reference/unstable.html#codegen-backend"),

    // Allow specifying different binary name apart from the crate name
    (unstable, different_binary_name, "", "reference/unstable.html#different-binary-name"),

    // Allow specifying rustflags directly in a profile
    (unstable, profile_rustflags, "", "reference/unstable.html#profile-rustflags-option"),

    // Allow specifying rustflags directly in a profile
    (stable, workspace_inheritance, "1.64", "reference/unstable.html#workspace-inheritance"),
}

pub struct Feature {
    name: &'static str,
    stability: Status,
    version: &'static str,
    docs: &'static str,
    get: fn(&Features) -> bool,
}

impl Features {
    pub fn new(
        features: &[String],
        config: &Config,
        warnings: &mut Vec<String>,
        is_local: bool,
    ) -> CargoResult<Features> {
        let mut ret = Features::default();
        ret.nightly_features_allowed = config.nightly_features_allowed;
        ret.is_local = is_local;
        for feature in features {
            ret.add(feature, config, warnings)?;
            ret.activated.push(feature.to_string());
        }
        Ok(ret)
    }

    fn add(
        &mut self,
        feature_name: &str,
        config: &Config,
        warnings: &mut Vec<String>,
    ) -> CargoResult<()> {
        let nightly_features_allowed = self.nightly_features_allowed;
        let is_local = self.is_local;
        let (slot, feature) = match self.status(feature_name) {
            Some(p) => p,
            None => bail!("unknown cargo feature `{}`", feature_name),
        };

        if *slot {
            bail!(
                "the cargo feature `{}` has already been activated",
                feature_name
            );
        }

        let see_docs = || {
            let url_channel = match channel().as_str() {
                "dev" | "nightly" => "nightly/",
                "beta" => "beta/",
                _ => "",
            };
            format!(
                "See https://doc.rust-lang.org/{}cargo/{} for more information \
                about using this feature.",
                url_channel, feature.docs
            )
        };

        match feature.stability {
            Status::Stable => {
                // The user can't do anything about non-local packages.
                // Warnings are usually suppressed, but just being cautious here.
                if is_local {
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
                if let Some(allow) = &config.cli_unstable().allow_features {
                    if !allow.contains(feature_name) {
                        bail!(
                            "the feature `{}` is not in the list of allowed features: [{}]",
                            feature_name,
                            iter_join(allow, ", "),
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

    pub fn activated(&self) -> &[String] {
        &self.activated
    }

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

    pub fn is_enabled(&self, feature: &Feature) -> bool {
        feature.is_enabled(self)
    }
}

macro_rules! unstable_cli_options {
    (
        $(
            $(#[$meta:meta])?
            $element: ident: $ty: ty = ($help: expr ),
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
                $(#[$meta])?
                pub $element: $ty
            ),*
        }
        impl CliUnstable {
            pub fn help() -> Vec<(&'static str, &'static str)> {
                let fields = vec![$((stringify!($element), $help)),*];
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
                snapbox::assert_eq(
                    format!("{:#?}", expected),
                    format!("{:#?}", vec![$(stringify!($element)),*])
                );
            }
        }
    }
}

unstable_cli_options!(
    // Permanently unstable features:
    allow_features: Option<BTreeSet<String>> = ("Allow *only* the listed unstable features"),
    print_im_a_teapot: bool = (HIDDEN),

    // All other unstable features.
    // Please keep this list lexicographically ordered.
    advanced_env: bool = (HIDDEN),
    avoid_dev_deps: bool = ("Avoid installing dev-dependencies if possible"),
    binary_dep_depinfo: bool = ("Track changes to dependency artifacts"),
    bindeps: bool = ("Allow Cargo packages to depend on bin, cdylib, and staticlib crates, and use the artifacts built by those crates"),
    #[serde(deserialize_with = "deserialize_build_std")]
    build_std: Option<Vec<String>>  = ("Enable Cargo to compile the standard library itself as part of a crate graph compilation"),
    build_std_features: Option<Vec<String>>  = ("Configure features enabled for the standard library itself when building the standard library"),
    #[serde(deserialize_with = "deserialize_check_cfg")]
    check_cfg: Option<(/*features:*/ bool, /*well_known_names:*/ bool, /*well_known_values:*/ bool, /*output:*/ bool)> = ("Specify scope of compile-time checking of `cfg` names/values"),
    codegen_backend: bool = ("Enable the `codegen-backend` option in profiles in .cargo/config.toml file"),
    config_include: bool = ("Enable the `include` key in config files"),
    credential_process: bool = ("Add a config setting to fetch registry authentication tokens by calling an external process"),
    direct_minimal_versions: bool = ("Resolve minimal dependency versions instead of maximum (direct dependencies only)"),
    doctest_in_workspace: bool = ("Compile doctests with paths relative to the workspace root"),
    doctest_xcompile: bool = ("Compile and run doctests for non-host target using runner config"),
    dual_proc_macros: bool = ("Build proc-macros for both the host and the target"),
    features: Option<Vec<String>>  = (HIDDEN),
    gitoxide: Option<GitoxideFeatures> = ("Use gitoxide for the given git interactions, or all of them if no argument is given"),
    host_config: bool = ("Enable the [host] section in the .cargo/config.toml file"),
    jobserver_per_rustc: bool = (HIDDEN),
    lints: bool = ("Pass `[lints]` to the linting tools"),
    minimal_versions: bool = ("Resolve minimal dependency versions instead of maximum"),
    msrv_policy: bool = ("Enable rust-version aware policy within cargo"),
    mtime_on_use: bool = ("Configure Cargo to update the mtime of used files"),
    no_index_update: bool = ("Do not update the registry index even if the cache is outdated"),
    panic_abort_tests: bool = ("Enable support to run tests with -Cpanic=abort"),
    profile_rustflags: bool = ("Enable the `rustflags` option in profiles in .cargo/config.toml file"),
    publish_timeout: bool = ("Enable the `publish.timeout` key in .cargo/config.toml file"),
    registry_auth: bool = ("Authentication for alternative registries, and generate registry authentication tokens using asymmetric cryptography"),
    rustdoc_map: bool = ("Allow passing external documentation mappings to rustdoc"),
    rustdoc_scrape_examples: bool = ("Allows Rustdoc to scrape code examples from reverse-dependencies"),
    separate_nightlies: bool = (HIDDEN),
    skip_rustdoc_fingerprint: bool = (HIDDEN),
    target_applies_to_host: bool = ("Enable the `target-applies-to-host` key in the .cargo/config.toml file"),
    unstable_options: bool = ("Allow the usage of unstable options"),
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

const STABILIZED_FUTURE_INCOMPAT_REPORT: &str =
    "The future-incompat-report feature is now always enabled.";

const STABILIZED_WEAK_DEP_FEATURES: &str = "Weak dependency features are now always available.";

const STABILISED_NAMESPACED_FEATURES: &str = "Namespaced features are now always available.";

const STABILIZED_TIMINGS: &str = "The -Ztimings option has been stabilized as --timings.";

const STABILISED_MULTITARGET: &str = "Multiple `--target` options are now always available.";

const STABILIZED_TERMINAL_WIDTH: &str =
    "The -Zterminal-width option is now always enabled for terminal output.";

const STABILISED_SPARSE_REGISTRY: &str = "The sparse protocol is now the default for crates.io";

fn deserialize_build_std<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let crates = match <Option<Vec<String>>>::deserialize(deserializer)? {
        Some(list) => list,
        None => return Ok(None),
    };
    let v = crates.join(",");
    Ok(Some(
        crate::core::compiler::standard_lib::parse_unstable_flag(Some(&v)),
    ))
}

fn deserialize_check_cfg<'de, D>(
    deserializer: D,
) -> Result<Option<(bool, bool, bool, bool)>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let crates = match <Option<Vec<String>>>::deserialize(deserializer)? {
        Some(list) => list,
        None => return Ok(None),
    };

    parse_check_cfg(crates.into_iter()).map_err(D::Error::custom)
}

#[derive(Debug, Copy, Clone, Default, Deserialize)]
pub struct GitoxideFeatures {
    /// All fetches are done with `gitoxide`, which includes git dependencies as well as the crates index.
    pub fetch: bool,
    /// When cloning the index, perform a shallow clone. Maintain shallowness upon subsequent fetches.
    pub shallow_index: bool,
    /// When cloning git dependencies, perform a shallow clone and maintain shallowness on subsequent fetches.
    pub shallow_deps: bool,
    /// Checkout git dependencies using `gitoxide` (submodules are still handled by git2 ATM, and filters
    /// like linefeed conversions are unsupported).
    pub checkout: bool,
    /// A feature flag which doesn't have any meaning except for preventing
    /// `__CARGO_USE_GITOXIDE_INSTEAD_OF_GIT2=1` builds to enable all safe `gitoxide` features.
    /// That way, `gitoxide` isn't actually used even though it's enabled.
    pub internal_use_git2: bool,
}

impl GitoxideFeatures {
    fn all() -> Self {
        GitoxideFeatures {
            fetch: true,
            shallow_index: true,
            checkout: true,
            shallow_deps: true,
            internal_use_git2: false,
        }
    }

    /// Features we deem safe for everyday use - typically true when all tests pass with them
    /// AND they are backwards compatible.
    fn safe() -> Self {
        GitoxideFeatures {
            fetch: true,
            shallow_index: false,
            checkout: true,
            shallow_deps: false,
            internal_use_git2: false,
        }
    }
}

fn parse_gitoxide(
    it: impl Iterator<Item = impl AsRef<str>>,
) -> CargoResult<Option<GitoxideFeatures>> {
    let mut out = GitoxideFeatures::default();
    let GitoxideFeatures {
        fetch,
        shallow_index,
        checkout,
        shallow_deps,
        internal_use_git2,
    } = &mut out;

    for e in it {
        match e.as_ref() {
            "fetch" => *fetch = true,
            "shallow-index" => *shallow_index = true,
            "shallow-deps" => *shallow_deps = true,
            "checkout" => *checkout = true,
            "internal-use-git2" => *internal_use_git2 = true,
            _ => {
                bail!("unstable 'gitoxide' only takes `fetch`, 'shallow-index', 'shallow-deps' and 'checkout' as valid inputs")
            }
        }
    }
    Ok(Some(out))
}

fn parse_check_cfg(
    it: impl Iterator<Item = impl AsRef<str>>,
) -> CargoResult<Option<(bool, bool, bool, bool)>> {
    let mut features = false;
    let mut well_known_names = false;
    let mut well_known_values = false;
    let mut output = false;

    for e in it {
        match e.as_ref() {
            "features" => features = true,
            "names" => well_known_names = true,
            "values" => well_known_values = true,
            "output" => output = true,
            _ => bail!("unstable check-cfg only takes `features`, `names`, `values` or `output` as valid inputs"),
        }
    }

    Ok(Some((
        features,
        well_known_names,
        well_known_values,
        output,
    )))
}

impl CliUnstable {
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

        fn parse_features(value: Option<&str>) -> Vec<String> {
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
                    iter_join(allowed, ", ")
                );
            }
        }

        match k {
            // Permanently unstable features
            // Sorted alphabetically:
            "allow-features" => self.allow_features = Some(parse_features(v).into_iter().collect()),
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
                let feats = parse_features(v);
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

            // Unstable features
            // Sorted alphabetically:
            "advanced-env" => self.advanced_env = parse_empty(k, v)?,
            "avoid-dev-deps" => self.avoid_dev_deps = parse_empty(k, v)?,
            "binary-dep-depinfo" => self.binary_dep_depinfo = parse_empty(k, v)?,
            "bindeps" => self.bindeps = parse_empty(k, v)?,
            "build-std" => {
                self.build_std = Some(crate::core::compiler::standard_lib::parse_unstable_flag(v))
            }
            "build-std-features" => self.build_std_features = Some(parse_features(v)),
            "check-cfg" => {
                self.check_cfg = v.map_or(Ok(None), |v| parse_check_cfg(v.split(',')))?
            }
            "codegen-backend" => self.codegen_backend = parse_empty(k, v)?,
            "config-include" => self.config_include = parse_empty(k, v)?,
            "credential-process" => self.credential_process = parse_empty(k, v)?,
            "direct-minimal-versions" => self.direct_minimal_versions = parse_empty(k, v)?,
            "doctest-in-workspace" => self.doctest_in_workspace = parse_empty(k, v)?,
            "doctest-xcompile" => self.doctest_xcompile = parse_empty(k, v)?,
            "dual-proc-macros" => self.dual_proc_macros = parse_empty(k, v)?,
            "gitoxide" => {
                self.gitoxide = v.map_or_else(
                    || Ok(Some(GitoxideFeatures::all())),
                    |v| parse_gitoxide(v.split(',')),
                )?
            }
            "host-config" => self.host_config = parse_empty(k, v)?,
            "jobserver-per-rustc" => self.jobserver_per_rustc = parse_empty(k, v)?,
            "lints" => self.lints = parse_empty(k, v)?,
            "minimal-versions" => self.minimal_versions = parse_empty(k, v)?,
            "msrv-policy" => self.msrv_policy = parse_empty(k, v)?,
            // can also be set in .cargo/config or with and ENV
            "mtime-on-use" => self.mtime_on_use = parse_empty(k, v)?,
            "no-index-update" => self.no_index_update = parse_empty(k, v)?,
            "panic-abort-tests" => self.panic_abort_tests = parse_empty(k, v)?,
            "profile-rustflags" => self.profile_rustflags = parse_empty(k, v)?,
            "publish-timeout" => self.publish_timeout = parse_empty(k, v)?,
            "registry-auth" => self.registry_auth = parse_empty(k, v)?,
            "rustdoc-map" => self.rustdoc_map = parse_empty(k, v)?,
            "rustdoc-scrape-examples" => self.rustdoc_scrape_examples = parse_empty(k, v)?,
            "separate-nightlies" => self.separate_nightlies = parse_empty(k, v)?,
            "skip-rustdoc-fingerprint" => self.skip_rustdoc_fingerprint = parse_empty(k, v)?,
            "target-applies-to-host" => self.target_applies_to_host = parse_empty(k, v)?,
            "unstable-options" => self.unstable_options = parse_empty(k, v)?,
            _ => bail!("unknown `-Z` flag specified: {}", k),
        }

        Ok(())
    }

    /// Generates an error if `-Z unstable-options` was not used for a new,
    /// unstable command-line flag.
    pub fn fail_if_stable_opt(&self, flag: &str, issue: u32) -> CargoResult<()> {
        if !self.unstable_options {
            let see = format!(
                "See https://github.com/rust-lang/cargo/issues/{issue} for more \
                 information about the `{flag}` flag."
            );
            // NOTE: a `config` isn't available here, check the channel directly
            let channel = channel();
            if channel == "nightly" || channel == "dev" {
                bail!(
                    "the `{flag}` flag is unstable, pass `-Z unstable-options` to enable it\n\
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
        config: &Config,
        command: &str,
        issue: u32,
    ) -> CargoResult<()> {
        if self.unstable_options {
            return Ok(());
        }
        let see = format!(
            "See https://github.com/rust-lang/cargo/issues/{} for more \
            information about the `cargo {}` command.",
            issue, command
        );
        if config.nightly_features_allowed {
            bail!(
                "the `cargo {}` command is unstable, pass `-Z unstable-options` to enable it\n\
                 {}",
                command,
                see
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
    // ALLOWED: the process of rustc boostrapping reads this through
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
