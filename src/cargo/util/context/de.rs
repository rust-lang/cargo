//! Deserialization for converting [`ConfigValue`] instances to target types.
//!
//! The [`Deserializer`] type is the main driver of deserialization.
//! The workflow is roughly:
//!
//! 1. [`GlobalContext::get<T>()`] creates [`Deserializer`] and calls `T::deserialize()`
//! 2. Then call type-specific deserialize methods as in normal serde deserialization.
//!     - For primitives, `deserialize_*` methods look up [`ConfigValue`] instances
//!       in [`GlobalContext`] and convert.
//!     - Structs and maps are handled by [`ConfigMapAccess`].
//!     - Sequences are handled by [`ConfigSeqAccess`],
//!       which later uses [`ArrayItemDeserializer`] for each array item.
//!     - [`Value<T>`] is delegated to [`ValueDeserializer`] in `deserialize_struct`.
//!
//! The purpose of this workflow is to:
//!
//! - Retrieve the correct config value based on source location precedence
//! - Provide richer error context showing where a config is defined
//! - Provide a richer internal API to map to concrete config types
//!   without touching underlying [`ConfigValue`] directly
//!
//! [`ConfigValue`]: CV

use crate::util::context::key::ArrayItemKeyPath;
use crate::util::context::value;
use crate::util::context::{ConfigError, ConfigKey, GlobalContext};
use crate::util::context::{ConfigValue as CV, Definition, Value};
use serde::{de, de::IntoDeserializer};
use std::collections::HashSet;
use std::vec;

/// Serde deserializer used to convert config values to a target type using
/// [`GlobalContext::get`].
#[derive(Clone)]
pub(super) struct Deserializer<'gctx> {
    pub(super) gctx: &'gctx GlobalContext,
    /// The current key being deserialized.
    pub(super) key: ConfigKey,
    /// Whether or not this key part is allowed to be an inner table. For
    /// example, `profile.dev.build-override` needs to check if
    /// `CARGO_PROFILE_DEV_BUILD_OVERRIDE_` prefixes exist. But
    /// `CARGO_BUILD_TARGET` should not check for prefixes because it would
    /// collide with `CARGO_BUILD_TARGET_DIR`. See `ConfigMapAccess` for
    /// details.
    pub(super) env_prefix_ok: bool,
}

macro_rules! deserialize_method {
    ($method:ident, $visit:ident, $getter:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: de::Visitor<'de>,
        {
            let v = self
                .gctx
                .$getter(&self.key)?
                .ok_or_else(|| ConfigError::missing(&self.key))?;
            let Value { val, definition } = v;
            let res: Result<V::Value, ConfigError> = visitor.$visit(val);
            res.map_err(|e| e.with_key_context(&self.key, Some(definition)))
        }
    };
}

impl<'de, 'gctx> de::Deserializer<'de> for Deserializer<'gctx> {
    type Error = ConfigError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let cv = self.gctx.get_cv_with_env(&self.key)?;
        if let Some(cv) = cv {
            let res: (Result<V::Value, ConfigError>, Definition) = match cv {
                CV::Integer(i, def) => (visitor.visit_i64(i), def),
                CV::String(s, def) => (visitor.visit_string(s), def),
                CV::List(_, def) => (visitor.visit_seq(ConfigSeqAccess::new(self.clone())?), def),
                CV::Table(_, def) => (
                    visitor.visit_map(ConfigMapAccess::new_map(self.clone())?),
                    def,
                ),
                CV::Boolean(b, def) => (visitor.visit_bool(b), def),
            };
            let (res, def) = res;
            return res.map_err(|e| e.with_key_context(&self.key, Some(def)));
        }

        // The effect here is the same as in `deserialize_option`.
        if self.gctx.has_key(&self.key, self.env_prefix_ok)? {
            return visitor.visit_some(self);
        }

