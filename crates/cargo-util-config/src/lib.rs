//! Cargo's layered config system
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo
//! > and not intended for external use (except as a transitive dependency). This
//! > crate may make major changes to its APIs or be deprecated without warning.

mod config_store;
mod config_value;
mod de;
mod definition;
mod env;
mod error;
mod key;
mod reader;
pub mod schema;
mod value;

pub use config_store::ConfigStore;
pub use config_value::ConfigValue;
pub use definition::{Definition, DefinitionKind};
pub use env::Env;
pub use error::ConfigError;
pub use key::ConfigKey;
pub use reader::{ConfigReader, expected};
pub use value::{OptValue, Value};

pub(crate) use config_value::is_nonmergeable_list;
pub(crate) use key::ArrayItemKeyPath;

const CRATES_IO_REGISTRY: &str = "crates-io";
