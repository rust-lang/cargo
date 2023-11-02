//! Support for deserializing configuration via `serde`

use crate::util::config::value;
use crate::util::config::{Config, ConfigError, ConfigKey};
use crate::util::config::{ConfigValue as CV, Definition, Value};
use serde::{de, de::IntoDeserializer};
use std::collections::HashSet;
use std::vec;

/// Serde deserializer used to convert config values to a target type using
/// `Config::get`.
#[derive(Clone)]
pub(super) struct Deserializer<'config> {
    pub(super) config: &'config Config,
    /// The current key being deserialized.
    pub(super) key: ConfigKey,
    /// Whether or not this key part is allowed to be an inner table. For
    /// example, `profile.dev.build-override` needs to check if
    /// CARGO_PROFILE_DEV_BUILD_OVERRIDE_ prefixes exist. But
    /// CARGO_BUILD_TARGET should not check for prefixes because it would
    /// collide with CARGO_BUILD_TARGET_DIR. See `ConfigMapAccess` for
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
                .config
                .$getter(&self.key)?
                .ok_or_else(|| ConfigError::missing(&self.key))?;
            let Value { val, definition } = v;
            let res: Result<V::Value, ConfigError> = visitor.$visit(val);
            res.map_err(|e| e.with_key_context(&self.key, definition))
        }
    };
}

impl<'de, 'config> de::Deserializer<'de> for Deserializer<'config> {
    type Error = ConfigError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let cv = self.config.get_cv_with_env(&self.key)?;
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
            return res.map_err(|e| e.with_key_context(&self.key, def));
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
        if self.config.has_key(&self.key, self.env_prefix_ok)? {
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
            return visitor.visit_map(ValueDeserializer::new(self)?);
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
        let merge = if name == "StringList" {
            true
        } else if name == "UnmergedStringList" {
            false
        } else {
            return visitor.visit_newtype_struct(self);
        };

        let vals = self.config.get_list_or_string(&self.key, merge)?;
        let vals: Vec<String> = vals.into_iter().map(|vd| vd.0).collect();
        visitor.visit_newtype_struct(vals.into_deserializer())
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
            .config
            .get_string_priv(&self.key)?
            .ok_or_else(|| ConfigError::missing(&self.key))?;

        let Value { val, definition } = value;
        visitor
            .visit_enum(val.into_deserializer())
            .map_err(|e: ConfigError| e.with_key_context(&self.key, definition))
    }

    // These aren't really supported, yet.
    serde::forward_to_deserialize_any! {
        f32 f64 char str bytes
        byte_buf unit unit_struct
        identifier ignored_any
    }
}

struct ConfigMapAccess<'config> {
    de: Deserializer<'config>,
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