        Err(ConfigError::missing(&self.key))
    }

    deserialize_method!(deserialize_bool, visit_bool, get_bool);
    deserialize_method!(deserialize_i8, visit_i64, get_integer);
    deserialize_method!(deserialize_i16, visit_i64, get_integer);
    deserialize_method!(deserialize_i32, visit_i64, get_integer);
    deserialize_method!(deserialize_i64, visit_i64, get_integer);
    deserialize_method!(deserialize_u8, visit_i64, get_integer);
    deserialize_method!(deserialize_u16, visit_i64, get_integer);
    deserialize_method!(deserialize_u32, visit_i64, get_integer);
    deserialize_method!(deserialize_u64, visit_i64, get_integer);
    deserialize_method!(deserialize_string, visit_string, get_string_priv);

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if self.gctx.has_key(&self.key, self.env_prefix_ok)? {
            visitor.visit_some(self)
        } else {
            // Treat missing values as `None`.
            visitor.visit_none()
        }
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        // Match on the magical struct name/field names that are passed in to
        // detect when we're deserializing `Value<T>`.
        //
        // See more comments in `value.rs` for the protocol used here.
        if name == value::NAME && fields == value::FIELDS {
            let source = ValueSource::with_deserializer(self)?;
            return visitor.visit_map(ValueDeserializer::new(source));
        }
        visitor.visit_map(ConfigMapAccess::new_struct(self, fields)?)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_map(ConfigMapAccess::new_map(self)?)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(ConfigSeqAccess::new(self)?)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(ConfigSeqAccess::new(self)?)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(ConfigSeqAccess::new(self)?)
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if name == "StringList" {
            let mut res = Vec::new();

            match self.gctx.get_cv(&self.key)? {
                Some(CV::List(val, _def)) => res.extend(val),
                Some(CV::String(val, def)) => {
                    let split_vs = val
                        .split_whitespace()
                        .map(|s| CV::String(s.to_string(), def.clone()));
                    res.extend(split_vs);
                }
                Some(val) => {
                    self.gctx
                        .expected("string or array of strings", &self.key, &val)?;
                }
                None => {}
            }

            self.gctx.get_env_list(&self.key, &mut res)?;

            let vals: Vec<String> = res
                .into_iter()
                .map(|val| match val {
                    CV::String(s, _defintion) => Ok(s),
                    other => Err(ConfigError::expected(&self.key, "string", &other)),
                })
                .collect::<Result<_, _>>()?;
            visitor.visit_newtype_struct(vals.into_deserializer())
        } else {
            visitor.visit_newtype_struct(self)
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let value = self
            .gctx
            .get_string_priv(&self.key)?
            .ok_or_else(|| ConfigError::missing(&self.key))?;

        let Value { val, definition } = value;
        visitor
            .visit_enum(val.into_deserializer())
            .map_err(|e: ConfigError| e.with_key_context(&self.key, Some(definition)))
    }

    // These aren't really supported, yet.
    serde::forward_to_deserialize_any! {
        f32 f64 char str bytes
        byte_buf unit unit_struct
        identifier ignored_any
    }
}

struct ConfigMapAccess<'gctx> {
    de: Deserializer<'gctx>,
    /// The fields that this map should deserialize.
    fields: Vec<KeyKind>,
    /// Current field being deserialized.
    field_index: usize,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum KeyKind {
    Normal(String),
    CaseSensitive(String),
}

impl<'gctx> ConfigMapAccess<'gctx> {
    fn new_map(de: Deserializer<'gctx>) -> Result<ConfigMapAccess<'gctx>, ConfigError> {
        let mut fields = Vec::new();
        if let Some(mut v) = de.gctx.get_table(&de.key)? {
            // `v: Value<HashMap<String, CV>>`
            for (key, _value) in v.val.drain() {
                fields.push(KeyKind::CaseSensitive(key));
            }
        }
        if de.gctx.cli_unstable().advanced_env {
            // `CARGO_PROFILE_DEV_PACKAGE_`
            let env_prefix = format!("{}_", de.key.as_env_key());
            for env_key in de.gctx.env_keys() {
                // `CARGO_PROFILE_DEV_PACKAGE_bar_OPT_LEVEL = 3`
                if let Some(rest) = env_key.strip_prefix(&env_prefix) {
                    // `rest = bar_OPT_LEVEL`
                    let part = rest.splitn(2, '_').next().unwrap();
                    // `part = "bar"`
                    fields.push(KeyKind::CaseSensitive(part.to_string()));
                }
            }
        }
        Ok(ConfigMapAccess {
            de,
            fields,
            field_index: 0,
        })
    }

    fn new_struct(
        de: Deserializer<'gctx>,
        given_fields: &'static [&'static str],
    ) -> Result<ConfigMapAccess<'gctx>, ConfigError> {
        let table = de.gctx.get_table(&de.key)?;

