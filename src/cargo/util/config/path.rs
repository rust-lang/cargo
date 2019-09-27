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
    pub fn resolve(self, config: &Config) -> PathBuf {
        config.string_to_path(self.0.val, &self.0.definition)
    }
}
