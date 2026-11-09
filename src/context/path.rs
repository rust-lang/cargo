use std::path::PathBuf;

use regex::Regex;

use super::{ConfigRelativePath, GlobalContext};

/// Resolves configuration-relative paths using the global context.
pub trait ConfigRelativePathResolveExt {
    /// Resolves this configuration-relative path to an absolute path.
    ///
    /// This will always return an absolute path where it's relative to the
    /// location for configuration for this value.
    fn resolve_path(&self, gctx: &GlobalContext) -> PathBuf;

    /// Same as [`Self::resolve_path`] but will make string replacements
    /// before resolving the path.
    ///
    /// `replacements` should be an [`IntoIterator`] of tuples with the "from" and "to" for the
    /// string replacement.
    fn resolve_templated_path(
        &self,
        gctx: &GlobalContext,
        replacements: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
    ) -> Result<PathBuf, ResolveTemplateError>;

    /// Resolves this configuration-relative path to either an absolute path or
    /// something appropriate to execute from `PATH`.
    ///
    /// Values which don't look like a filesystem path (don't contain `/` or
    /// `\`) will be returned as-is, and everything else will fall through to an
    /// absolute path.
    fn resolve_program(&self, gctx: &GlobalContext) -> PathBuf;
}

impl ConfigRelativePathResolveExt for ConfigRelativePath {
    fn resolve_path(&self, gctx: &GlobalContext) -> PathBuf {
        let value = self.value();
        value.definition.root(gctx.cwd()).join(&value.val)
    }

    fn resolve_templated_path(
        &self,
        gctx: &GlobalContext,
        replacements: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
    ) -> Result<PathBuf, ResolveTemplateError> {
        let path = self.value();
        let mut value = path.val.clone();

        for (from, to) in replacements {
            value = value.replace(from.as_ref(), to.as_ref());
        }

        // Check for expected variables
        let re = Regex::new(r"\{(.*)\}").unwrap();
        if let Some(caps) = re.captures(&value) {
            return Err(ResolveTemplateError::UnexpectedVariable {
                variable: caps[1].to_string(),
                raw_template: path.val.clone(),
            });
        };

        if value.contains("{") {
            return Err(ResolveTemplateError::UnexpectedBracket {
                bracket_type: BracketType::Opening,
                raw_template: path.val.clone(),
            });
        }

        if value.contains("}") {
            return Err(ResolveTemplateError::UnexpectedBracket {
                bracket_type: BracketType::Closing,
                raw_template: path.val.clone(),
            });
        }

        Ok(path.definition.root(gctx.cwd()).join(&value))
    }

    fn resolve_program(&self, gctx: &GlobalContext) -> PathBuf {
        let value = self.value();
        gctx.string_to_path(&value.val, &value.definition)
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