        // Assume that if we're deserializing a struct it exhaustively lists all
        // possible fields on this key that we're *supposed* to use, so take
        // this opportunity to warn about any keys that aren't recognized as
        // fields and warn about them.
        if let Some(v) = table.as_ref() {
            let unused_keys = v
                .val
                .iter()
                .filter(|(k, _v)| !given_fields.iter().any(|gk| gk == k));
            for (unused_key, unused_value) in unused_keys {
                de.gctx.shell().warn(format!(
                    "unused config key `{}.{}` in `{}`",
                    de.key,
                    unused_key,
                    unused_value.definition()
                ))?;
            }
        }

        let mut fields = HashSet::new();

        // If the caller is interested in a field which we can provide from
        // the environment, get it from there.
        for field in given_fields {
            let mut field_key = de.key.clone();
            field_key.push(field);
            for env_key in de.gctx.env_keys() {
                let Some(nested_field) = env_key.strip_prefix(field_key.as_env_key()) else {
                    continue;
                };
                // This distinguishes fields that share the same prefix.
                // For example, when env_key is UNSTABLE_GITOXIDE_FETCH
                // and field_key is UNSTABLE_GIT, the field shouldn't be
                // added because `unstable.gitoxide.fetch` doesn't
                // belong to `unstable.git` struct.
                if nested_field.is_empty() || nested_field.starts_with('_') {
                    fields.insert(KeyKind::Normal(field.to_string()));
                }
            }
        }

        // Add everything from the config table we're interested in that we
        // haven't already provided via an environment variable
        if let Some(v) = table {
            for key in v.val.keys() {
                fields.insert(KeyKind::Normal(key.clone()));
            }
        }

        Ok(ConfigMapAccess {
            de,
            fields: fields.into_iter().collect(),
            field_index: 0,
        })
    }
}

impl<'de, 'gctx> de::MapAccess<'de> for ConfigMapAccess<'gctx> {
    type Error = ConfigError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        if self.field_index >= self.fields.len() {
            return Ok(None);
        }
        let field = match &self.fields[self.field_index] {
            KeyKind::Normal(s) | KeyKind::CaseSensitive(s) => s.as_str(),
        };
        seed.deserialize(field.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let field = &self.fields[self.field_index];
        self.field_index += 1;
        // Set this as the current key in the deserializer.
        let field = match field {
            KeyKind::Normal(field) => {
                self.de.key.push(field);
                field
            }
            KeyKind::CaseSensitive(field) => {
                self.de.key.push_sensitive(field);
                field
            }
        };
        // Env vars that are a prefix of another with a dash/underscore cannot
        // be supported by our serde implementation, so check for them here.
        // Example:
        //     CARGO_BUILD_TARGET
        //     CARGO_BUILD_TARGET_DIR
        // or
        //     CARGO_PROFILE_DEV_DEBUG
        //     CARGO_PROFILE_DEV_DEBUG_ASSERTIONS
        // The `deserialize_option` method does not know the type of the field.
        // If the type is an Option<struct> (like
        // `profile.dev.build-override`), then it needs to check for env vars
        // starting with CARGO_FOO_BAR_. This is a problem for keys like
        // CARGO_BUILD_TARGET because checking for a prefix would incorrectly
        // match CARGO_BUILD_TARGET_DIR. `deserialize_option` would have no
        // choice but to call `visit_some()` which would then fail if
        // CARGO_BUILD_TARGET isn't set. So we check for these prefixes and
        // disallow them here.
        let env_prefix = format!("{}_", field).replace('-', "_");
        let env_prefix_ok = !self.fields.iter().any(|field| {
            let field = match field {
                KeyKind::Normal(s) | KeyKind::CaseSensitive(s) => s.as_str(),
            };
            field.replace('-', "_").starts_with(&env_prefix)
        });

        let result = seed
            .deserialize(Deserializer {
                gctx: self.de.gctx,
                key: self.de.key.clone(),
                env_prefix_ok,
            })
            .map_err(|e| {
                if !e.is_missing_field() {
                    return e;
                }
                e.with_key_context(
                    &self.de.key,
                    self.de
                        .gctx
                        .get_cv_with_env(&self.de.key)
                        .ok()
                        .and_then(|cv| cv.map(|cv| cv.definition().clone())),
                )
            });
        self.de.key.pop();
        result
    }
}

