use std::path::PathBuf;

use regex::Regex;

use super::{ConfigRelativePath, GlobalContext};

/// Resolves configuration-relative paths using the global context.
pub trait ConfigRelativePathResolveExt {
    /// Applies string replacements before resolving the path.
    ///
    /// `replacements` should be an [`IntoIterator`] of tuples with the "from" and "to" for the
    /// string replacement.
    fn resolve_templated_path(
        &self,
        gctx: &GlobalContext,
        replacements: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
    ) -> Result<PathBuf, ResolveTemplateError>;
}

impl ConfigRelativePathResolveExt for ConfigRelativePath {
    fn resolve_templated_path(
        &self,
        gctx: &GlobalContext,
        replacements: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
    ) -> Result<PathBuf, ResolveTemplateError> {
        let path = self.as_value();
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
