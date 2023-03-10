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
        // Only keep entries where both the key and value are valid UTF-8
        .filter_map(|(k, v)| Some((k.to_str()?, v.to_str()?)))
        .map(|(k, _)| (k.to_uppercase().replace("-", "_"), k.to_owned()))
        .collect();
    (case_insensitive_env, normalized_env)
}

#[derive(Debug)]
pub struct Env {
    /// A snapshot of the process's environment variables.
    env: HashMap<OsString, OsString>,
    /// A map from normalized (upper case and with "-" replaced by "_") env keys to the actual key
    /// in the environment.
    /// The "normalized" key is the format expected by Cargo.
    /// This is used to warn users when env keys are not provided in this format.
    normalized_env: HashMap<String, String>,
    /// A map from uppercased env keys to the actual key in the environment.
    /// This is relevant on Windows, where env keys are case-insensitive.
    /// For example, this might hold a pair `("PATH", "Path")`.
    case_insensitive_env: HashMap<String, String>,
}

impl Env {
    /// Create a new `Env` from process's environment variables.
    pub fn new() -> Self {
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
    pub(super) fn get_str(&self, key: impl AsRef<OsStr>) -> Option<&str> {
        self.env.get(key.as_ref()).and_then(|s| s.to_str())
    }

    /// Check if the environment contains `key`.
    ///
    /// This is intended for use in private methods of `Config`,
    /// and does not check for env key case mismatch.
    pub(super) fn contains_key(&self, key: impl AsRef<OsStr>) -> bool {
        self.env.contains_key(key.as_ref())
    }

    /// Looks up a normalized `key` in the `normalized_env`.
    /// Returns the corresponding (non-normalized) env key if it exists, else `None`.
    ///
    /// This is used by `Config::check_environment_key_mismatch`.
    pub(super) fn get_normalized(&self, key: &str) -> Option<&str> {
        self.normalized_env.get(key).map(|s| s.as_ref())
    }
}