struct ConfigSeqAccess<'gctx> {
    de: Deserializer<'gctx>,
    list_iter: std::iter::Enumerate<vec::IntoIter<CV>>,
}

impl ConfigSeqAccess<'_> {
    fn new(de: Deserializer<'_>) -> Result<ConfigSeqAccess<'_>, ConfigError> {
        let mut res = Vec::new();

        match de.gctx.get_cv(&de.key)? {
            Some(CV::List(val, _definition)) => {
                res.extend(val);
            }
            Some(val) => {
                de.gctx.expected("list", &de.key, &val)?;
            }
            None => {}
        }

        de.gctx.get_env_list(&de.key, &mut res)?;

        Ok(ConfigSeqAccess {
            de,
            list_iter: res.into_iter().enumerate(),
        })
    }
}

impl<'de, 'gctx> de::SeqAccess<'de> for ConfigSeqAccess<'gctx> {
    type Error = ConfigError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        let Some((i, cv)) = self.list_iter.next() else {
            return Ok(None);
        };

        let mut key_path = ArrayItemKeyPath::new(self.de.key.clone());
        let definition = Some(cv.definition().clone());
        let de = ArrayItemDeserializer {
            cv,
            key_path: &mut key_path,
        };
        seed.deserialize(de)
            .map_err(|e| {
                // This along with ArrayItemKeyPath provide a better error context of the
                // ConfigValue definition + the key path within an array item that native
                // TOML key path can't express. For example, `foo.bar[3].baz`.
                key_path.push_index(i);
                e.with_array_item_key_context(&key_path, definition)
            })
            .map(Some)
    }
}

/// Source of data for [`ValueDeserializer`]
enum ValueSource<'gctx, 'err> {
    /// The deserializer used to actually deserialize a Value struct.
    Deserializer {
        de: Deserializer<'gctx>,
        definition: Definition,
    },
    /// A [`ConfigValue`](CV).
    ///
    /// This is used for situations where you can't address type via a TOML key,
    /// such as a value inside an array.
    /// The [`ConfigSeqAccess`] doesn't know what type it should deserialize to
    /// so [`ArrayItemDeserializer`] needs to be able to handle all of them.
    ConfigValue {
        cv: CV,
        key_path: &'err mut ArrayItemKeyPath,
    },
}

impl<'gctx, 'err> ValueSource<'gctx, 'err> {
    fn with_deserializer(de: Deserializer<'gctx>) -> Result<ValueSource<'gctx, 'err>, ConfigError> {
        // Figure out where this key is defined.
        let definition = {
            let env = de.key.as_env_key();
            let env_def = Definition::Environment(env.to_string());
            match (de.gctx.env.contains_key(env), de.gctx.get_cv(&de.key)?) {
                (true, Some(cv)) => {
                    // Both, pick highest priority.
                    if env_def.is_higher_priority(cv.definition()) {
                        env_def
                    } else {
                        cv.definition().clone()
                    }
                }
                (false, Some(cv)) => cv.definition().clone(),
                // Assume it is an environment, even if the key is not set.
                // This can happen for intermediate tables, like
                // CARGO_FOO_BAR_* where `CARGO_FOO_BAR` is not set.
                (_, None) => env_def,
            }
        };

        Ok(Self::Deserializer { de, definition })
    }

    fn with_cv(cv: CV, key_path: &'err mut ArrayItemKeyPath) -> ValueSource<'gctx, 'err> {
        ValueSource::ConfigValue { cv, key_path }
    }
}

/// This is a deserializer that deserializes into a `Value<T>` for
/// configuration.
///
/// This is a special deserializer because it deserializes one of its struct
/// fields into the location that this configuration value was defined in.
///
/// See more comments in `value.rs` for the protocol used here.
struct ValueDeserializer<'gctx, 'err> {
    hits: u32,
    source: ValueSource<'gctx, 'err>,
}

