//! Encapsulates snapshotting of environment variables.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};

use crate::util::errors::CargoResult;
use anyhow::{anyhow, bail};

/// Generate `case_insensitive_env` and `normalized_env` from the `env`.
fn make_case_insensitive_and_normalized_env(
    env: &HashMap<OsString, OsString>,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let case_insensitive_env: HashMap<_, _> = env
        .keys()
        .filter_map(|k| k.to_str())
        .map(|k| (k.to_uppercase(), k.to_owned()))
        .collect();
    let normalized_env = env
        .iter()
        // Only keep entries where both the key and value are valid UTF-8,
        // since the config env vars only support UTF-8 keys and values.
        // Otherwise, the normalized map warning could incorrectly warn about entries that can't be
        // read by the config system.
        // Please see the docs for `Env` for more context.
        .filter_map(|(k, v)| Some((k.to_str()?, v.to_str()?)))
        .map(|(k, _)| (k.to_uppercase().replace("-", "_"), k.to_owned()))
        .collect();
    (case_insensitive_env, normalized_env)
}

/// A snapshot of the environment variables available to [`super::Config`].
///
/// Currently, the [`Config`](super::Config) supports lookup of environment variables
/// through two different means:
///
/// - [`Config::get_env`](super::Config::get_env)
///   and [`Config::get_env_os`](super::Config::get_env_os)
///   for process environment variables (similar to [`std::env::var`] and [`std::env::var_os`]),
/// - Typed Config Value API via [`Config::get`](super::Config::get).
///   This is only available for `CARGO_` prefixed environment keys.
///
/// This type contains the env var snapshot and helper methods for both APIs.
#[derive(Debug)]
pub struct Env {
    /// A snapshot of the process's environment variables.
    env: HashMap<OsString, OsString>,
    /// Used in the typed Config value API for warning messages when config keys are
    /// given in the wrong format.
    ///
    /// Maps from "normalized" (upper case and with "-" replaced by "_") env keys
    /// to the actual keys in the environment.
    /// The normalized format is the one expected by Cargo.
    ///
    /// This only holds env keys that are valid UTF-8, since [`super::ConfigKey`] only supports UTF-8 keys.
    /// In addition, this only holds env keys whose value in the environment is also valid UTF-8,
    /// since the typed Config value API only supports UTF-8 values.
    normalized_env: HashMap<String, String>,
    /// Used to implement `get_env` and `get_env_os` on Windows, where env keys are case-insensitive.
    ///
    /// Maps from uppercased env keys to the actual key in the environment.
    /// For example, this might hold a pair `("PATH", "Path")`.
    /// Currently only supports UTF-8 keys and values.
    case_insensitive_env: HashMap<String, String>,
}

impl Env {
    /// Create a new `Env` from process's environment variables.
    pub fn new() -> Self {
        // ALLOWED: This is the only permissible usage of `std::env::vars{_os}`
        // within cargo. If you do need access to individual variables without
        // interacting with `Config` system, please use `std::env::var{_os}`
        // and justify the validity of the usage.
        #[allow(clippy::disallowed_methods)]
        let env: HashMap<_, _> = std::env::vars_os().collect();
        let (case_insensitive_env, normalized_env) = make_case_insensitive_and_normalized_env(&env);
        Self {
            env,
            case_insensitive_env,
            normalized_env,
        }
    }

    /// Set the env directly from a `HashMap`.
    /// This should be used for debugging purposes only.
    pub(super) fn from_map(env: HashMap<String, String>) -> Self {
        let env = env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        let (case_insensitive_env, normalized_env) = make_case_insensitive_and_normalized_env(&env);
        Self {
            env,
            case_insensitive_env,
            normalized_env,
        }
    }

    /// Returns all environment variables as an iterator,
    /// keeping only entries where both the key and value are valid UTF-8.
    pub fn iter_str(&self) -> impl Iterator<Item = (&str, &str)> {
        self.env
            .iter()
            .filter_map(|(k, v)| Some((k.to_str()?, v.to_str()?)))
    }

