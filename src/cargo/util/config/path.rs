use crate::util::config::{Config, Value};
use serde::Deserialize;
use std::path::PathBuf;

/// Use with the `get` API to fetch a string that will be converted to a
/// `PathBuf`. Relative paths are converted to absolute paths based on the
/// location of the config file.
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(transparent)]
pub struct ConfigRelativePath(Value<String>);

impl ConfigRelativePath {
    /// Returns the raw underlying configuration value for this key.
    pub fn raw_value(&self) -> &str {
        &self.0.val
    }

    /// Resolves this configuration-relative path to an absolute path.
    ///
    /// This will always return an absolute path where it's relative to the
    /// location for configuration for this value.
    pub fn resolve_path(&self, config: &Config) -> PathBuf {
        self.0.definition.root(config).join(&self.0.val)
    }

    /// Resolves this configuration-relative path to either an absolute path or
    /// something appropriate to execute from `PATH`.
    ///
    /// Values which don't look like a filesystem path (don't contain `/` or
    /// `\`) will be returned as-is, and everything else will fall through to an
    /// absolute path.
    pub fn resolve_program(self, config: &Config) -> PathBuf {
        config.string_to_path(self.0.val, &self.0.definition)
    }
}
