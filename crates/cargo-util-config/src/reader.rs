use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;

use anyhow::{Result, anyhow, bail};
use cargo_util_terminal::Shell;

use super::de::Deserializer;
use super::{ConfigError, OptValue, Value};
use crate::{ConfigKey, ConfigStore, ConfigValue, Definition, Env, is_nonmergeable_list};
use cargo_util::collections::HashMap;

use ConfigValue as CV;

/// Reads configuration values from the loaded store and the environment.
#[derive(Clone, Copy)]
pub struct ConfigReader<'a> {
    pub(crate) store: &'a ConfigStore,
    pub(crate) shell: &'a Mutex<Shell>,
    pub(crate) env: &'a Env,
    pub(crate) unstable_advanced_env: bool,
}

/// Helper macro for creating typed access methods.
macro_rules! get_value_typed {
    ($name:ident, $ty:ty, $variant:ident, $expected:expr) => {
        /// Low-level private method for getting a config value as an [`OptValue`].
        pub(crate) fn $name(&self, key: &ConfigKey) -> Result<OptValue<$ty>, ConfigError> {
            let cv = self.store.get_cv(key)?;
            let env = self.get_config_env::<$ty>(key)?;
            match (cv, env) {
                (Some(CV::$variant(val, definition)), Some(env)) => {
                    if definition.is_higher_priority(&env.definition) {
                        Ok(Some(Value { val, definition }))
                    } else {
                        Ok(Some(env))
                    }
                }
                (Some(CV::$variant(val, definition)), None) => Ok(Some(Value { val, definition })),
                (Some(cv), _) => Err(ConfigError::expected(key, $expected, &cv)),
                (None, Some(env)) => Ok(Some(env)),
                (None, None) => Ok(None),
            }
        }
    };
}

impl<'a> ConfigReader<'a> {
    pub fn new(shell: &'a Mutex<Shell>) -> Self {
        static STORE: ConfigStore = ConfigStore::new();
        static ENV: Env = Env::new();
        Self {
            store: &STORE,
            shell,
            env: &ENV,
            unstable_advanced_env: false,
        }
    }

    #[must_use]
    pub fn config(mut self, store: &'a ConfigStore) -> Self {
        self.store = store;
        self
    }

    #[must_use]
    pub fn env(mut self, env: &'a Env) -> Self {
        self.env = env;
        self
    }

    #[must_use]
    pub fn unstable_advanced_env(mut self, yes: bool) -> Self {
        self.unstable_advanced_env = yes;
        self
    }

    pub fn get<'de, T: serde::Deserialize<'de>>(self, key: ConfigKey) -> Result<T> {
        let de = Deserializer {
            reader: self,
            key,
            env_prefix_ok: true,
        };
        Ok(T::deserialize(de)?)
    }

    pub fn get_cv_with_env(&self, key: &ConfigKey) -> Result<Option<CV>> {
        // Determine if value comes from env, cli, or file, and merge env if
        // possible.
        let cv = self.store.get_cv(key)?;
        if key.is_root() {
            // Root table can't have env value.
            return Ok(cv);
        }
        let env = self.env.get_str(key.as_env_key());
        let env_def = Definition::Environment(key.as_env_key().to_string());
        let use_env = match (&cv, env) {
            // Lists are always merged.
            (Some(CV::List(..)), Some(_)) => true,
            (Some(cv), Some(_)) => env_def.is_higher_priority(cv.definition()),
            (None, Some(_)) => true,
            _ => false,
        };

        if !use_env {
            return Ok(cv);
        }

        // Future note: If you ever need to deserialize a non-self describing
        // map type, this should implement a starts_with check (similar to how
        // ConfigMapAccess does).
        let env = env.unwrap();
        if env == "true" {
            Ok(Some(CV::Boolean(true, env_def)))
        } else if env == "false" {
            Ok(Some(CV::Boolean(false, env_def)))
        } else if let Ok(i) = env.parse::<i64>() {
            Ok(Some(CV::Integer(i, env_def)))
        } else if self.unstable_advanced_env && env.starts_with('[') && env.ends_with(']') {
            match cv {
                Some(CV::List(mut cv_list, cv_def)) => {
                    // Merge with config file.
                    self.get_env_list(key, &mut cv_list)?;
                    Ok(Some(CV::List(cv_list, cv_def)))
                }
                Some(cv) => {
                    // This can't assume StringList.
                    // Return an error, which is the behavior of merging
                    // multiple config.toml files with the same scenario.
                    bail!(
                        "unable to merge array env for config `{}`\n\
                        file: {:?}\n\
                        env: {}",
                        key,
                        cv,
                        env
                    );
                }
                None => {
                    let mut cv_list = Vec::new();
                    self.get_env_list(key, &mut cv_list)?;
                    Ok(Some(CV::List(cv_list, env_def)))
                }
            }
        } else {
            // Try to merge if possible.
            match cv {
                Some(CV::List(mut cv_list, cv_def)) => {
                    // Merge with config file.
                    self.get_env_list(key, &mut cv_list)?;
                    Ok(Some(CV::List(cv_list, cv_def)))
                }
                _ => {
                    // Note: CV::Table merging is not implemented, as env
                    // vars do not support table values. In the future, we
                    // could check for `{}`, and interpret it as TOML if
                    // that seems useful.
                    Ok(Some(CV::String(env.to_string(), env_def)))
                }
            }
        }
    }

