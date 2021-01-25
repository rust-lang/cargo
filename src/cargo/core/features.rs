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
//!    find the `features!` macro and add your feature to the list.
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
//! The steps to add a new `-Z` option are:
//!
//! 1. Add the option to the [`CliUnstable`] struct below. Flags can take an
//!    optional value if you want.
//! 2. Update the [`CliUnstable::add`] function to parse the flag.
//! 3. Wherever the new functionality is implemented, call
//!    [`Config::cli_unstable`][crate::util::config::Config::cli_unstable] to
//!    get an instance of `CliUnstable` and check if the option has been
//!    enabled on the `CliUnstable` instance. Nightly gating is already
//!    handled, so no need to worry about that.
//! 4. Update the `-Z help` documentation in the `main` function.
//!
//! ## Stabilization
//!
//! For the stabilization process, see
//! <https://doc.crates.io/contrib/process/unstable.html#stabilization>.
//!
//! The steps for stabilizing are roughly:
//!
//! 1. Update the feature to be stable, based on the kind of feature:
//!   1. `cargo-features`: Change the feature to `stable` in the `features!`
//!      macro below.
//!   2. `-Z unstable-options`: Find the call to `fail_if_stable_opt` and
//!      remove it. Be sure to update the man pages if necessary.
//!   3. `-Z` flag: Change the parsing code in [`CliUnstable::add`] to call
//!      `stabilized_warn` or `stabilized_err`. Remove it from the `-Z help`
//!      docs in the `main` function. Remove the `(unstable)` note in the
//!      clap help text if necessary.
//! 2. Remove `masquerade_as_nightly_cargo` from any tests, and remove
//!    `cargo-features` from `Cargo.toml` test files if any.
//! 3. Remove the docs from unstable.md and update the redirect at the bottom
//!    of that page. Update the rest of the documentation to add the new
//!    feature.

use std::cell::Cell;
use std::env;
use std::fmt;
use std::str::FromStr;

use anyhow::{bail, Error};
use serde::{Deserialize, Serialize};

use crate::util::errors::CargoResult;
use crate::util::indented_lines;

pub const SEE_CHANNELS: &str =
    "See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information \
     about Rust release channels.";

