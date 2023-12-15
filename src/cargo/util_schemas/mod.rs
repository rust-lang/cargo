//! Low-level Cargo format schemas
//!
//! This is types with logic mostly focused on `serde` and `FromStr` for use in reading files and
//! parsing command-lines.
//! Any logic for getting final semantics from these will likely need other tools to process, like
//! `cargo metadata`.

pub mod core;
pub mod manifest;

mod restricted_names;
