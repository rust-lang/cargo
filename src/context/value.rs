//! Deserialization of a [`Value<T>`] type which tracks where it was deserialized from.
//!
//! ## Rationale for `Value<T>`
//!
//! Often Cargo wants to report semantic error information or other sorts of
//! error information about configuration keys but it also may wish to indicate
//! as an error context where the key was defined as well (to help user
//! debugging). The `Value<T>` type here can be used to deserialize a `T` value
//! from configuration, but also record where it was deserialized from when it
//! was read.
//!
//! Deserializing `Value<T>` is pretty special, and serde doesn't have built-in
//! support for this operation. To implement this we extend serde's "data model"
//! a bit. We configure deserialization of `Value<T>` to basically only work with
//! our one deserializer using configuration.
//!
//! ## How `Value<T>` deserialization works
//!
//! `Value<T>` uses a custom protocol to inject source location information
//! into serde's deserialization process:
//!
//! **Magic identifiers**: `Value<T>::deserialize` requests a struct with special
//! [name](NAME) and [field names](FIELDS) that use invalid Rust syntax to avoid
//! conflicts. This signals to Cargo's deserializer that location tracking is needed.
//!
//! **Custom deserializer response**: When Cargo's deserializer sees these magic
//! identifiers, it switches to `ValueDeserializer` (from the [`de`] module)
//! instead of normal struct deserialization.
//!
//! **Two-field protocol**: `ValueDeserializer` presents exactly two fields
//! through map visiting:
//! * The actual value (deserialized normally)
//! * The definition context (encoded as a `(u32, String)` tuple acting as a
//!   tagged union of [`Definition`] variants)
//!
//! This allows `Value<T>` to capture both the deserialized data and where it
//! came from.
//!
//! **Note**: When modifying [`Definition`] variants, be sure to update both
//! the `Definition::deserialize` implementation here and the
//! `MapAccess::next_value_seed` implementation in `ValueDeserializer`.
//!
//! [`de`]: crate::util::context::de

use serde::de;
use std::cmp::Ordering;
use std::fmt;
use std::marker;
use std::mem;
use std::path::{Path, PathBuf};

/// A type which can be deserialized as a configuration value which records
/// where it was deserialized from.
#[derive(Debug, PartialEq, Clone)]
pub struct Value<T> {
    /// The inner value that was deserialized.
    pub val: T,
    /// The location where `val` was defined in configuration (e.g. file it was
    /// defined in, env var etc).
    pub definition: Definition,
}

pub type OptValue<T> = Option<Value<T>>;

// The names below are intended to be invalid Rust identifiers
// to avoid conflicts with other valid structures.
pub(crate) const VALUE_FIELD: &str = "$__cargo_private_value";
pub(crate) const DEFINITION_FIELD: &str = "$__cargo_private_definition";
pub(crate) const NAME: &str = "$__cargo_private_Value";
pub(crate) static FIELDS: [&str; 2] = [VALUE_FIELD, DEFINITION_FIELD];

/// Location where a config value is defined.
#[derive(Clone, Debug, Eq)]
pub enum Definition {
    BuiltIn,
    /// Defined in a `.cargo/config`, includes the path to the file.
    Path(PathBuf),
    /// Defined in an environment variable, includes the environment key.
    Environment(String),
    /// Passed in on the command line.
    /// A path is attached when the config value is a path to a config file.
    Cli(Option<PathBuf>),
}

impl PartialOrd for Definition {
    fn partial_cmp(&self, other: &Definition) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Definition {
    fn cmp(&self, other: &Definition) -> Ordering {
        if mem::discriminant(self) == mem::discriminant(other) {
            Ordering::Equal
        } else if self.is_higher_priority(other) {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}

impl Definition {
    /// Root directory where this is defined.
    ///
    /// If from a file, it is the directory above `.cargo/config.toml`.
    /// CLI and env use the provided current working directory.
    pub fn root<'a>(&'a self, cwd: &'a Path) -> &'a Path {
        match self {
            Definition::Path(p) | Definition::Cli(Some(p)) => p.parent().unwrap().parent().unwrap(),
            Definition::Environment(_) | Definition::Cli(None) | Definition::BuiltIn => cwd,
        }
    }

    /// Returns true if self is a higher priority to other.
    ///
    /// CLI is preferred over environment, which is preferred over files.
    pub fn is_higher_priority(&self, other: &Definition) -> bool {
        matches!(
            (self, other),
            (Definition::Cli(_), Definition::Environment(_))
                | (Definition::Cli(_), Definition::Path(_))
                | (Definition::Cli(_), Definition::BuiltIn)
                | (Definition::Environment(_), Definition::Path(_))
                | (Definition::Environment(_), Definition::BuiltIn)
                | (Definition::Path(_), Definition::BuiltIn)
        )
    }
}

impl PartialEq for Definition {
    fn eq(&self, other: &Definition) -> bool {
        // configuration values are equivalent no matter where they're defined,
        // but they need to be defined in the same location. For example if
        // they're defined in the environment that's different than being
        // defined in a file due to path interpretations.
        mem::discriminant(self) == mem::discriminant(other)
    }
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Definition::Path(p) | Definition::Cli(Some(p)) => p.display().fmt(f),
            Definition::Environment(key) => write!(f, "environment variable `{}`", key),
            Definition::Cli(None) => write!(f, "--config cli option"),
            Definition::BuiltIn => write!(f, "default"),
        }
    }
}

impl<T> From<T> for Value<T> {
    fn from(val: T) -> Self {
        Self {
            val,
            definition: Definition::BuiltIn,
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
        match discr {
            0 => Ok(Definition::BuiltIn),
            1 => Ok(Definition::Path(value.into())),
            2 => Ok(Definition::Environment(value)),
            3 => {
                let path = (value.len() > 0).then_some(value.into());
                Ok(Definition::Cli(path))
            }
            _ => panic!("unexpected discriminant {discr} value {value}"),
        }
    }
}
