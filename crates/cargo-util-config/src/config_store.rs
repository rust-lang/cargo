use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use super::{ConfigKey, ConfigValue, Definition, DefinitionKind, MergePriority};
use cargo_util::collections::{BuildHasher, HashMap};

use ConfigValue as CV;

#[derive(Default, Debug)]
pub struct ConfigStore {
    /// A collection of configuration options
    values: HashMap<String, ConfigValue>,
    def: Definition,
}

impl ConfigStore {
    pub const fn new() -> Self {
        Self {
            values: HashMap::with_hasher(BuildHasher),
            def: Definition::BuiltIn,
        }
    }

    pub fn from_values(values: HashMap<String, ConfigValue>) -> Self {
        Self {
            values,
            def: Definition::BuiltIn,
        }
    }

    pub fn from_table_value(value: ConfigValue) -> Option<Self> {
        match value {
            CV::Table(values, def) => Some(Self { values, def }),
            _ => None,
        }
    }

    /// Merges config `layers` given from lowest to highest precedence.
    ///
    /// A value with a higher-priority [`Definition`] wins regardless of its layer's position.
    pub fn from_layers(layers: impl IntoIterator<Item = Self>) -> Result<Self> {
        let layers: Vec<_> = layers.into_iter().collect();
        let mut merged = Self::default();
        // Merge from highest precedence, so ties keep the value already merged.
        for mut layer in layers.into_iter().rev() {
            layer.values.remove("include");
            merged.merge(layer, MergePriority::Definition)?;
        }
        Ok(merged)
    }

    /// Loads configuration from a path without processing includes.
    pub fn read_layer(path: &Path, definition_kind: DefinitionKind) -> Result<Self> {
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
        Ok(Self::from_table_value(value).unwrap())
    }

    /// Parses a single inline `KEY=VALUE` argument into a configuration layer.
    pub fn parse_cli_key_value(arg: &str) -> Result<Self> {
        let doc = arg.parse::<toml::Table>().with_context(|| {
            format!(
                "failed to parse value from --config argument `{arg}` as a dotted key expression"
            )
        })?;
        validate_cli_key_value(arg)?;

        if doc
            .get("registry")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("token"))
            .is_some()
        {
            bail!("registry.token cannot be set through --config for security reasons");
        } else if let Some((k, _)) = doc
            .get("registries")
            .and_then(|v| v.as_table())
            .and_then(|t| t.iter().find(|(_, v)| v.get("token").is_some()))
        {
            bail!(
                "registries.{}.token cannot be set through --config for security reasons",
                k
            );
        }

        let table = CV::from_toml(Definition::Cli(None), toml::Value::Table(doc))
            .with_context(|| format!("failed to convert --config argument `{arg}`"))?;
        Ok(Self::from_table_value(table).unwrap())
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
    /// Tables merge recursively. [`MergePriority`] controls value replacement and list ordering.
    ///
    /// Container and non-container types cannot be mixed.
    pub fn merge(&mut self, from: Self, priority: MergePriority) -> Result<()> {
        let mut root_cv = std::mem::take(self).into_value();
        let from = from.into_value();
        root_cv.merge(from, priority)?;
        *self = Self::from_table_value(root_cv).unwrap();

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
    pub fn get_cv(&self, key: &ConfigKey) -> Result<Option<ConfigValue>> {
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

/// Ensure the correct format of `--config key = value`
///
/// We only want to allow "dotted key" (see <https://toml.io/en/v1.0.0#keys>)
/// expressions followed by a value that's not an [inline table](https://toml.io/en/v1.0.0#inline-table).
fn validate_cli_key_value(arg: &str) -> Result<()> {
    use toml_parser::parser::EventKind;

    let invalid_expression = || {
        anyhow::anyhow!(
            "--config argument `{arg}` was not a TOML dotted key expression (such as `build.jobs = 2`)"
        )
    };
    let decoration =
        || anyhow::anyhow!("--config argument `{arg}` includes non-whitespace decoration");

    let tokens = toml_parser::Source::new(arg).lex().into_vec();
    let mut events = Vec::new();
    toml_parser::parser::parse_document(&tokens, &mut events, &mut ());
    let mut events = events
        .into_iter()
        .map(|token| token.kind())
        .filter(|kind| !matches!(*kind, EventKind::Whitespace | EventKind::Newline))
        .peekable();

    if events.next_if_eq(&EventKind::Comment).is_some() {
        return Err(decoration());
    }

    if !events
        .next_if(|token| matches!(token, EventKind::SimpleKey))
        .is_some()
    {
        return Err(invalid_expression());
    }
    while events
        .next_if(|token| matches!(token, EventKind::KeySep | EventKind::SimpleKey))
        .is_some()
    {}

    if !events.next_if_eq(&EventKind::KeyValSep).is_some() {
        return Err(invalid_expression());
    }

    let mut array_depth = None::<usize>;
    while let Some(token) = events.next() {
        match token {
            EventKind::StdTableOpen | EventKind::ArrayTableOpen => {
                return Err(invalid_expression());
            }
            EventKind::SimpleKey if array_depth.unwrap_or(0) == 0 => {
                // Started a new key-value pair
                return Err(invalid_expression());
            }
            EventKind::InlineTableOpen if !array_depth.is_some() => {
                bail!(
                    "--config argument `{arg}` sets a value to an inline table, which is not accepted"
                );
            }
            EventKind::ArrayOpen => *array_depth.get_or_insert_default() += 1,
            EventKind::ArrayClose => *array_depth.get_or_insert_default() -= 1,
            EventKind::Comment => return Err(decoration()),
            _ => {}
        }
    }

    Ok(())
}
