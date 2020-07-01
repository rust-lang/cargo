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
//! See also this [discussion].
//!
//! [discussion]: https://github.com/rust-lang/rust/pull/46799#issuecomment-361156935

#![doc(html_root_url = "https://docs.rs/home/0.5.3")]
#![deny(rust_2018_idioms)]

#[cfg(windows)]
mod windows;

use std::env;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Permits parameterising the home functions via the _from variants - used for
/// in-process unit testing by rustup.
pub trait Env {
    fn home_dir(&self) -> Option<PathBuf>;
    fn current_dir(&self) -> io::Result<PathBuf>;
    fn var_os(&self, key: &str) -> Option<OsString>;
}

/// Implements Env for the OS context, both Unix style and Windows.
///
/// This is trait permits in-process testing by providing a control point to
/// allow in-process divergence on what is normally process wide state.
///
/// Implementations should be provided by whatever testing framework the caller
/// is using. Code that is not performing in-process threaded testing requiring
/// isolated rustup/cargo directories does not need this trait or the _from
/// functions.
pub struct OsEnv {}
impl Env for OsEnv {
    fn home_dir(&self) -> Option<PathBuf> {
        home_dir_inner()
    }
    fn current_dir(&self) -> io::Result<PathBuf> {
        env::current_dir()
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        env::var_os(key)
    }
}

pub static OS_ENV: OsEnv = OsEnv {};

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
/// [`SHGetFolderPathW`][msdn] is used to return the appropriate path.
///
/// [msdn]: https://docs.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetfolderpathw
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
    home_dir_from(&OS_ENV)
}

/// Variant of home_dir where the environment source is parameterised. This is
/// specifically to support in-process testing scenarios as environment
/// variables and user home metadata are normally process global state. See the
/// OsEnv trait.
pub fn home_dir_from(env: &dyn Env) -> Option<PathBuf> {
    env.home_dir()
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
    cargo_home_from(&OS_ENV)
}

/// Variant of cargo_home where the environment source is parameterised. This is
/// specifically to support in-process testing scenarios as environment
/// variables and user home metadata are normally process global state. See the
/// OsEnv trait.
pub fn cargo_home_from(env: &dyn Env) -> io::Result<PathBuf> {
    let cwd = env.current_dir()?;
    cargo_home_with_cwd_from(env, &cwd)
}

/// Returns the storage directory used by Cargo within `cwd`.
/// For more details, see [`cargo_home`](fn.cargo_home.html).
pub fn cargo_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    cargo_home_with_cwd_from(&OS_ENV, cwd)
}

/// Variant of cargo_home_with_cwd where the environment source is
/// parameterised. This is specifically to support in-process testing scenarios
/// as environment variables and user home metadata are normally process global
/// state. See the OsEnv trait.
pub fn cargo_home_with_cwd_from(env: &dyn Env, cwd: &Path) -> io::Result<PathBuf> {
    match env.var_os("CARGO_HOME").filter(|h| !h.is_empty()) {
        Some(home) => {
            let home = PathBuf::from(home);
            if home.is_absolute() {
                Ok(home)
            } else {
                Ok(cwd.join(&home))
            }
        }
        _ => home_dir_from(env)
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
    rustup_home_from(&OS_ENV)
}

/// Variant of cargo_home_with_cwd where the environment source is
/// parameterised. This is specifically to support in-process testing scenarios
/// as environment variables and user home metadata are normally process global
/// state. See the OsEnv trait.
pub fn rustup_home_from(env: &dyn Env) -> io::Result<PathBuf> {
    let cwd = env.current_dir()?;
    rustup_home_with_cwd_from(env, &cwd)
}

/// Returns the storage directory used by rustup within `cwd`.
/// For more details, see [`rustup_home`](fn.rustup_home.html).
pub fn rustup_home_with_cwd(cwd: &Path) -> io::Result<PathBuf> {
    rustup_home_with_cwd_from(&OS_ENV, cwd)
}

/// Variant of cargo_home_with_cwd where the environment source is
/// parameterised. This is specifically to support in-process testing scenarios
/// as environment variables and user home metadata are normally process global
/// state. See the OsEnv trait.
pub fn rustup_home_with_cwd_from(env: &dyn Env, cwd: &Path) -> io::Result<PathBuf> {
    match env.var_os("RUSTUP_HOME").filter(|h| !h.is_empty()) {
        Some(home) => {
            let home = PathBuf::from(home);
            if home.is_absolute() {
                Ok(home)
            } else {
                Ok(cwd.join(&home))
            }
        }
        _ => home_dir_from(env)
            .map(|d| d.join(".rustup"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "could not find rustup home dir")),
    }
}
