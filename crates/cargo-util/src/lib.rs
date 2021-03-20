//! Miscellaneous support code used by Cargo.

pub use self::read2::read2;
pub use process_builder::ProcessBuilder;
pub use process_error::{exit_status_to_string, is_simple_exit_code, ProcessError};

mod process_builder;
mod process_error;
mod read2;
