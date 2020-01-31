//! Support for nightly features in Cargo itself.
//!
//! This file is the version of `feature_gate.rs` in upstream Rust for Cargo
//! itself and is intended to be the avenue for which new features in Cargo are
//! gated by default and then eventually stabilized. All known stable and
//! unstable features are tracked in this file.
//!
//! If you're reading this then you're likely interested in adding a feature to
//! Cargo, and the good news is that it shouldn't be too hard! To do this you'll
//! want to follow these steps:
//!
//! 1. Add your feature. Do this by searching for "look here" in this file and
//!    expanding the macro invocation that lists all features with your new
//!    feature.
//!
//! 2. Find the appropriate place to place the feature gate in Cargo itself. If
//!    you're extending the manifest format you'll likely just want to modify
//!    the `Manifest::feature_gate` function, but otherwise you may wish to
//!    place the feature gate elsewhere in Cargo.
//!
//! 3. To actually perform the feature gate, you'll want to have code that looks
//!    like:
//!
//! ```rust,compile_fail
//! use core::{Feature, Features};
//!
//! let feature = Feature::launch_into_space();
//! package.manifest().features().require(feature).chain_err(|| {
//!     "launching Cargo into space right now is unstable and may result in \
//!      unintended damage to your codebase, use with caution"
//! })?;
//! ```
//!
//! Notably you'll notice the `require` function called with your `Feature`, and
//! then you use `chain_err` to tack on more context for why the feature was
//! required when the feature isn't activated.
//!
//! 4. Update the unstable documentation at
//!    `src/doc/src/reference/unstable.md` to include a short description of
//!    how to use your new feature. When the feature is stabilized, be sure
//!    that the Cargo Guide or Reference is updated to fully document the
//!    feature and remove the entry from the Unstable section.
//!
//! And hopefully that's it! Bear with us though that this is, at the time of
//! this writing, a very new feature in Cargo. If the process differs from this
//! we'll be sure to update this documentation!

use std::cell::Cell;
use std::env;
use std::fmt;
use std::str::FromStr;

use anyhow::{bail, Error};
use serde::{Deserialize, Serialize};

use crate::util::errors::CargoResult;

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
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Edition::Edition2015 => f.write_str("2015"),
            Edition::Edition2018 => f.write_str("2018"),
        }
    }
}
impl FromStr for Edition {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "2015" => Ok(Edition::Edition2015),
            "2018" => Ok(Edition::Edition2018),
            s => bail!(
                "supported edition values are `2015` or `2018`, but `{}` \
                 is unknown",
                s
            ),
        }
    }
}

#[derive(PartialEq)]
enum Status {
    Stable,
    Unstable,
}

