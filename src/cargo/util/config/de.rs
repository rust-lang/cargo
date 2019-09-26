//! Support for deserializing configuration via `serde`

use crate::util::config::{Config, ConfigError, ConfigKey, ConfigKeyPart};
use crate::util::config::{ConfigValue as CV, Value, Definition};
use std::path::PathBuf;
use serde::{de, de::IntoDeserializer};
use std::collections::HashSet;
use std::vec;

/// Serde deserializer used to convert config values to a target type using
/// `Config::get`.
pub(crate) struct Deserializer<'config> {
    pub(crate) config: &'config Config,
    pub(crate) key: ConfigKey,
}

macro_rules! deserialize_method {
    ($method:ident, $visit:ident, $getter:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: de::Visitor<'de>,
        {
            let v = self.config.$getter(&self.key)?.ok_or_else(||
                ConfigError::missing(&self.key.to_config()))?;
            let Value{val, definition} = v;
            let res: Result<V::Value, ConfigError> = visitor.$visit(val);
            res.map_err(|e| e.with_key_context(&self.key.to_config(), definition))
        }
    }
}

impl<'de, 'config> de::Deserializer<'de> for Deserializer<'config> {
    type Error = ConfigError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        // Future note: If you ever need to deserialize a non-self describing
        // map type, this should implement a starts_with check (similar to how
        // ConfigMapAccess does).
        if let Some(v) = self.config.env.get(&self.key.to_env()) {
            let res: Result<V::Value, ConfigError> = if v == "true" || v == "false" {
                visitor.visit_bool(v.parse().unwrap())
            } else if let Ok(v) = v.parse::<i64>() {
                visitor.visit_i64(v)
            } else if self.config.cli_unstable().advanced_env
                && v.starts_with('[')
                && v.ends_with(']')
            {
                visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?)
            } else {
                visitor.visit_string(v.clone())
            };
            return res.map_err(|e| {
                e.with_key_context(
                    &self.key.to_config(),
                    Definition::Environment(self.key.to_env()),
                )
            });
        }

        let o_cv = self.config.get_cv(&self.key.to_config())?;
        if let Some(cv) = o_cv {
            let res: (Result<V::Value, ConfigError>, PathBuf) = match cv {
                CV::Integer(i, path) => (visitor.visit_i64(i), path),
                CV::String(s, path) => (visitor.visit_string(s), path),
                CV::List(_, path) => (
                    visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?),
                    path,
                ),
                CV::Table(_, path) => (
                    visitor.visit_map(ConfigMapAccess::new_map(self.config, self.key.clone())?),
                    path,
                ),
                CV::Boolean(b, path) => (visitor.visit_bool(b), path),
            };
            let (res, path) = res;
            return res
                .map_err(|e| e.with_key_context(&self.key.to_config(), Definition::Path(path)));
        }
        Err(ConfigError::missing(&self.key.to_config()))
    }

    deserialize_method!(deserialize_bool, visit_bool, get_bool_priv);
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
        if self.config.has_key(&self.key) {
            visitor.visit_some(self)
        } else {
            // Treat missing values as `None`.
            visitor.visit_none()
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_map(ConfigMapAccess::new_struct(self.config, self.key, fields)?)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_map(ConfigMapAccess::new_map(self.config, self.key)?)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?)
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
        visitor.visit_seq(ConfigSeqAccess::new(self.config, &self.key)?)
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if name == "ConfigRelativePath" {
            match self.config.get_string_priv(&self.key)? {
                Some(v) => {
                    let path = v
                        .definition
                        .root(self.config)
                        .join(v.val)
                        .display()
                        .to_string();
                    visitor.visit_newtype_struct(path.into_deserializer())
                }
                None => Err(ConfigError::missing(&self.key.to_config())),
            }
        } else {
            visitor.visit_newtype_struct(self)
        }
    }

    // These aren't really supported, yet.
    serde::forward_to_deserialize_any! {
        f32 f64 char str bytes
        byte_buf unit unit_struct
        enum identifier ignored_any
    }
}

struct ConfigMapAccess<'config> {
    config: &'config Config,
    key: ConfigKey,
    set_iter: <HashSet<ConfigKeyPart> as IntoIterator>::IntoIter,
    next: Option<ConfigKeyPart>,
}