impl<'config> ConfigMapAccess<'config> {
    fn new_map(de: Deserializer<'config>) -> Result<ConfigMapAccess<'config>, ConfigError> {
        let mut fields = Vec::new();
        if let Some(mut v) = de.config.get_table(&de.key)? {
            // `v: Value<HashMap<String, CV>>`
            for (key, _value) in v.val.drain() {
                fields.push(KeyKind::CaseSensitive(key));
            }
        }
        if de.config.cli_unstable().advanced_env {
            // `CARGO_PROFILE_DEV_PACKAGE_`
            let env_prefix = format!("{}_", de.key.as_env_key());
            for env_key in de.config.env_keys() {
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
        de: Deserializer<'config>,
        given_fields: &'static [&'static str],
    ) -> Result<ConfigMapAccess<'config>, ConfigError> {
        let table = de.config.get_table(&de.key)?;

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
                de.config.shell().warn(format!(
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
            for env_key in de.config.env_keys() {
                if env_key.starts_with(field_key.as_env_key()) {
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

impl<'de, 'config> de::MapAccess<'de> for ConfigMapAccess<'config> {
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

        let result = seed.deserialize(Deserializer {
            config: self.de.config,
            key: self.de.key.clone(),
            env_prefix_ok,
        });
        self.de.key.pop();
        result
    }
}

struct ConfigSeqAccess {
    list_iter: vec::IntoIter<(String, Definition)>,
}

impl ConfigSeqAccess {
    fn new(de: Deserializer<'_>) -> Result<ConfigSeqAccess, ConfigError> {
        let mut res = Vec::new();
        if let Some(v) = de.config._get_list(&de.key)? {
            res.extend(v.val);
        }

        de.config.get_env_list(&de.key, &mut res)?;

        Ok(ConfigSeqAccess {
            list_iter: res.into_iter(),
        })
    }
}

impl<'de> de::SeqAccess<'de> for ConfigSeqAccess {
    type Error = ConfigError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.list_iter.next() {
            // TODO: add `def` to error?
            Some((value, def)) => {
                // This might be a String or a Value<String>.
                // ValueDeserializer will handle figuring out which one it is.
                let maybe_value_de = ValueDeserializer::new_with_string(value, def);
                seed.deserialize(maybe_value_de).map(Some)
            }
            None => Ok(None),
        }
    }
}

/// This is a deserializer that deserializes into a `Value<T>` for
/// configuration.
///
/// This is a special deserializer because it deserializes one of its struct
/// fields into the location that this configuration value was defined in.
///
/// See more comments in `value.rs` for the protocol used here.
struct ValueDeserializer<'config> {
    hits: u32,
    definition: Definition,
    /// The deserializer, used to actually deserialize a Value struct.
    /// This is `None` if deserializing a string.
    de: Option<Deserializer<'config>>,
    /// A string value to deserialize.
    ///
    /// This is used for situations where you can't address a string via a
    /// TOML key, such as a string inside an array. The `ConfigSeqAccess`
    /// doesn't know if the type it should deserialize to is a `String` or
    /// `Value<String>`, so `ValueDeserializer` needs to be able to handle
    /// both.
    str_value: Option<String>,
}

impl<'config> ValueDeserializer<'config> {
    fn new(de: Deserializer<'config>) -> Result<ValueDeserializer<'config>, ConfigError> {
        // Figure out where this key is defined.
        let definition = {
            let env = de.key.as_env_key();
            let env_def = Definition::Environment(env.to_string());
            match (de.config.env.contains_key(env), de.config.get_cv(&de.key)?) {
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
        Ok(ValueDeserializer {
            hits: 0,
            definition,
            de: Some(de),
            str_value: None,
        })
    }

    fn new_with_string(s: String, definition: Definition) -> ValueDeserializer<'config> {
        ValueDeserializer {
            hits: 0,
            definition,
            de: None,
            str_value: Some(s),
        }
    }
}

impl<'de, 'config> de::MapAccess<'de> for ValueDeserializer<'config> {
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
            if let Some(de) = &self.de {
                return seed
                    .deserialize(de.clone())
                    .map_err(|e| e.with_key_context(&de.key, self.definition.clone()));
            } else {
                return seed
                    .deserialize(self.str_value.as_ref().unwrap().clone().into_deserializer());
            }
        }

        // ... otherwise we're deserializing the `definition` field, so we need
        // to figure out where the field we just deserialized was defined at.
        match &self.definition {
            Definition::Path(path) => {
                seed.deserialize(Tuple2Deserializer(0i32, path.to_string_lossy()))
            }
            Definition::Environment(env) => {
                seed.deserialize(Tuple2Deserializer(1i32, env.as_str()))
            }
            Definition::Cli(path) => {
                let str = path
                    .as_ref()
                    .map(|p| p.to_string_lossy())
                    .unwrap_or_default();
                seed.deserialize(Tuple2Deserializer(2i32, str))
            }
        }
    }
}

// Deserializer is only implemented to handle deserializing a String inside a
// sequence (like `Vec<String>` or `Vec<Value<String>>`). `Value<String>` is
// handled by deserialize_struct, and the plain `String` is handled by all the
// other functions here.
impl<'de, 'config> de::Deserializer<'de> for ValueDeserializer<'config> {
    type Error = ConfigError;

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_str(&self.str_value.expect("string expected"))
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.str_value.expect("string expected"))
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
            return visitor.visit_map(self);
        }
        unimplemented!("only strings and Value can be deserialized from a sequence");
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.str_value.expect("string expected"))
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        i8 i16 i32 i64
        u8 u16 u32 u64
        option
        newtype_struct seq tuple tuple_struct map enum bool
        f32 f64 char bytes
        byte_buf unit unit_struct
        identifier
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
