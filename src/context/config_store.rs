use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};

use crate::context::{ConfigKey, ConfigValue, Definition, DefinitionKind};
use crate::util::data_structures::HashMap;
use crate::util::errors::CargoResult;

use ConfigValue as CV;

#[derive(Default, Debug)]
pub struct ConfigStore {
    /// A collection of configuration options
    values: HashMap<String, ConfigValue>,
    def: Definition,
}

impl ConfigStore {
    pub fn with_values(values: HashMap<String, ConfigValue>) -> Self {
        Self {
            values,
            def: Definition::BuiltIn,
        }
    }

    pub fn with_table_value(value: ConfigValue) -> Option<Self> {
        match value {
            CV::Table(values, def) => Some(Self { values, def }),
            _ => None,
        }
    }

    /// Loads configuration from a path without processing includes.
    pub fn read_layer(path: &Path, definition_kind: DefinitionKind) -> CargoResult<Self> {
        tracing::debug!(?path, ?definition_kind, "load config from file");

        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read configuration file `{}`", path.display()))?;
        let toml = contents.parse::<toml::Table>().with_context(|| {
            format!("could not parse TOML configuration in `{}`", path.display())
        })?;
        let def = match definition_kind {
            DefinitionKind::Cli => Definition::Cli(Some(path.into())),
            DefinitionKind::Path => Definition::Path(path.into()),
            DefinitionKind::BuiltIn => Definition::BuiltIn,
            DefinitionKind::Environment => {
                unimplemented!("Environment defiinitions are not supported")
            }
        };
        let value = CV::from_toml(def, toml::Value::Table(toml)).with_context(|| {
            format!(
                "failed to load TOML configuration from `{}`",
                path.display()
            )
        })?;
        Ok(Self::with_table_value(value).unwrap())
    }

    pub fn into_value(self) -> ConfigValue {
        let Self { values, def } = self;
        CV::Table(values, def)
    }

    pub fn definition(&self) -> &Definition {
        &self.def
    }

    /// Merge the given values into self.
    ///
    /// If `force` is true, primitive (non-container) types will override existing values
    /// of equal priority. For arrays, incoming values of equal priority will be placed later.
    ///
    /// Container types (tables and arrays) are merged with existing values.
    ///
    /// Container and non-container types cannot be mixed.
    pub fn merge(&mut self, from: Self, force: bool) -> CargoResult<()> {
        let mut root_cv = std::mem::take(self).into_value();
        let from = from.into_value();
        root_cv.merge(from, force)?;
        *self = Self::with_table_value(root_cv).unwrap();

        Ok(())
    }

    pub fn values(&self) -> &HashMap<String, ConfigValue> {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut HashMap<String, ConfigValue> {
        &mut self.values
    }

    /// Get a configuration value by key.
    ///
    /// This does NOT look at environment variables. See `get_cv_with_env` for
    /// a variant that supports environment variables.
    pub fn get_cv(&self, key: &ConfigKey) -> CargoResult<Option<ConfigValue>> {
        let vals = self.values();
        tracing::trace!("get cv {:?}", key);
        if key.is_root() {
            // Returning the entire root table (for example `cargo config get`
            // with no key). The definition here shouldn't matter.
            return Ok(Some(CV::Table(
                vals.clone(),
                Definition::Path(PathBuf::new()),
            )));
        }
        let mut parts = key.parts().enumerate();
        let Some(mut val) = vals.get(parts.next().unwrap().1) else {
            return Ok(None);
        };
        for (i, part) in parts {
            match val {
                CV::Table(map, _) => {
                    val = match map.get(part) {
                        Some(val) => val,
                        None => return Ok(None),
                    }
                }
                CV::Integer(_, def)
                | CV::String(_, def)
                | CV::List(_, def)
                | CV::Boolean(_, def) => {
                    let key_so_far = ConfigKey::from_parts(key.parts().take(i));
                    bail!(
                        "expected table for configuration key `{}`, \
                         but found {} in {}",
                        key_so_far,
                        val.desc(),
                        def
                    )
                }
            }
        }
        Ok(Some(val.clone()))
    }
}
