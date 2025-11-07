//! [`ConfigValue`] represents Cargo configuration values loaded from TOML.
//!
//! See [the module-level doc](crate::util::context)
//! for how configuration is parsed and deserialized.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::mem;

use anyhow::Context as _;
use anyhow::anyhow;
use anyhow::bail;

use super::ConfigKey;
use super::Definition;
use super::key;
use super::key::KeyOrIdx;
use crate::CargoResult;

use self::ConfigValue as CV;

/// List of which configuration lists cannot be merged.
///
/// Instead of merging,
/// the higher priority list should replaces the lower priority list.
///
/// ## What kind of config is non-mergeable
///
/// The rule of thumb is that if a config is a path of a program,
/// it should be added to this list.
const NON_MERGEABLE_LISTS: &[&str] = &[
    "credential-alias.*",
    "doc.browser",
    "host.runner",
    "registries.*.credential-provider",
    "registry.credential-provider",
    "target.*.runner",
];

/// Similar to [`toml::Value`] but includes the source location where it is defined.
#[derive(Eq, PartialEq, Clone)]
pub enum ConfigValue {
    Integer(i64, Definition),
    String(String, Definition),
    List(Vec<ConfigValue>, Definition),
    Table(HashMap<String, ConfigValue>, Definition),
    Boolean(bool, Definition),
}

impl fmt::Debug for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CV::Integer(i, def) => write!(f, "{} (from {})", i, def),
            CV::Boolean(b, def) => write!(f, "{} (from {})", b, def),
            CV::String(s, def) => write!(f, "{} (from {})", s, def),
            CV::List(list, def) => {
                write!(f, "[")?;
                for (i, item) in list.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item:?}")?;
                }
                write!(f, "] (from {})", def)
            }
            CV::Table(table, _) => write!(f, "{:?}", table),
        }
    }
}

impl ConfigValue {
    pub(super) fn from_toml(def: Definition, toml: toml::Value) -> CargoResult<ConfigValue> {
        let mut error_path = Vec::new();
        Self::from_toml_inner(def, toml, &mut error_path).with_context(|| {
            let mut it = error_path.iter().rev().peekable();
            let mut key_path = String::with_capacity(error_path.len() * 3);
            while let Some(k) = it.next() {
                match k {
                    KeyOrIdx::Key(s) => key_path.push_str(&key::escape_key_part(&s)),
                    KeyOrIdx::Idx(i) => key_path.push_str(&format!("[{i}]")),
                }
                if matches!(it.peek(), Some(KeyOrIdx::Key(_))) {
                    key_path.push('.');
                }
            }
            format!("failed to parse config at `{key_path}`")
        })
    }

    fn from_toml_inner(
        def: Definition,
        toml: toml::Value,
        path: &mut Vec<KeyOrIdx>,
    ) -> CargoResult<ConfigValue> {
        match toml {
            toml::Value::String(val) => Ok(CV::String(val, def)),
            toml::Value::Boolean(b) => Ok(CV::Boolean(b, def)),
            toml::Value::Integer(i) => Ok(CV::Integer(i, def)),
            toml::Value::Array(val) => Ok(CV::List(
                val.into_iter()
                    .enumerate()
                    .map(|(i, toml)| {
                        CV::from_toml_inner(def.clone(), toml, path)
                            .inspect_err(|_| path.push(KeyOrIdx::Idx(i)))
                    })
                    .collect::<CargoResult<_>>()?,
                def,
            )),
            toml::Value::Table(val) => Ok(CV::Table(
                val.into_iter()
                    .map(
                        |(key, value)| match CV::from_toml_inner(def.clone(), value, path) {
                            Ok(value) => Ok((key, value)),
                            Err(e) => {
                                path.push(KeyOrIdx::Key(key));
                                Err(e)
                            }
                        },
                    )
                    .collect::<CargoResult<_>>()?,
                def,
            )),
            v => bail!("unsupported TOML configuration type `{}`", v.type_str()),
        }
    }

    pub(super) fn into_toml(self) -> toml::Value {
        match self {
            CV::Boolean(s, _) => toml::Value::Boolean(s),
            CV::String(s, _) => toml::Value::String(s),
            CV::Integer(i, _) => toml::Value::Integer(i),
            CV::List(l, _) => toml::Value::Array(l.into_iter().map(|cv| cv.into_toml()).collect()),
            CV::Table(l, _) => {
                toml::Value::Table(l.into_iter().map(|(k, v)| (k, v.into_toml())).collect())
            }
        }
    }

    /// Merge the given value into self.
    ///
    /// If `force` is true, primitive (non-container) types will override existing values
    /// of equal priority. For arrays, incoming values of equal priority will be placed later.
    ///
    /// Container types (tables and arrays) are merged with existing values.
    ///
    /// Container and non-container types cannot be mixed.
    pub(super) fn merge(&mut self, from: ConfigValue, force: bool) -> CargoResult<()> {
        self.merge_helper(from, force, &mut ConfigKey::new())
    }

