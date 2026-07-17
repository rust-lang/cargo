use super::{GlobalContext, StringList, Value};
use regex::Regex;
use serde::{Deserialize, de::Error};
use std::path::PathBuf;

/// Use with the `get` API to fetch a string that will be converted to a
/// `PathBuf`. Relative paths are converted to absolute paths based on the
/// location of the config file.
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(transparent)]
pub struct ConfigRelativePath(Value<String>);

impl ConfigRelativePath {
    pub fn new(path: Value<String>) -> ConfigRelativePath {
        ConfigRelativePath(path)
    }

    /// Returns the underlying value.
    pub fn value(&self) -> &Value<String> {
        &self.0
    }

    /// Returns the raw underlying configuration value for this key.
    pub fn raw_value(&self) -> &str {
        &self.0.val
    }

    /// Resolves this configuration-relative path to an absolute path.
    ///
    /// This will always return an absolute path where it's relative to the
    /// location for configuration for this value.
    pub fn resolve_path(&self, gctx: &GlobalContext) -> PathBuf {
        self.0.definition.root(gctx.cwd()).join(&self.0.val)
    }

    /// Same as [`Self::resolve_path`] but will make string replacements
    /// before resolving the path.
    ///
    /// `replacements` should be an [`IntoIterator`] of tuples with the "from" and "to" for the
    /// string replacement
    pub fn resolve_templated_path(
        &self,
        gctx: &GlobalContext,
        replacements: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
    ) -> Result<PathBuf, ResolveTemplateError> {
        let mut value = self.0.val.clone();

        for (from, to) in replacements {
            value = value.replace(from.as_ref(), to.as_ref());
        }

        // Check for expected variables
        let re = Regex::new(r"\{(.*)\}").unwrap();
        if let Some(caps) = re.captures(&value) {
            return Err(ResolveTemplateError::UnexpectedVariable {
                variable: caps[1].to_string(),
                raw_template: self.0.val.clone(),
            });
        };

        if value.contains("{") {
            return Err(ResolveTemplateError::UnexpectedBracket {
                bracket_type: BracketType::Opening,
                raw_template: self.0.val.clone(),
            });
        }

        if value.contains("}") {
            return Err(ResolveTemplateError::UnexpectedBracket {
                bracket_type: BracketType::Closing,
                raw_template: self.0.val.clone(),
            });
        }

        Ok(self.0.definition.root(gctx.cwd()).join(&value))
    }

    /// Resolves this configuration-relative path to either an absolute path or
    /// something appropriate to execute from `PATH`.
    ///
    /// Values which don't look like a filesystem path (don't contain `/` or
    /// `\`) will be returned as-is, and everything else will fall through to an
    /// absolute path.
    pub fn resolve_program(&self, gctx: &GlobalContext) -> PathBuf {
        gctx.string_to_path(&self.0.val, &self.0.definition)
    }
}

/// A config type that is a program to run.
///
/// This supports a list of strings like `['/path/to/program', 'somearg']`
/// or a space separated string like `'/path/to/program somearg'`.
///
/// This expects the first value to be the path to the program to run.
/// Subsequent values are strings of arguments to pass to the program.
///
/// Typically you should use `ConfigRelativePath::resolve_program` on the path
/// to get the actual program.
///
/// **Note**: Any usage of this type in config needs to be listed in
/// the `NON_MERGEABLE_LISTS` check to prevent list merging
/// from multiple config files.
#[derive(Debug, Clone, PartialEq)]
pub struct PathAndArgs {
    pub path: ConfigRelativePath,
    pub args: Vec<String>,
}

impl<'de> serde::Deserialize<'de> for PathAndArgs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vsl = Value::<StringList>::deserialize(deserializer)?;
        let mut strings = vsl.val.0;
        if strings.is_empty() {
            return Err(D::Error::invalid_length(0, &"at least one element"));
        }
        let first = strings.remove(0);
        let crp = Value {
            val: first,
            definition: vsl.definition,
        };
        Ok(PathAndArgs {
            path: ConfigRelativePath(crp),
            args: strings,
        })
    }
}

impl PathAndArgs {
    /// Construct a `PathAndArgs` from a string. The string will be split on ascii whitespace,
    /// with the first item being treated as a `ConfigRelativePath` to the executable, and subsequent
    /// items as arguments.
    pub fn from_whitespace_separated_string(p: &Value<String>) -> PathAndArgs {
        let mut iter = p.val.split_ascii_whitespace().map(str::to_string);
        let val = iter.next().unwrap_or_default();
        let args = iter.collect();
        let crp = Value {
            val,
            definition: p.definition.clone(),
        };
        PathAndArgs {
            path: ConfigRelativePath(crp),
            args,
        }
    }
}

#[derive(Debug)]
pub enum ResolveTemplateError {
    UnexpectedVariable {
        variable: String,
        raw_template: String,
    },
    UnexpectedBracket {
        bracket_type: BracketType,
        raw_template: String,
    },
}

#[derive(Debug)]
pub enum BracketType {
    Opening,
    Closing,
}