/// The edition of the compiler (RFC 2052)
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
    pub(crate) fn first_version(&self) -> Option<semver::Version> {
        use Edition::*;
        match self {
            Edition2015 => None,
            Edition2018 => Some(semver::Version::new(1, 31, 0)),
            Edition2021 => Some(semver::Version::new(1, 62, 0)),
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
    (removed, publish_lockfile, "", PUBLISH_LOCKFILE_REMOVED),

    // Overriding profiles for dependencies.
    (stable, profile_overrides, "1.41", "reference/profiles.html#overrides"),

    // "default-run" manifest option,
    (stable, default_run, "1.37", "reference/manifest.html#the-default-run-field"),

    // Declarative build scripts.
    (unstable, metabuild, "", "reference/unstable.html#metabuild"),

    // Specifying the 'public' attribute on dependencies
    (unstable, public_dependency, "", "reference/unstable.html#public-dependency"),

    // Allow to specify profiles other than 'dev', 'release', 'test', etc.
    (unstable, named_profiles, "", "reference/unstable.html#custom-named-profiles"),

    // Opt-in new-resolver behavior.
    (stable, resolver, "1.51", "reference/resolver.html#resolver-versions"),

    // Allow to specify whether binaries should be stripped.
    (unstable, strip, "", "reference/unstable.html#profile-strip-option"),

    // Specifying a minimal 'rust-version' attribute for crates
    (unstable, rust_version, "", "reference/unstable.html#rust-version"),
}

const PUBLISH_LOCKFILE_REMOVED: &str = "The publish-lockfile key in Cargo.toml \
    has been removed. The Cargo.lock file is always included when a package is \
    published if the package contains a binary target. `cargo install` requires \
    the `--locked` flag to use the Cargo.lock file.\n\
    See https://doc.rust-lang.org/cargo/commands/cargo-package.html and \
    https://doc.rust-lang.org/cargo/commands/cargo-install.html for more \
    information.";

pub struct Feature {
    name: &'static str,
    stability: Status,
    version: &'static str,
    docs: &'static str,
    get: fn(&Features) -> bool,
}

impl Features {
    pub fn new(features: &[String], warnings: &mut Vec<String>) -> CargoResult<Features> {
        let mut ret = Features::default();
        for feature in features {
            ret.add(feature, warnings)?;
            ret.activated.push(feature.to_string());
        }
        Ok(ret)
    }

    fn add(&mut self, feature_name: &str, warnings: &mut Vec<String>) -> CargoResult<()> {
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
            Status::Unstable if !nightly_features_allowed() => bail!(
                "the cargo feature `{}` requires a nightly version of \
                 Cargo, but this is the `{}` channel\n\
                 {}\n{}",
                feature_name,
                channel(),
                SEE_CHANNELS,
                see_docs()
            ),
            Status::Unstable => {}
            Status::Removed => bail!(
                "the cargo feature `{}` has been removed\n\
                Remove the feature from Cargo.toml to remove this error.\n\
                {}",
                feature_name,
                feature.docs
            ),
        }

        *slot = true;

        Ok(())
    }

    pub fn activated(&self) -> &[String] {
        &self.activated
    }

    pub fn require(&self, feature: &Feature) -> CargoResult<()> {
        if feature.is_enabled(self) {
            Ok(())
        } else {
            let feature = feature.name.replace("_", "-");
            let mut msg = format!("feature `{}` is required", feature);

            if nightly_features_allowed() {
                let s = format!(
                    "\n\nconsider adding `cargo-features = [\"{0}\"]` \
                     to the manifest",
                    feature
                );
                msg.push_str(&s);
            } else {
                let s = format!(
                    "\n\n\
                     this Cargo does not support nightly features, but if you\n\
                     switch to nightly channel you can add\n\
                     `cargo-features = [\"{}\"]` to enable this feature",
                    feature
                );
                msg.push_str(&s);
            }
            bail!("{}", msg);
        }
    }

    pub fn is_enabled(&self, feature: &Feature) -> bool {
        feature.is_enabled(self)
    }
}

/// A parsed representation of all unstable flags that Cargo accepts.
///
/// Cargo, like `rustc`, accepts a suite of `-Z` flags which are intended for
/// gating unstable functionality to Cargo. These flags are only available on
/// the nightly channel of Cargo.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct CliUnstable {
    pub print_im_a_teapot: bool,
    pub unstable_options: bool,
    pub no_index_update: bool,
    pub avoid_dev_deps: bool,
    pub minimal_versions: bool,
    pub advanced_env: bool,
    pub config_include: bool,
    pub dual_proc_macros: bool,
    pub mtime_on_use: bool,
    pub named_profiles: bool,
    pub binary_dep_depinfo: bool,
    #[serde(deserialize_with = "deserialize_build_std")]
    pub build_std: Option<Vec<String>>,
    pub build_std_features: Option<Vec<String>>,
    pub timings: Option<Vec<String>>,
    pub doctest_xcompile: bool,
    pub panic_abort_tests: bool,
    pub jobserver_per_rustc: bool,
    pub features: Option<Vec<String>>,
    pub separate_nightlies: bool,
    pub multitarget: bool,
    pub rustdoc_map: bool,
    pub terminal_width: Option<Option<usize>>,
    pub namespaced_features: bool,
    pub weak_dep_features: bool,
    pub extra_link_arg: bool,
    pub credential_process: bool,
}

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

