//! Canonical definitions of `home_dir`, `cargo_home`, and `rustup_home`.
//!
//! This provides the definition of `home_dir` used by Cargo and
//! rustup, as well functions to find the correct value of
//! `CARGO_HOME` and `RUSTUP_HOME`.
//!
//! See also the [`dirs`](https://docs.rs/dirs) crate.
//!
//! _Note that as of 2019/08/06 it appears that cargo uses this crate. And
//! rustup has used this crate since 2019/08/21._
//!
//! The definition of `home_dir` provided by the standard library is
//! incorrect because it considers the `HOME` environment variable on
//! Windows. This causes surprising situations where a Rust program
//! will behave differently depending on whether it is run under a
//! Unix emulation environment like Cygwin or MinGW. Neither Cargo nor
//! rustup use the standard libraries definition - they use the
//! definition here.
//!
//! This crate further provides two functions, `cargo_home` and
//! `rustup_home`, which are the canonical way to determine the
//! location that Cargo and rustup store their data.
//!
//! See [rust-lang/rust#43321].
//!
//! [rust-lang/rust#43321]: https://github.com/rust-lang/rust/issues/43321

#![doc(html_root_url = "https://docs.rs/home/0.5.1")]
#![deny(rust_2018_idioms)]

mod windows;

use std::env;
use std::io;
use std::path::{Path, PathBuf};

/// Returns the path of the current user's home directory if known.
///
/// # Unix
///
/// Returns the value of the `HOME` environment variable if it is set
/// and not equal to the empty string. Otherwise, it tries to determine the
/// home directory by invoking the `getpwuid_r` function on the UID of the
/// current user.
///
/// # Windows
///
/// Returns the value of the `USERPROFILE` environment variable if it
/// is set and not equal to the empty string. If both do not exist,
/// [`GetUserProfileDirectory`][msdn] is used to return the
/// appropriate path.
///
/// [msdn]: https://msdn.microsoft.com/en-us/library/windows/desktop/bb762280(v=vs.85).aspx
///
/// # Examples
///
/// ```
/// match home::home_dir() {
///     Some(path) => println!("{}", path.display()),
///     None => println!("Impossible to get your home dir!"),
/// }
/// ```
pub fn home_dir() -> Option<PathBuf> {
    home_dir_inner()
}

#[cfg(windows)]
use windows::home_dir_inner;

#[cfg(any(unix, target_os = "redox"))]
fn home_dir_inner() -> Option<PathBuf> {
    #[allow(deprecated)]
    env::home_dir()
}

/// Returns the storage directory used by Cargo, often knowns as
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
    let cwd = env::current_dir()?;
    cargo_home_with_cwd(&cwd)
}

pub fn cargo_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    let cargo_home_env = match env::var_os("CARGO_HOME") {
        Some(p) => match p.as_os_str().to_str() {
            Some(q) if !q.trim().is_empty() => Some(p),
            _ => None,
        },
        _ => None,
    };

    match cargo_home_env {
        Some(home) => {
            let home = PathBuf::from(home);
            if home.is_absolute() {
                Ok(home)
            } else {
                Ok(cwd.join(&home))
            }
        }
        None => home_dir()
            .map(|p| p.join(".cargo"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "could not find cargo home dir")),
    }
}

/// Returns the storage directory used by rustup, often knowns as
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
/// As a matter of backwards compatibility, this function _may_ return
/// the `.multirust` directory in the user's home directory, only if
/// it determines that the user is running an old version of rustup
/// where that is necessary.
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
    let cwd = env::current_dir()?;
    rustup_home_with_cwd(&cwd)
}

pub fn rustup_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    match env::var_os("RUSTUP_HOME") {
        Some(home) => {
            let home = PathBuf::from(home);
            if home.is_absolute() {
                Ok(home)
            } else {
                Ok(cwd.join(&home))
            }
        }
        None => home_dir()
            .map(|d| d.join(".rustup"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "could not find rustup home dir")),
    }
}
