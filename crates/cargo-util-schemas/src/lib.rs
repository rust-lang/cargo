//! Low-level Cargo format schemas
//!
//! This is types with logic mostly focused on `serde` and `FromStr` for use in reading files and
//! parsing command-lines.
//! Any logic for getting final semantics from these will likely need other tools to process, like
//! `cargo metadata`.
//!
//! > This crate is maintained by the Cargo team for use by the wider
//! > ecosystem. This crate follows semver compatibility for its APIs.

pub mod core;
pub mod manifest;

mod restricted_names;