impl CliUnstable {
    pub fn parse(&mut self, flags: &[String]) -> CargoResult<Vec<String>> {
        if !flags.is_empty() && !nightly_features_allowed() {
            bail!(
                "the `-Z` flag is only accepted on the nightly channel of Cargo, \
                 but this is the `{}` channel\n\
                 {}",
                channel(),
                SEE_CHANNELS
            );
        }
        let mut warnings = Vec::new();
        for flag in flags {
            self.add(flag, &mut warnings)?;
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

        fn parse_timings(value: Option<&str>) -> Vec<String> {
            match value {
                None => vec!["html".to_string(), "info".to_string()],
                Some(v) => v.split(',').map(|s| s.to_string()).collect(),
            }
        }

        fn parse_features(value: Option<&str>) -> Vec<String> {
            match value {
                None => Vec::new(),
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

        fn parse_usize_opt(value: Option<&str>) -> CargoResult<Option<usize>> {
            Ok(match value {
                Some(value) => match value.parse::<usize>() {
                    Ok(value) => Some(value),
                    Err(e) => bail!("expected a number, found: {}", e),
                },
                None => None,
            })
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

        match k {
            "print-im-a-teapot" => self.print_im_a_teapot = parse_bool(k, v)?,
            "unstable-options" => self.unstable_options = parse_empty(k, v)?,
            "no-index-update" => self.no_index_update = parse_empty(k, v)?,
            "avoid-dev-deps" => self.avoid_dev_deps = parse_empty(k, v)?,
            "minimal-versions" => self.minimal_versions = parse_empty(k, v)?,
            "advanced-env" => self.advanced_env = parse_empty(k, v)?,
            "config-include" => self.config_include = parse_empty(k, v)?,
            "dual-proc-macros" => self.dual_proc_macros = parse_empty(k, v)?,
            // can also be set in .cargo/config or with and ENV
            "mtime-on-use" => self.mtime_on_use = parse_empty(k, v)?,
            "named-profiles" => self.named_profiles = parse_empty(k, v)?,
            "binary-dep-depinfo" => self.binary_dep_depinfo = parse_empty(k, v)?,
            "build-std" => {
                self.build_std = Some(crate::core::compiler::standard_lib::parse_unstable_flag(v))
            }
            "build-std-features" => self.build_std_features = Some(parse_features(v)),
            "timings" => self.timings = Some(parse_timings(v)),
            "doctest-xcompile" => self.doctest_xcompile = parse_empty(k, v)?,
            "panic-abort-tests" => self.panic_abort_tests = parse_empty(k, v)?,
            "jobserver-per-rustc" => self.jobserver_per_rustc = parse_empty(k, v)?,
            "features" => {
                // For now this is still allowed (there are still some
                // unstable options like "compare"). This should be removed at
                // some point, and migrate to a new -Z flag for any future
                // things.
                let feats = parse_features(v);
                let stab: Vec<_> = feats
                    .iter()
                    .filter(|feat| {
                        matches!(
                            feat.as_str(),
                            "build_dep" | "host_dep" | "dev_dep" | "itarget" | "all"
                        )
                    })
                    .collect();
                if !stab.is_empty() || feats.is_empty() {
                    // Make this stabilized_err once -Zfeature support is removed.
                    stabilized_warn(k, "1.51", STABILIZED_FEATURES);
                }
                self.features = Some(feats);
            }
            "separate-nightlies" => self.separate_nightlies = parse_empty(k, v)?,
            "multitarget" => self.multitarget = parse_empty(k, v)?,
            "rustdoc-map" => self.rustdoc_map = parse_empty(k, v)?,
            "terminal-width" => self.terminal_width = Some(parse_usize_opt(v)?),
            "namespaced-features" => self.namespaced_features = parse_empty(k, v)?,
            "weak-dep-features" => self.weak_dep_features = parse_empty(k, v)?,
            "extra-link-arg" => self.extra_link_arg = parse_empty(k, v)?,
            "credential-process" => self.credential_process = parse_empty(k, v)?,
            "compile-progress" => stabilized_warn(k, "1.30", STABILIZED_COMPILE_PROGRESS),
            "offline" => stabilized_err(k, "1.36", STABILIZED_OFFLINE)?,
            "cache-messages" => stabilized_warn(k, "1.40", STABILIZED_CACHE_MESSAGES),
            "install-upgrade" => stabilized_warn(k, "1.41", STABILIZED_INSTALL_UPGRADE),
            "config-profile" => stabilized_warn(k, "1.43", STABILIZED_CONFIG_PROFILE),
            "crate-versions" => stabilized_warn(k, "1.47", STABILIZED_CRATE_VERSIONS),
            "package-features" => stabilized_warn(k, "1.51", STABILIZED_PACKAGE_FEATURES),
            _ => bail!("unknown `-Z` flag specified: {}", k),
        }

        Ok(())
    }

    /// Generates an error if `-Z unstable-options` was not used.
    /// Intended to be used when a user passes a command-line flag that
    /// requires `-Z unstable-options`.
    pub fn fail_if_stable_opt(&self, flag: &str, issue: u32) -> CargoResult<()> {
        if !self.unstable_options {
            let see = format!(
                "See https://github.com/rust-lang/cargo/issues/{} for more \
                 information about the `{}` flag.",
                issue, flag
            );
            if nightly_features_allowed() {
                bail!(
                    "the `{}` flag is unstable, pass `-Z unstable-options` to enable it\n\
                     {}",
                    flag,
                    see
                );
            } else {
                bail!(
                    "the `{}` flag is unstable, and only available on the nightly channel \
                     of Cargo, but this is the `{}` channel\n\
                     {}\n\
                     {}",
                    flag,
                    channel(),
                    SEE_CHANNELS,
                    see
                );
            }
        }
        Ok(())
    }
}

/// Returns the current release channel ("stable", "beta", "nightly", "dev").
pub fn channel() -> String {
    if let Ok(override_channel) = env::var("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS") {
        return override_channel;
    }
    if let Ok(staging) = env::var("RUSTC_BOOTSTRAP") {
        if staging == "1" {
            return "dev".to_string();
        }
    }
    crate::version()
        .cfg_info
        .map(|c| c.release_channel)
        .unwrap_or_else(|| String::from("dev"))
}

thread_local!(
    static NIGHTLY_FEATURES_ALLOWED: Cell<bool> = Cell::new(false);
    static ENABLE_NIGHTLY_FEATURES: Cell<bool> = Cell::new(false);
);

/// This is a little complicated.
/// This should return false if:
/// - this is an artifact of the rustc distribution process for "stable" or for "beta"
/// - this is an `#[test]` that does not opt in with `enable_nightly_features`
/// - this is a integration test that uses `ProcessBuilder`
///      that does not opt in with `masquerade_as_nightly_cargo`
/// This should return true if:
/// - this is an artifact of the rustc distribution process for "nightly"
/// - this is being used in the rustc distribution process internally
/// - this is a cargo executable that was built from source
/// - this is an `#[test]` that called `enable_nightly_features`
/// - this is a integration test that uses `ProcessBuilder`
///       that called `masquerade_as_nightly_cargo`
pub fn nightly_features_allowed() -> bool {
    if ENABLE_NIGHTLY_FEATURES.with(|c| c.get()) {
        return true;
    }
    match &channel()[..] {
        "nightly" | "dev" => NIGHTLY_FEATURES_ALLOWED.with(|c| c.get()),
        _ => false,
    }
}

/// Allows nightly features to be enabled for this thread, but only if the
/// development channel is nightly or dev.
///
/// Used by cargo main to ensure that a cargo build from source has nightly features
pub fn maybe_allow_nightly_features() {
    NIGHTLY_FEATURES_ALLOWED.with(|c| c.set(true));
}

/// Forcibly enables nightly features for this thread.
///
/// Used by tests to allow the use of nightly features.
pub fn enable_nightly_features() {
    ENABLE_NIGHTLY_FEATURES.with(|c| c.set(true));
}
