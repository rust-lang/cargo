//! Cargo's layered config system
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo
//! > and not intended for external use (except as a transitive dependency). This
//! > crate may make major changes to its APIs or be deprecated without warning.
//!
//! [`ConfigLoader`] discovers and loads config files as [`ConfigStore`] layers.
//! [`ConfigReader::get`] is the primary way to load [`schema`]s.
//!
//! # Example
//!
//! ```rust
//! # fn main() -> anyhow::Result<()> {
//! let shell = // ...
//! # std::sync::Mutex::new(cargo_util_terminal::Shell::new());
//! let current_dir = // ...
//! # std::path::PathBuf::from("/nonexistent/project");
//! let cargo_home = // ...
//! # std::path::PathBuf::from("/nonexistent/cargo-home");
//!
//! let loader = cargo_util_config::ConfigLoader::new(&shell, &current_dir, &cargo_home);
//! let layers = loader.load_layers(&current_dir)?;
//! let config = cargo_util_config::ConfigStore::from_layers(layers)?;
//! let env = // ...
//! # cargo_util_config::Env::new();
//!
//! let reader = cargo_util_config::ConfigReader::new(&shell)
//!   .config(&config)
//!   .env(&env);
//!
//! use cargo_util_config::schema::CargoBuildConfig;
//! let build_key = cargo_util_config::ConfigKey::from_parts(["build"]);
//! let build = reader.get::<CargoBuildConfig>(build_key)?;
//! # Ok(())
//! # }
//! ```

mod config_store;
mod config_value;
mod de;
mod definition;
mod env;
mod error;
mod key;
mod loader;
mod reader;
pub mod schema;
mod value;

pub use config_store::ConfigStore;
pub use config_value::ConfigValue;
pub use definition::{Definition, DefinitionKind};
pub use env::Env;
pub use error::ConfigError;
pub use key::ConfigKey;
pub use loader::ConfigLoader;
pub use reader::{ConfigReader, expected};
pub use value::{OptValue, Value};

pub(crate) use config_value::is_nonmergeable_list;
pub(crate) use key::ArrayItemKeyPath;

const CRATES_IO_REGISTRY: &str = "crates-io";