impl<'gctx, 'err> ValueDeserializer<'gctx, 'err> {
    fn new(source: ValueSource<'gctx, 'err>) -> ValueDeserializer<'gctx, 'err> {
        Self { hits: 0, source }
    }

    fn definition(&self) -> &Definition {
        match &self.source {
            ValueSource::Deserializer { definition, .. } => definition,
            ValueSource::ConfigValue { cv, .. } => cv.definition(),
        }
    }
}

impl<'de, 'gctx, 'err> de::MapAccess<'de> for ValueDeserializer<'gctx, 'err> {
    type Error = ConfigError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        self.hits += 1;
        match self.hits {
            1 => seed
                .deserialize(value::VALUE_FIELD.into_deserializer())
                .map(Some),
            2 => seed
                .deserialize(value::DEFINITION_FIELD.into_deserializer())
                .map(Some),
            _ => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        // If this is the first time around we deserialize the `value` field
        // which is the actual deserializer
        if self.hits == 1 {
            return match &mut self.source {
                ValueSource::Deserializer { de, definition } => seed
                    .deserialize(de.clone())
                    .map_err(|e| e.with_key_context(&de.key, Some(definition.clone()))),
                ValueSource::ConfigValue { cv, key_path } => {
                    let de = ArrayItemDeserializer {
                        cv: cv.clone(),
                        key_path,
                    };
                    seed.deserialize(de)
                }
            };
        }

        // ... otherwise we're deserializing the `definition` field, so we need
        // to figure out where the field we just deserialized was defined at.
        match self.definition() {
            Definition::BuiltIn => seed.deserialize(0.into_deserializer()),
            Definition::Path(path) => {
                seed.deserialize(Tuple2Deserializer(1i32, path.to_string_lossy()))
            }
            Definition::Environment(env) => {
                seed.deserialize(Tuple2Deserializer(2i32, env.as_str()))
            }
            Definition::Cli(path) => {
                let s = path
                    .as_ref()
                    .map(|p| p.to_string_lossy())
                    .unwrap_or_default();
                seed.deserialize(Tuple2Deserializer(3i32, s))
            }
        }
    }
}

/// A deserializer for individual [`ConfigValue`](CV) items in arrays
///
/// It is implemented to handle any types inside a sequence, like `Vec<String>`,
/// `Vec<Value<i32>>`, or even `Vev<HashMap<String, Vec<bool>>>`.
struct ArrayItemDeserializer<'err> {
    cv: CV,
    key_path: &'err mut ArrayItemKeyPath,
}

impl<'de, 'err> de::Deserializer<'de> for ArrayItemDeserializer<'err> {
    type Error = ConfigError;

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        // Match on the magical struct name/field names that are passed in to
        // detect when we're deserializing `Value<T>`.
        //
        // See more comments in `value.rs` for the protocol used here.
        if name == value::NAME && fields == value::FIELDS {
            let source = ValueSource::with_cv(self.cv, self.key_path);
            return visitor.visit_map(ValueDeserializer::new(source));
        }
        visitor.visit_map(ArrayItemMapAccess::with_struct(
            self.cv,
            fields,
            self.key_path,
        ))
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.cv {
            CV::String(s, _) => visitor.visit_string(s),
            CV::Integer(i, _) => visitor.visit_i64(i),
            CV::Boolean(b, _) => visitor.visit_bool(b),
            l @ CV::List(_, _) => visitor.visit_seq(ArrayItemSeqAccess::new(l, self.key_path)),
            t @ CV::Table(_, _) => visitor.visit_map(ArrayItemMapAccess::new(t, self.key_path)),
        }
    }

    // Forward everything to deserialize_any
    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map option unit newtype_struct
        ignored_any unit_struct tuple_struct tuple enum identifier
    }
}

/// Sequence access for nested arrays within [`ArrayItemDeserializer`]
struct ArrayItemSeqAccess<'err> {
    items: std::iter::Enumerate<vec::IntoIter<CV>>,
    key_path: &'err mut ArrayItemKeyPath,
}

