//! Support for nightly features in Cargo itself
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
//! 3. Do actually perform the feature gate, you'll want to have code that looks
//!    like:
//!
//! ```rust,ignore
//! use core::{Feature, Features};
//!
//! let feature = Feature::launch_into_space();
//! package.manifest().features().require(feature).chain_err(|| {
//!     "launching Cargo into space right now is unstable and may result in \
//!      unintended damage to your codebase, use with caution"
//! })?;
//! ```
//!
//! Notably you'll notice the `require` funciton called with your `Feature`, and
//! then you use `chain_err` to tack on more context for why the feature was
//! required when the feature isn't activated.
//!
//! And hopefully that's it! Bear with us though that this is, at the time of
//! this writing, a very new feature in Cargo. If the process differs from this
//! we'll be sure to update this documentation!

use std::env;

use util::errors::CargoResult;

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
                        features.$feature
                    }
                    static FEAT: Feature = Feature {
                        name: stringify!($feature),
                        get: get,
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
    (stable) => (Status::Stable);
    (unstable) => (Status::Unstable);
}

/// A listing of all features in Cargo
///
/// "look here"
///
/// This is the macro that lists all stable and unstable features in Cargo.
/// You'll want to add to this macro whenever you add a feature to Cargo, also
/// following the directions above.
///
/// Note that all feature names here are valid Rust identifiers, but the `_`
/// character is translated to `-` when specified in the `cargo-features`
/// manifest entry in `Cargo.toml`.
features! {
    pub struct Features {

        // A dummy feature that doesn't actually gate anything, but it's used in
        // testing to ensure that we can enable stable features.
        [stable] test_dummy_stable: bool,

        // A dummy feature that gates the usage of the `im-a-teapot` manifest
        // entry. This is basically just intended for tests.
        [unstable] test_dummy_unstable: bool,
    }
}

pub struct Feature {
    name: &'static str,
    get: fn(&Features) -> bool,
}

impl Features {
    pub fn new(features: &[String],
               warnings: &mut Vec<String>) -> CargoResult<Features> {
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
            bail!("the cargo feature `{}` has already bene activated", feature);
        }

        match status {
            Status::Stable => {
                let warning = format!("the cargo feature `{}` is now stable \
                                       and is no longer necessary to be listed \
                                       in the manifest", feature);
                warnings.push(warning);
            }
            Status::Unstable if !nightly_features_allowed() => {
                bail!("the cargo feature `{}` requires a nightly version of \
                       Cargo, but this is the `{}` channel",
                      feature,
                      channel())
            }
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
                let s = format!("\n\nconsider adding `cargo-features = [\"{0}\"]` \
                                 to the manifest", feature);
                msg.push_str(&s);
            } else {
                let s = format!("\n\n\
                    this Cargo does not support nightly features, but if you\n\
                    switch to nightly channel you can add\n\
                    `cargo-features = [\"{}\"]` to enable this feature",
                    feature);
                msg.push_str(&s);
            }
            bail!("{}", msg);
        }
    }
}

/// A parsed represetnation of all unstable flags that Cargo accepts.
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
}

impl CliUnstable {
    pub fn parse(&mut self, flags: &[String]) -> CargoResult<()> {
        if !flags.is_empty() && !nightly_features_allowed() {
            bail!("the `-Z` flag is only accepted on the nightly channel of Cargo")
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

        fn parse_bool(value: Option<&str>) -> CargoResult<bool> {
            match value {
                None |
                Some("yes") => Ok(true),
                Some("no") => Ok(false),
                Some(s) => bail!("expected `no` or `yes`, found: {}", s),
            }
        }

        match k {
            "print-im-a-teapot" => self.print_im_a_teapot = parse_bool(v)?,
            _ => bail!("unknown `-Z` flag specified: {}", k),
        }

        Ok(())
    }
}

fn channel() -> String {
    env::var("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS").unwrap_or_else(|_| {
        ::version().cfg_info.map(|c| c.release_channel)
            .unwrap_or_else(|| String::from("dev"))
    })
}

fn nightly_features_allowed() -> bool {
    match &channel()[..] {
        "nightly" | "dev" => true,
        _ => false,
    }
}
