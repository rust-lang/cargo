//! Miscellaneous support code used by Cargo.
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo
//! > and not intended for external use (except as a transitive dependency). This
//! > crate may make major changes to its APIs or be deprecated without warning.

#![allow(clippy::disallowed_methods)]

pub use self::read2::read2;
pub use du::du;
pub use process_builder::ProcessBuilder;
pub use process_error::{ProcessError, exit_status_to_string, is_simple_exit_code};
pub use sha256::Sha256;

mod du;
pub mod paths;
mod process_builder;
mod process_error;
mod read2;
pub mod registry;
mod sha256;

/// Whether or not this running in a Continuous Integration environment.
pub fn is_ci() -> bool {
    std::env::var("CI").is_ok() || std::env::var("TF_BUILD").is_ok()
}