    fn merge_helper(
        &mut self,
        from: ConfigValue,
        force: bool,
        parts: &mut ConfigKey,
    ) -> CargoResult<()> {
        let is_higher_priority = from.definition().is_higher_priority(self.definition());
        match (self, from) {
            (&mut CV::List(ref mut old, _), CV::List(ref mut new, _)) => {
                if is_nonmergeable_list(&parts) {
                    // Use whichever list is higher priority.
                    if force || is_higher_priority {
                        mem::swap(new, old);
                    }
                } else {
                    // Merge the lists together.
                    if force {
                        old.append(new);
                    } else {
                        new.append(old);
                        mem::swap(new, old);
                    }
                }
                old.sort_by(|a, b| a.definition().cmp(b.definition()));
            }
            (&mut CV::Table(ref mut old, _), CV::Table(ref mut new, _)) => {
                for (key, value) in mem::take(new) {
                    match old.entry(key.clone()) {
                        Entry::Occupied(mut entry) => {
                            let new_def = value.definition().clone();
                            let entry = entry.get_mut();
                            parts.push(&key);
                            entry.merge_helper(value, force, parts).with_context(|| {
                                format!(
                                    "failed to merge key `{}` between \
                                     {} and {}",
                                    key,
                                    entry.definition(),
                                    new_def,
                                )
                            })?;
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(value);
                        }
                    };
                }
            }
            // Allow switching types except for tables or arrays.
            (expected @ &mut CV::List(_, _), found)
            | (expected @ &mut CV::Table(_, _), found)
            | (expected, found @ CV::List(_, _))
            | (expected, found @ CV::Table(_, _)) => {
                return Err(anyhow!(
                    "failed to merge config value from `{}` into `{}`: expected {}, but found {}",
                    found.definition(),
                    expected.definition(),
                    expected.desc(),
                    found.desc()
                ));
            }
            (old, mut new) => {
                if force || is_higher_priority {
                    mem::swap(old, &mut new);
                }
            }
        }

        Ok(())
    }

    pub fn i64(&self, key: &str) -> CargoResult<(i64, &Definition)> {
        match self {
            CV::Integer(i, def) => Ok((*i, def)),
            _ => self.expected("integer", key),
        }
    }

    pub fn string(&self, key: &str) -> CargoResult<(&str, &Definition)> {
        match self {
            CV::String(s, def) => Ok((s, def)),
            _ => self.expected("string", key),
        }
    }

    pub fn table(&self, key: &str) -> CargoResult<(&HashMap<String, ConfigValue>, &Definition)> {
        match self {
            CV::Table(table, def) => Ok((table, def)),
            _ => self.expected("table", key),
        }
    }

    pub fn table_mut(
        &mut self,
        key: &str,
    ) -> CargoResult<(&mut HashMap<String, ConfigValue>, &mut Definition)> {
        match self {
            CV::Table(table, def) => Ok((table, def)),
            _ => self.expected("table", key),
        }
    }

    pub fn is_table(&self) -> bool {
        matches!(self, CV::Table(..))
    }

    pub fn string_list(&self, key: &str) -> CargoResult<Vec<(String, Definition)>> {
        match self {
            CV::List(list, _) => list
                .iter()
                .map(|cv| match cv {
                    CV::String(s, def) => Ok((s.clone(), def.clone())),
                    _ => self.expected("string", key),
                })
                .collect::<CargoResult<_>>(),
            _ => self.expected("list", key),
        }
    }

    pub fn boolean(&self, key: &str) -> CargoResult<(bool, &Definition)> {
        match self {
            CV::Boolean(b, def) => Ok((*b, def)),
            _ => self.expected("bool", key),
        }
    }

    pub fn desc(&self) -> &'static str {
        match *self {
            CV::Table(..) => "table",
            CV::List(..) => "array",
            CV::String(..) => "string",
            CV::Boolean(..) => "boolean",
            CV::Integer(..) => "integer",
        }
    }

    pub fn definition(&self) -> &Definition {
        match self {
            CV::Boolean(_, def)
            | CV::Integer(_, def)
            | CV::String(_, def)
            | CV::List(_, def)
            | CV::Table(_, def) => def,
        }
    }

    pub(super) fn expected<T>(&self, wanted: &str, key: &str) -> CargoResult<T> {
        bail!(
            "expected a {}, but found a {} for `{}` in {}",
            wanted,
            self.desc(),
            key,
            self.definition()
        )
    }
}

pub(super) fn is_nonmergeable_list(key: &ConfigKey) -> bool {
    NON_MERGEABLE_LISTS.iter().any(|l| key.matches(l))
}
