use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use serde::de::IntoDeserializer as _;
use toml_edit::Item;

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
        let doc = toml_dotted_keys(arg)?;
        let doc: toml::Value = toml::Value::deserialize(doc.into_deserializer())
            .with_context(|| format!("failed to parse value from --config argument `{arg}`"))?;

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

        let table = CV::from_toml(Definition::Cli(None), doc)
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

fn toml_dotted_keys(arg: &str) -> Result<toml_edit::DocumentMut> {
    // We only want to allow "dotted key" (see https://toml.io/en/v1.0.0#keys)
    // expressions followed by a value that's not an "inline table"
    // (https://toml.io/en/v1.0.0#inline-table). Easiest way to check for that is to
    // parse the value as a toml_edit::DocumentMut, and check that the (single)
    // inner-most table is set via dotted keys.
    let doc: toml_edit::DocumentMut = arg.parse().with_context(|| {
        format!("failed to parse value from --config argument `{arg}` as a dotted key expression")
    })?;
    fn non_empty(d: Option<&toml_edit::RawString>) -> bool {
        d.map_or(false, |p| !p.as_str().unwrap_or_default().trim().is_empty())
    }
    fn non_empty_decor(d: &toml_edit::Decor) -> bool {
        non_empty(d.prefix()) || non_empty(d.suffix())
    }
    fn non_empty_key_decor(k: &toml_edit::Key) -> bool {
        non_empty_decor(k.leaf_decor()) || non_empty_decor(k.dotted_decor())
    }
    let ok = {
        let mut got_to_value = false;
        let mut table = doc.as_table();
        let mut is_root = true;
        while table.is_dotted() || is_root {
            is_root = false;
            if table.len() != 1 {
                break;
            }
            let (k, n) = table.iter().next().expect("len() == 1 above");
            match n {
                Item::Table(nt) => {
                    if table.key(k).map_or(false, non_empty_key_decor)
                        || non_empty_decor(nt.decor())
                    {
                        bail!(
                            "--config argument `{arg}` \
                                includes non-whitespace decoration"
                        )
                    }
                    table = nt;
                }
                Item::Value(v) if v.is_inline_table() => {
                    bail!(
                        "--config argument `{arg}` \
                        sets a value to an inline table, which is not accepted"
                    );
                }
                Item::Value(v) => {
                    if table
                        .key(k)
                        .map_or(false, |k| non_empty(k.leaf_decor().prefix()))
                        || non_empty_decor(v.decor())
                    {
                        bail!(
                            "--config argument `{arg}` \
                                includes non-whitespace decoration"
                        )
                    }
                    got_to_value = true;
                    break;
                }
                Item::ArrayOfTables(_) => {
                    bail!(
                        "--config argument `{arg}` \
                        sets a value to an array of tables, which is not accepted"
                    );
                }

                Item::None => {
                    bail!("--config argument `{arg}` doesn't provide a value")
                }
            }
        }
        got_to_value
    };
    if !ok {
        bail!(
            "--config argument `{arg}` was not a TOML dotted key expression (such as `build.jobs = 2`)"
        );
    }
    Ok(doc)
}
