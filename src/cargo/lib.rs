// For various reasons, some idioms are still allow'ed, but we would like to
// test and enforce them.
#![warn(rust_2018_idioms)]
#![cfg_attr(test, deny(warnings))]
// Due to some of the default clippy lints being somewhat subjective and not
// necessarily an improvement, we prefer to not use them at this time.
#![allow(clippy::all)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::redundant_clone)]

use crate::core::shell::Verbosity::Verbose;
use crate::core::Shell;
use anyhow::Error;
use log::debug;
use std::fmt;

pub use crate::util::errors::{InternalError, VerboseError};
pub use crate::util::{indented_lines, CargoResult, CliError, CliResult, Config};

pub const CARGO_ENV: &str = "CARGO";

#[macro_use]
mod macros;

pub mod core;
pub mod ops;
pub mod sources;
pub mod util;

pub struct CommitInfo {
    pub short_commit_hash: String,
    pub commit_hash: String,
    pub commit_date: String,
}

pub struct CfgInfo {
    // Information about the Git repository we may have been built from.
    pub commit_info: Option<CommitInfo>,
    // The release channel we were built for.
    pub release_channel: String,
}

pub struct VersionInfo {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub pre_release: Option<String>,
    // Information that's only available when we were built with
    // configure/make, rather than Cargo itself.
    pub cfg_info: Option<CfgInfo>,
}

impl fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(channel) = self.cfg_info.as_ref().map(|ci| &ci.release_channel) {
            if channel != "stable" {
                write!(f, "-{}", channel)?;
                let empty = String::new();
                write!(f, "{}", self.pre_release.as_ref().unwrap_or(&empty))?;
            }
        };

        if let Some(ref cfg) = self.cfg_info {
            if let Some(ref ci) = cfg.commit_info {
                write!(f, " ({} {})", ci.short_commit_hash, ci.commit_date)?;
            }
        };
        Ok(())
    }
}

pub fn exit_with_error(err: CliError, shell: &mut Shell) -> ! {
    debug!("exit_with_error; err={:?}", err);
    if let Some(ref err) = err.error {
        if let Some(clap_err) = err.downcast_ref::<clap::Error>() {
            clap_err.exit()
        }
    }

    let CliError { error, exit_code } = err;
    if let Some(error) = error {
        display_error(&error, shell);
    }

    std::process::exit(exit_code)
}

/// Displays an error, and all its causes, to stderr.
pub fn display_error(err: &Error, shell: &mut Shell) {
    debug!("display_error; err={:?}", err);
    _display_error(err, shell, true);
    if err
        .chain()
        .any(|e| e.downcast_ref::<InternalError>().is_some())
    {
        drop(shell.note("this is an unexpected cargo internal error"));
        drop(
            shell.note(
                "we would appreciate a bug report: https://github.com/rust-lang/cargo/issues/",
            ),
        );
        drop(shell.note(format!("cargo {}", version())));
        // Once backtraces are stabilized, this should print out a backtrace
        // if it is available.
    }
}

/// Displays a warning, with an error object providing detailed information
/// and context.
pub fn display_warning_with_error(warning: &str, err: &Error, shell: &mut Shell) {
    drop(shell.warn(warning));
    drop(writeln!(shell.err()));
    _display_error(err, shell, false);
}

fn _display_error(err: &Error, shell: &mut Shell, as_err: bool) -> bool {
    let verbosity = shell.verbosity();
    let is_verbose = |e: &(dyn std::error::Error + 'static)| -> bool {
        verbosity != Verbose && e.downcast_ref::<VerboseError>().is_some()
    };
    // Generally the top error shouldn't be verbose, but check it anyways.
    if is_verbose(err.as_ref()) {
        return true;
    }
    if as_err {
        drop(shell.error(&err));
    } else {
        drop(writeln!(shell.err(), "{}", err));
    }
    for cause in err.chain().skip(1) {
        // If we're not in verbose mode then print remaining errors until one
        // marked as `VerboseError` appears.
        if is_verbose(cause) {
            return true;
        }
        drop(writeln!(shell.err(), "\nCaused by:"));
        drop(write!(
            shell.err(),
            "{}",
            indented_lines(&cause.to_string())
        ));
    }
    false
}

pub fn version() -> VersionInfo {
    macro_rules! option_env_str {
        ($name:expr) => {
            option_env!($name).map(|s| s.to_string())
        };
    }

    // So this is pretty horrible...
    // There are two versions at play here:
    //   - version of cargo-the-binary, which you see when you type `cargo --version`
    //   - version of cargo-the-library, which you download from crates.io for use
    //     in your packages.
    //
    // We want to make the `binary` version the same as the corresponding Rust/rustc release.
    // At the same time, we want to keep the library version at `0.x`, because Cargo as
    // a library is (and probably will always be) unstable.
    //
    // Historically, Cargo used the same version number for both the binary and the library.
    // Specifically, rustc 1.x.z was paired with cargo 0.x+1.w.
    // We continue to use this scheme for the library, but transform it to 1.x.w for the purposes
    // of `cargo --version`.
    let major = 1;
    let minor = env!("CARGO_PKG_VERSION_MINOR").parse::<u8>().unwrap() - 1;
    let patch = env!("CARGO_PKG_VERSION_PATCH").parse::<u8>().unwrap();

    match option_env!("CFG_RELEASE_CHANNEL") {
        // We have environment variables set up from configure/make.
        Some(_) => {
            let commit_info = option_env!("CFG_COMMIT_HASH").map(|s| CommitInfo {
                commit_hash: s.to_string(),
                short_commit_hash: option_env_str!("CFG_SHORT_COMMIT_HASH").unwrap(),
                commit_date: option_env_str!("CFG_COMMIT_DATE").unwrap(),
            });
            VersionInfo {
                major,
                minor,
                patch,
                pre_release: option_env_str!("CARGO_PKG_VERSION_PRE"),
                cfg_info: Some(CfgInfo {
                    release_channel: option_env_str!("CFG_RELEASE_CHANNEL").unwrap(),
                    commit_info,
                }),
            }
        }
        // We are being compiled by Cargo itself.
        None => VersionInfo {
            major,
            minor,
            patch,
            pre_release: option_env_str!("CARGO_PKG_VERSION_PRE"),
            cfg_info: None,
        },
    }
}
