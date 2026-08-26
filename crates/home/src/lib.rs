//! Canonical definitions of `home_dir`, `cargo_home`, and `rustup_home`.
//!
//! The definition of `home_dir` provided by the standard library is
//! incorrect because it considers the `HOME` environment variable on
//! Windows. This causes surprising situations where a Rust program
//! will behave differently depending on whether it is run under a
//! Unix emulation environment like Cygwin or MinGW. Neither Cargo nor
//! rustup use the standard libraries definition - they use the
//! definition here.
//!
//! This crate provides two additional functions, `cargo_home` and
//! `rustup_home`, which are the canonical way to determine the
//! location that Cargo and rustup use to store their data.
//! The `env` module contains utilities for mocking the process environment
//! by Cargo and rustup.
//!
//! See also this [discussion].
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo and Rustup
//! > and not intended for external use. This
//! > crate may make major changes to its APIs or be deprecated without warning.
//!
//! [discussion]: https://github.com/rust-lang/rust/pull/46799#issuecomment-361156935

#![allow(clippy::disallowed_methods)]

pub mod env;

use std::io;
use std::path::{Path, PathBuf};

/// Returns the path of the current user's home directory using environment
/// variables or OS-specific APIs.
///
/// This function is just a wrapper around [`std::env::home_dir`](https://doc.rust-lang.org/std/env/fn.home_dir.html)
///
/// # Examples
///
/// ```
/// match home::home_dir() {
///     Some(path) if !path.as_os_str().is_empty() => println!("{}", path.display()),
///     _ => println!("Unable to get your home dir!"),
/// }
/// ```
pub fn home_dir() -> Option<PathBuf> {
    env::home_dir_with_env(&env::OS_ENV)
}

/// Returns the storage directory used by Cargo, often known as
/// `.cargo` or `CARGO_HOME`.
///
/// It returns one of the following values, in this order of
/// preference:
///
/// - The value of the `CARGO_HOME` environment variable, if it is
///   an absolute path.
/// - The value of the current working directory joined with the value
///   of the `CARGO_HOME` environment variable, if `CARGO_HOME` is a
///   relative directory.
/// - The `.cargo` directory in the user's home directory, as reported
///   by the `home_dir` function.
///
/// # Errors
///
/// This function fails if it fails to retrieve the current directory,
/// or if the home directory cannot be determined.
///
/// # Examples
///
/// ```
/// match home::cargo_home() {
///     Ok(path) => println!("{}", path.display()),
///     Err(err) => eprintln!("Cannot get your cargo home dir: {:?}", err),
/// }
/// ```
pub fn cargo_home() -> io::Result<PathBuf> {
    env::cargo_home_with_env(&env::OS_ENV)
}

/// Returns the storage directory used by Cargo within `cwd`.
/// For more details, see [`cargo_home`].
pub fn cargo_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    env::cargo_home_with_cwd_env(&env::OS_ENV, cwd)
}

/// Returns the storage directory used by rustup, often known as
/// `.rustup` or `RUSTUP_HOME`.
///
/// It returns one of the following values, in this order of
/// preference:
///
/// - The value of the `RUSTUP_HOME` environment variable, if it is
///   an absolute path.
/// - The value of the current working directory joined with the value
///   of the `RUSTUP_HOME` environment variable, if `RUSTUP_HOME` is a
///   relative directory.
/// - The `.rustup` directory in the user's home directory, as reported
///   by the `home_dir` function.
///
/// # Errors
///
/// This function fails if it fails to retrieve the current directory,
/// or if the home directory cannot be determined.
///
/// # Examples
///
/// ```
/// match home::rustup_home() {
///     Ok(path) => println!("{}", path.display()),
///     Err(err) => eprintln!("Cannot get your rustup home dir: {:?}", err),
/// }
/// ```
pub fn rustup_home() -> io::Result<PathBuf> {
    env::rustup_home_with_env(&env::OS_ENV)
}

/// Returns the storage directory used by rustup within `cwd`.
/// For more details, see [`rustup_home`].
pub fn rustup_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    env::rustup_home_with_cwd_env(&env::OS_ENV, cwd)
}