    /// Returns all environment variable keys, filtering out keys that are not valid UTF-8.
    pub fn keys_str(&self) -> impl Iterator<Item = &str> {
        self.env.keys().filter_map(|k| k.to_str())
    }

    /// Get the value of environment variable `key` through the `Config` snapshot.
    ///
    /// This can be used similarly to `std::env::var_os`.
    /// On Windows, we check for case mismatch since environment keys are case-insensitive.
    pub fn get_env_os(&self, key: impl AsRef<OsStr>) -> Option<OsString> {
        match self.env.get(key.as_ref()) {
            Some(s) => Some(s.clone()),
            None => {
                if cfg!(windows) {
                    self.get_env_case_insensitive(key).cloned()
                } else {
                    None
                }
            }
        }
    }

    /// Get the value of environment variable `key` through the `self.env` snapshot.
    ///
    /// This can be used similarly to `std::env::var`.
    /// On Windows, we check for case mismatch since environment keys are case-insensitive.
    pub fn get_env(&self, key: impl AsRef<OsStr>) -> CargoResult<String> {
        let key = key.as_ref();
        let s = self
            .get_env_os(key)
            .ok_or_else(|| anyhow!("{key:?} could not be found in the environment snapshot"))?;

        match s.to_str() {
            Some(s) => Ok(s.to_owned()),
            None => bail!("environment variable value is not valid unicode: {s:?}"),
        }
    }

    /// Performs a case-insensitive lookup of `key` in the environment.
    ///
    /// This is relevant on Windows, where environment variables are case-insensitive.
    /// Note that this only works on keys that are valid UTF-8 and it uses Unicode uppercase,
    /// which may differ from the OS's notion of uppercase.
    fn get_env_case_insensitive(&self, key: impl AsRef<OsStr>) -> Option<&OsString> {
        let upper_case_key = key.as_ref().to_str()?.to_uppercase();
        let env_key: &OsStr = self.case_insensitive_env.get(&upper_case_key)?.as_ref();
        self.env.get(env_key)
    }

    /// Get the value of environment variable `key` as a `&str`.
    /// Returns `None` if `key` is not in `self.env` or if the value is not valid UTF-8.
    ///
    /// This is intended for use in private methods of `Config`,
    /// and does not check for env key case mismatch.
    ///
    /// This is case-sensitive on Windows (even though environment keys on Windows are usually
    /// case-insensitive) due to an unintended regression in 1.28 (via #5552).
    /// This should only affect keys used for cargo's config-system env variables (`CARGO_`
    /// prefixed ones), which are currently all uppercase.
    /// We may want to consider rectifying it if users report issues.
    /// One thing that adds a wrinkle here is the unstable advanced-env option that *requires*
    /// case-sensitive keys.
    ///
    /// Do not use this for any other purposes.
    /// Use [`Env::get_env_os`] or [`Env::get_env`] instead, which properly handle case
    /// insensitivity on Windows.
    pub(super) fn get_str(&self, key: impl AsRef<OsStr>) -> Option<&str> {
        self.env.get(key.as_ref()).and_then(|s| s.to_str())
    }

    /// Check if the environment contains `key`.
    ///
    /// This is intended for use in private methods of `Config`,
    /// and does not check for env key case mismatch.
    /// See the docstring of [`Env::get_str`] for more context.
    pub(super) fn contains_key(&self, key: impl AsRef<OsStr>) -> bool {
        self.env.contains_key(key.as_ref())
    }

    /// Looks up a normalized `key` in the `normalized_env`.
    /// Returns the corresponding (non-normalized) env key if it exists, else `None`.
    ///
    /// This is used by [`super::Config::check_environment_key_case_mismatch`].
    pub(super) fn get_normalized(&self, key: &str) -> Option<&str> {
        self.normalized_env.get(key).map(|s| s.as_ref())
    }
}
