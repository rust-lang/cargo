//! Miscellaneous support code used by Cargo.

pub use self::read2::read2;
pub use process_builder::ProcessBuilder;
pub use process_error::{exit_status_to_string, is_simple_exit_code, ProcessError};

pub mod paths;
mod process_builder;
mod process_error;
mod read2;

/// Whether or not this running in a Continuous Integration environment.
pub fn is_ci() -> bool {
    std::env::var("CI").is_ok() || std::env::var("TF_BUILD").is_ok()
}
