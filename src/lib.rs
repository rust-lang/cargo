//! Convenience wrappers for cargo buildscript input/output.

/// Inputs to the build script, in the form of environment variables.
pub mod input;
/// Outputs from the build script, in the form of `cargo:` printed lines.
///
/// _Does not print a leading newline._ Thus, if you ever write to stdout and
/// don't lock until a trailing newline, these instructions will likely fail.
pub mod output;

#[doc(no_inline)]
pub use crate::{input::*, output::*};