impl<'err> ArrayItemSeqAccess<'err> {
    fn new(cv: CV, key_path: &'err mut ArrayItemKeyPath) -> ArrayItemSeqAccess<'err> {
        let items = match cv {
            CV::List(list, _) => list.into_iter().enumerate(),
            _ => unreachable!("must be a list"),
        };
        Self { items, key_path }
    }
}

impl<'de, 'err> de::SeqAccess<'de> for ArrayItemSeqAccess<'err> {
    type Error = ConfigError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.items.next() {
            Some((i, cv)) => {
                let de = ArrayItemDeserializer {
                    cv,
                    key_path: self.key_path,
                };
                seed.deserialize(de)
                    .inspect_err(|_| self.key_path.push_index(i))
                    .map(Some)
            }
            None => Ok(None),
        }
    }
}

/// Map access for nested tables within [`ArrayItemDeserializer`]
struct ArrayItemMapAccess<'err> {
    cv: CV,
    keys: vec::IntoIter<String>,
    current_key: Option<String>,
    key_path: &'err mut ArrayItemKeyPath,
}

impl<'err> ArrayItemMapAccess<'err> {
    fn new(cv: CV, key_path: &'err mut ArrayItemKeyPath) -> Self {
        let keys = match &cv {
            CV::Table(map, _) => map.keys().cloned().collect::<Vec<_>>().into_iter(),
            _ => unreachable!("must be a map"),
        };
        Self {
            cv,
            keys,
            current_key: None,
            key_path,
        }
    }

    fn with_struct(cv: CV, given_fields: &[&str], key_path: &'err mut ArrayItemKeyPath) -> Self {
        // TODO: We might want to warn unused fields,
        // like what we did in ConfigMapAccess::new_struct
        let keys = given_fields
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter();
        Self {
            cv,
            keys,
            current_key: None,
            key_path,
        }
    }
}

impl<'de, 'err> de::MapAccess<'de> for ArrayItemMapAccess<'err> {
    type Error = ConfigError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.keys.next() {
            Some(key) => {
                self.current_key = Some(key.clone());
                seed.deserialize(key.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let key = self.current_key.take().unwrap();
        match &self.cv {
            CV::Table(map, _) => {
                if let Some(cv) = map.get(&key) {
                    let de = ArrayItemDeserializer {
                        cv: cv.clone(),
                        key_path: self.key_path,
                    };
                    seed.deserialize(de)
                        .inspect_err(|_| self.key_path.push_key(key))
                } else {
                    Err(ConfigError::new(
                        format!("missing config key `{key}`"),
                        self.cv.definition().clone(),
                    ))
                }
            }
            _ => Err(ConfigError::new(
                "expected table".to_string(),
                self.cv.definition().clone(),
            )),
        }
    }
}

/// A deserializer which takes two values and deserializes into a tuple of those
/// two values. This is similar to types like `StrDeserializer` in upstream
/// serde itself.
struct Tuple2Deserializer<T, U>(T, U);

impl<'de, T, U> de::Deserializer<'de> for Tuple2Deserializer<T, U>
where
    T: IntoDeserializer<'de, ConfigError>,
    U: IntoDeserializer<'de, ConfigError>,
{
    type Error = ConfigError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, ConfigError>
    where
        V: de::Visitor<'de>,
    {
        struct SeqVisitor<T, U> {
            first: Option<T>,
            second: Option<U>,
        }
        impl<'de, T, U> de::SeqAccess<'de> for SeqVisitor<T, U>
        where
            T: IntoDeserializer<'de, ConfigError>,
            U: IntoDeserializer<'de, ConfigError>,
        {
            type Error = ConfigError;
            fn next_element_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
            where
                K: de::DeserializeSeed<'de>,
            {
                if let Some(first) = self.first.take() {
                    return seed.deserialize(first.into_deserializer()).map(Some);
                }
                if let Some(second) = self.second.take() {
                    return seed.deserialize(second.into_deserializer()).map(Some);
                }
                Ok(None)
            }
        }

        visitor.visit_seq(SeqVisitor {
            first: Some(self.0),
            second: Some(self.1),
        })
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map struct option unit newtype_struct
        ignored_any unit_struct tuple_struct tuple enum identifier
    }
}
