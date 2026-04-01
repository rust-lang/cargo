//! Miscellaneous support code used by Cargo.
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo
//! > and not intended for external use (except as a transitive dependency). This
//! > crate may make major changes to its APIs or be deprecated without warning.

#![allow(clippy::disallowed_methods)]

mod shell;

pub mod style;

pub use annotate_snippets as report;
pub use shell::ColorChoice;
pub use shell::Hyperlink;
pub use shell::Shell;
pub use shell::TtyWidth;
pub use shell::Verbosity;

pub type CargoResult<T> = anyhow::Result<T>;