    /// Low-level method for getting a config value as an `OptValue<HashMap<String, CV>>`.
    ///
    /// NOTE: This does not read from env. The caller is responsible for that.
    pub fn get_table(&self, key: &ConfigKey) -> Result<OptValue<HashMap<String, CV>>> {
        match self.store.get_cv(key)? {
            Some(CV::Table(val, definition)) => Ok(Some(Value { val, definition })),
            Some(val) => expected("table", key, &val),
            None => Ok(None),
        }
    }

    get_value_typed! {get_integer, i64, Integer, "an integer"}
    get_value_typed! {get_bool, bool, Boolean, "true/false"}
    get_value_typed! {get_string_priv, String, String, "a string"}

    /// Check if the configuration contains a given [`ConfigKey`].
    ///
    /// See `ConfigMapAccess` for a description of `env_prefix_ok`.
    pub(crate) fn has_key(&self, key: &ConfigKey, env_prefix_ok: bool) -> Result<bool> {
        if self.env.contains_key(key.as_env_key()) {
            return Ok(true);
        }
        if env_prefix_ok {
            let env_prefix = format!("{}_", key.as_env_key());
            if self.env.keys_str().any(|k| k.starts_with(&env_prefix)) {
                return Ok(true);
            }
        }
        if self.store.get_cv(key)?.is_some() {
            return Ok(true);
        }
        self.check_environment_key_case_mismatch(key);

        Ok(false)
    }

    /// Internal method for getting an environment variable as a list.
    /// If the key is a non-mergeable list and a value is found in the environment, existing values are cleared.
    pub(crate) fn get_env_list(
        &self,
        key: &ConfigKey,
        output: &mut Vec<ConfigValue>,
    ) -> Result<()> {
        let Some(env_val) = self.env.get_str(key.as_env_key()) else {
            self.check_environment_key_case_mismatch(key);
            return Ok(());
        };

        let env_def = Definition::Environment(key.as_env_key().to_string());

        if is_nonmergeable_list(&key) {
            assert!(
                output
                    .windows(2)
                    .all(|cvs| cvs[0].definition() == cvs[1].definition()),
                "non-mergeable list must have only one definition: {output:?}",
            );

            // Keep existing config if higher priority than env (e.g., --config CLI),
            // otherwise clear for env
            if output
                .first()
                .map(|o| o.definition() > &env_def)
                .unwrap_or_default()
            {
                return Ok(());
            } else {
                output.clear();
            }
        }

        if self.unstable_advanced_env && env_val.starts_with('[') && env_val.ends_with(']') {
            // Parse an environment string as a TOML array.
            let toml_v = env_val.parse::<toml::Value>().map_err(|e| {
                ConfigError::new(format!("could not parse TOML list: {}", e), env_def.clone())
            })?;
            let values = toml_v.as_array().expect("env var was not array");
            for value in values {
                // Until we figure out how to deal with it through `-Zadvanced-env`,
                // complex array types are unsupported.
                let s = value.as_str().ok_or_else(|| {
                    ConfigError::new(
                        format!("expected string, found {}", value.type_str()),
                        env_def.clone(),
                    )
                })?;
                output.push(CV::String(s.to_string(), env_def.clone()))
            }
        } else {
            output.extend(
                env_val
                    .split_whitespace()
                    .map(|s| CV::String(s.to_string(), env_def.clone())),
            );
        }
        output.sort_by(|a, b| a.definition().cmp(b.definition()));
        Ok(())
    }

    pub(crate) fn warn(&self, message: String) -> Result<()> {
        self.shell.lock().unwrap().warn(message)
    }

    fn get_config_env<T>(&self, key: &ConfigKey) -> Result<OptValue<T>, ConfigError>
    where
        T: FromStr,
        <T as FromStr>::Err: fmt::Display,
    {
        match self.env.get_str(key.as_env_key()) {
            Some(value) => {
                let definition = Definition::Environment(key.as_env_key().to_string());
                Ok(Some(Value {
                    val: value
                        .parse()
                        .map_err(|e| ConfigError::new(format!("{}", e), definition.clone()))?,
                    definition,
                }))
            }
            None => {
                self.check_environment_key_case_mismatch(key);
                Ok(None)
            }
        }
    }

    fn check_environment_key_case_mismatch(&self, key: &ConfigKey) {
        if let Some(env_key) = self.env.get_normalized(key.as_env_key()) {
            let _ = self.warn(format!(
                "environment variables are expected to use uppercase letters and underscores, \
                the variable `{}` will be ignored and have no effect",
                env_key
            ));
        }
    }
}

/// Generate an error when the given value is the wrong type.
pub fn expected<T>(ty: &str, key: &ConfigKey, val: &CV) -> Result<T> {
    val.expected(ty, &key.to_string())
        .map_err(|e| anyhow!("invalid configuration for key `{}`\n{}", key, e))
}
