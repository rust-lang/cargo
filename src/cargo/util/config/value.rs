use crate::util::config::Config;
use serde::de;
use std::fmt;
use std::marker;
use std::path::{Path, PathBuf};

pub struct Value<T> {
    pub val: T,
    pub definition: Definition,
}

pub type OptValue<T> = Option<Value<T>>;

pub(crate) const VALUE_FIELD: &str = "$__cargo_private_value";
pub(crate) const DEFINITION_FIELD: &str = "$__cargo_private_definition";
pub(crate) const NAME: &str = "$__cargo_private_Value";
pub(crate) static FIELDS: [&str; 2] = [VALUE_FIELD, DEFINITION_FIELD];

#[derive(Clone, Debug)]
pub enum Definition {
    Path(PathBuf),
    Environment(String),
}

impl Definition {
    pub fn root<'a>(&'a self, config: &'a Config) -> &'a Path {
        match self {
            Definition::Path(p) => p.parent().unwrap().parent().unwrap(),
            Definition::Environment(_) => config.cwd(),
        }
    }
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Definition::Path(p) => p.display().fmt(f),
            Definition::Environment(key) => write!(f, "environment variable `{}`", key),
        }
    }
}

impl<'de, T> de::Deserialize<'de> for Value<T>
where
    T: de::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Value<T>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct ValueVisitor<T> {
            _marker: marker::PhantomData<T>,
        }

        impl<'de, T> de::Visitor<'de> for ValueVisitor<T>
        where
            T: de::Deserialize<'de>,
        {
            type Value = Value<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a value")
            }

            fn visit_map<V>(self, mut visitor: V) -> Result<Value<T>, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let value = visitor.next_key::<ValueKey>()?;
                if value.is_none() {
                    return Err(de::Error::custom("value not found"));
                }
                let val: T = visitor.next_value()?;

                let definition = visitor.next_key::<DefinitionKey>()?;
                if definition.is_none() {
                    return Err(de::Error::custom("definition not found"));
                }
                let definition: Definition = visitor.next_value()?;
                Ok(Value { val, definition })
            }
        }

        deserializer.deserialize_struct(
            NAME,
            &FIELDS,
            ValueVisitor {
                _marker: marker::PhantomData,
            },
        )
    }
}

struct FieldVisitor {
    expected: &'static str,
}

impl<'de> de::Visitor<'de> for FieldVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a valid value field")
    }

    fn visit_str<E>(self, s: &str) -> Result<(), E>
    where
        E: de::Error,
    {
        if s == self.expected {
            Ok(())
        } else {
            Err(de::Error::custom("expected field with custom name"))
        }
    }
}

struct ValueKey;

impl<'de> de::Deserialize<'de> for ValueKey {
    fn deserialize<D>(deserializer: D) -> Result<ValueKey, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_identifier(FieldVisitor {
            expected: VALUE_FIELD,
        })?;
        Ok(ValueKey)
    }
}

struct DefinitionKey;

impl<'de> de::Deserialize<'de> for DefinitionKey {
    fn deserialize<D>(deserializer: D) -> Result<DefinitionKey, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_identifier(FieldVisitor {
            expected: DEFINITION_FIELD,
        })?;
        Ok(DefinitionKey)
    }
}

impl<'de> de::Deserialize<'de> for Definition {
    fn deserialize<D>(deserializer: D) -> Result<Definition, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let (discr, value) = <(u32, String)>::deserialize(deserializer)?;
        if discr == 0 {
            Ok(Definition::Path(value.into()))
        } else {
            Ok(Definition::Environment(value))
        }
    }
}