impl<'config> ConfigMapAccess<'config> {
    fn new_map(
        config: &'config Config,
        key: ConfigKey,
    ) -> Result<ConfigMapAccess<'config>, ConfigError> {
        let mut set = HashSet::new();
        if let Some(mut v) = config.get_table(&key.to_config())? {
            // `v: Value<HashMap<String, CV>>`
            for (key, _value) in v.val.drain() {
                set.insert(ConfigKeyPart::CasePart(key));
            }
        }
        if config.cli_unstable().advanced_env {
            // `CARGO_PROFILE_DEV_OVERRIDES_`
            let env_pattern = format!("{}_", key.to_env());
            for env_key in config.env.keys() {
                if env_key.starts_with(&env_pattern) {
                    // `CARGO_PROFILE_DEV_OVERRIDES_bar_OPT_LEVEL = 3`
                    let rest = &env_key[env_pattern.len()..];
                    // `rest = bar_OPT_LEVEL`
                    let part = rest.splitn(2, '_').next().unwrap();
                    // `part = "bar"`
                    set.insert(ConfigKeyPart::CasePart(part.to_string()));
                }
            }
        }
        Ok(ConfigMapAccess {
            config,
            key,
            set_iter: set.into_iter(),
            next: None,
        })
    }

    fn new_struct(
        config: &'config Config,
        key: ConfigKey,
        fields: &'static [&'static str],
    ) -> Result<ConfigMapAccess<'config>, ConfigError> {
        let mut set = HashSet::new();
        for field in fields {
            set.insert(ConfigKeyPart::Part(field.to_string()));
        }
        if let Some(mut v) = config.get_table(&key.to_config())? {
            for (t_key, value) in v.val.drain() {
                let part = ConfigKeyPart::Part(t_key);
                if !set.contains(&part) {
                    config.shell().warn(format!(
                        "unused key `{}` in config file `{}`",
                        key.join(part).to_config(),
                        value.definition_path().display()
                    ))?;
                }
            }
        }
        Ok(ConfigMapAccess {
            config,
            key,
            set_iter: set.into_iter(),
            next: None,
        })
    }
}

impl<'de, 'config> de::MapAccess<'de> for ConfigMapAccess<'config> {
    type Error = ConfigError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.set_iter.next() {
            Some(key) => {
                let de_key = key.to_config();
                self.next = Some(key);
                seed.deserialize(de_key.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let next_key = self.next.take().expect("next field missing");
        let next_key = self.key.join(next_key);
        seed.deserialize(Deserializer {
            config: self.config,
            key: next_key,
        })
    }
}

struct ConfigSeqAccess {
    list_iter: vec::IntoIter<(String, Definition)>,
}

impl ConfigSeqAccess {
    fn new(config: &Config, key: &ConfigKey) -> Result<ConfigSeqAccess, ConfigError> {
        let mut res = Vec::new();
        if let Some(v) = config.get_list(&key.to_config())? {
            for (s, path) in v.val {
                res.push((s, Definition::Path(path)));
            }
        }

        if config.cli_unstable().advanced_env {
            // Parse an environment string as a TOML array.
            let env_key = key.to_env();
            let def = Definition::Environment(env_key.clone());
            if let Some(v) = config.env.get(&env_key) {
                if !(v.starts_with('[') && v.ends_with(']')) {
                    return Err(ConfigError::new(
                        format!("should have TOML list syntax, found `{}`", v),
                        def,
                    ));
                }
                let temp_key = key.last().to_env();
                let toml_s = format!("{}={}", temp_key, v);
                let toml_v: toml::Value = toml::de::from_str(&toml_s).map_err(|e| {
                    ConfigError::new(format!("could not parse TOML list: {}", e), def.clone())
                })?;
                let values = toml_v
                    .as_table()
                    .unwrap()
                    .get(&temp_key)
                    .unwrap()
                    .as_array()
                    .expect("env var was not array");
                for value in values {
                    // TODO: support other types.
                    let s = value.as_str().ok_or_else(|| {
                        ConfigError::new(
                            format!("expected string, found {}", value.type_str()),
                            def.clone(),
                        )
                    })?;
                    res.push((s.to_string(), def.clone()));
                }
            }
        }
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
            Some((value, _def)) => seed.deserialize(value.into_deserializer()).map(Some),
            None => Ok(None),
        }
    }
}
