use std::fmt;

use crate::util::ConfigValue;
use crate::util::context::ConfigKey;
use crate::util::context::Definition;
use crate::util::context::key::ArrayItemKeyPath;

/// Internal error for serde errors.
#[derive(Debug)]
pub struct ConfigError {
    error: anyhow::Error,
    definition: Option<Definition>,
}

impl ConfigError {
    pub(super) fn new(message: String, definition: Definition) -> ConfigError {
        ConfigError {
            error: anyhow::Error::msg(message),
            definition: Some(definition),
        }
    }

    pub(super) fn expected(key: &ConfigKey, expected: &str, found: &ConfigValue) -> ConfigError {
        ConfigError {
            error: anyhow::anyhow!(
                "`{}` expected {}, but found a {}",
                key,
                expected,
                found.desc()
            ),
            definition: Some(found.definition().clone()),
        }
    }

    pub(super) fn is_missing_field(&self) -> bool {
        self.error.downcast_ref::<MissingFieldError>().is_some()
    }

    pub(super) fn missing(key: &ConfigKey) -> ConfigError {
        ConfigError {
            error: anyhow::anyhow!("missing config key `{}`", key),
            definition: None,
        }
    }

    pub(super) fn with_key_context(
        self,
        key: &ConfigKey,
        definition: Option<Definition>,
    ) -> ConfigError {
        ConfigError {
            error: anyhow::Error::from(self)
                .context(format!("could not load config key `{}`", key)),
            definition: definition,
        }
    }

    pub(super) fn with_array_item_key_context(
        self,
        key: &ArrayItemKeyPath,
        definition: Option<Definition>,
    ) -> ConfigError {
        ConfigError {
            error: anyhow::Error::from(self).context(format!("failed to parse config at `{key}`")),
            definition,
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(definition) = &self.definition {
            write!(f, "error in {}: {}", definition, self.error)
        } else {
            self.error.fmt(f)
        }
    }
}

#[derive(Debug)]
struct MissingFieldError(String);

impl fmt::Display for MissingFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "missing field `{}`", self.0)
    }
}

impl std::error::Error for MissingFieldError {}

impl serde::de::Error for ConfigError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        ConfigError {
            error: anyhow::Error::msg(msg.to_string()),
            definition: None,
        }
    }

    fn missing_field(field: &'static str) -> Self {
        ConfigError {
            error: anyhow::Error::new(MissingFieldError(field.to_string())),
            definition: None,
        }
    }
}

impl From<anyhow::Error> for ConfigError {
    fn from(error: anyhow::Error) -> Self {
        ConfigError {
            error,
            definition: None,
        }
    }
}