macro_rules! features {
    (
        pub struct Features {
            $([$stab:ident] $feature:ident: bool,)*
        }
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
            fn status(&mut self, feature: &str) -> Option<(&mut bool, Status)> {
                if feature.contains("_") {
                    return None
                }
                let feature = feature.replace("-", "_");
                $(
                    if feature == stringify!($feature) {
                        return Some((&mut self.$feature, stab!($stab)))
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
    pub struct Features {

        // A dummy feature that doesn't actually gate anything, but it's used in
        // testing to ensure that we can enable stable features.
        [stable] test_dummy_stable: bool,

        // A dummy feature that gates the usage of the `im-a-teapot` manifest
        // entry. This is basically just intended for tests.
        [unstable] test_dummy_unstable: bool,

        // Downloading packages from alternative registry indexes.
        [stable] alternative_registries: bool,

        // Using editions
        [stable] edition: bool,

        // Renaming a package in the manifest via the `package` key
        [stable] rename_dependency: bool,

        // Whether a lock file is published with this crate
        // This is deprecated, and will likely be removed in a future version.
        [unstable] publish_lockfile: bool,

        // Overriding profiles for dependencies.
        [stable] profile_overrides: bool,

        // Separating the namespaces for features and dependencies
        [unstable] namespaced_features: bool,

        // "default-run" manifest option,
        [stable] default_run: bool,

        // Declarative build scripts.
        [unstable] metabuild: bool,

        // Specifying the 'public' attribute on dependencies
        [unstable] public_dependency: bool,

        // Allow to specify profiles other than 'dev', 'release', 'test', etc.
        [unstable] named_profiles: bool,
    }
}

pub struct Feature {
    name: &'static str,
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

    fn add(&mut self, feature: &str, warnings: &mut Vec<String>) -> CargoResult<()> {
        let (slot, status) = match self.status(feature) {
            Some(p) => p,
            None => bail!("unknown cargo feature `{}`", feature),
        };

        if *slot {
            bail!("the cargo feature `{}` has already been activated", feature);
        }

        match status {
            Status::Stable => {
                let warning = format!(
                    "the cargo feature `{}` is now stable \
                     and is no longer necessary to be listed \
                     in the manifest",
                    feature
                );
                warnings.push(warning);
            }
            Status::Unstable if !nightly_features_allowed() => bail!(
                "the cargo feature `{}` requires a nightly version of \
                 Cargo, but this is the `{}` channel\n\
                 {}",
                feature,
                channel(),
                SEE_CHANNELS
            ),
            Status::Unstable => {}
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
///
/// This struct doesn't have quite the same convenience macro that the features
/// have above, but the procedure should still be relatively stable for adding a
/// new unstable flag:
///
/// 1. First, add a field to this `CliUnstable` structure. All flags are allowed
///    to have a value as the `-Z` flags are either of the form `-Z foo` or
///    `-Z foo=bar`, and it's up to you how to parse `bar`.
///
/// 2. Add an arm to the match statement in `CliUnstable::add` below to match on
///    your new flag. The key (`k`) is what you're matching on and the value is
///    in `v`.
///
/// 3. (optional) Add a new parsing function to parse your datatype. As of now
///    there's an example for `bool`, but more can be added!
///
/// 4. In Cargo use `config.cli_unstable()` to get a reference to this structure
///    and then test for your flag or your value and act accordingly.
///
/// If you have any trouble with this, please let us know!
#[derive(Default, Debug)]
pub struct CliUnstable {
    pub print_im_a_teapot: bool,
    pub unstable_options: bool,
    pub no_index_update: bool,
    pub avoid_dev_deps: bool,
    pub minimal_versions: bool,
    pub package_features: bool,
    pub advanced_env: bool,
    pub config_profile: bool,
    pub config_include: bool,
    pub dual_proc_macros: bool,
    pub mtime_on_use: bool,
    pub named_profiles: bool,
    pub binary_dep_depinfo: bool,
    pub build_std: Option<Vec<String>>,
    pub timings: Option<Vec<String>>,
    pub doctest_xcompile: bool,
    pub panic_abort_tests: bool,
}

impl CliUnstable {
    pub fn parse(&mut self, flags: &[String]) -> CargoResult<()> {
        if !flags.is_empty() && !nightly_features_allowed() {
            bail!(
                "the `-Z` flag is only accepted on the nightly channel of Cargo, \
                 but this is the `{}` channel\n\
                 {}",
                channel(),
                SEE_CHANNELS
            );
        }
        for flag in flags {
            self.add(flag)?;
        }
        Ok(())
    }

    fn add(&mut self, flag: &str) -> CargoResult<()> {
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

        // Asserts that there is no argument to the flag.
        fn parse_empty(key: &str, value: Option<&str>) -> CargoResult<bool> {
            if let Some(v) = value {
                bail!("flag -Z{} does not take a value, found: `{}`", key, v);
            }
            Ok(true)
        };

        match k {
            "print-im-a-teapot" => self.print_im_a_teapot = parse_bool(k, v)?,
            "unstable-options" => self.unstable_options = parse_empty(k, v)?,
            "no-index-update" => self.no_index_update = parse_empty(k, v)?,
            "avoid-dev-deps" => self.avoid_dev_deps = parse_empty(k, v)?,
            "minimal-versions" => self.minimal_versions = parse_empty(k, v)?,
            "package-features" => self.package_features = parse_empty(k, v)?,
            "advanced-env" => self.advanced_env = parse_empty(k, v)?,
            "config-profile" => self.config_profile = parse_empty(k, v)?,
            "config-include" => self.config_include = parse_empty(k, v)?,
            "dual-proc-macros" => self.dual_proc_macros = parse_empty(k, v)?,
            // can also be set in .cargo/config or with and ENV
            "mtime-on-use" => self.mtime_on_use = parse_empty(k, v)?,
            "named-profiles" => self.named_profiles = parse_empty(k, v)?,
            "binary-dep-depinfo" => self.binary_dep_depinfo = parse_empty(k, v)?,
            "build-std" => {
                self.build_std = Some(crate::core::compiler::standard_lib::parse_unstable_flag(v))
            }
            "timings" => self.timings = Some(parse_timings(v)),
            "doctest-xcompile" => self.doctest_xcompile = parse_empty(k, v)?,
            "panic-abort-tests" => self.panic_abort_tests = parse_empty(k, v)?,
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
