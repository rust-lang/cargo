//! Convenience wrappers for cargo buildscript input/output.
//!
//! # Example
//!
//! ```rust
//! build::rerun_if_changed("build.rs"); // only rerun for buildscript changes
//! build::rustc_cfg("has_buildrs"); // set #[cfg(has_buildrs)]
//! dbg!(build::cargo()); // path to the cargo executable
//! dbg!(build::cargo_manifest_dir()); // the directory of the build manifest
//! ```

/// Inputs to the build script, in the form of environment variables.
pub mod input;
/// Outputs from the build script, in the form of `cargo:` printed lines.
///
/// _Does not print a leading newline._ Thus, if you ever write to stdout and
/// don't lock until a trailing newline, these instructions will likely fail.
pub mod output;

#[doc(no_inline)]
pub use crate::{input::*, output::*};
