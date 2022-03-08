// For various reasons, some idioms are still allow'ed, but we would like to
// test and enforce them.
#![warn(rust_2018_idioms)]
#![cfg_attr(test, deny(warnings))]
// Due to some of the default clippy lints being somewhat subjective and not
// necessarily an improvement, we prefer to not use them at this time.
#![allow(clippy::all)]

use crate::core::shell::Verbosity::Verbose;
use crate::core::Shell;
use anyhow::Error;
use log::debug;

pub use crate::util::errors::{InternalError, VerboseError};
pub use crate::util::{indented_lines, CargoResult, CliError, CliResult, Config};
pub use crate::version::version;

pub const CARGO_ENV: &str = "CARGO";

#[macro_use]
mod macros;

pub mod core;
pub mod ops;
pub mod sources;
pub mod util;
mod version;

pub fn exit_with_error(err: CliError, shell: &mut Shell) -> ! {
    debug!("exit_with_error; err={:?}", err);

    if let Some(ref err) = err.error {
        if let Some(clap_err) = err.downcast_ref::<clap::Error>() {
            let exit_code = if clap_err.use_stderr() { 1 } else { 0 };
            let _ = clap_err.print();
            std::process::exit(exit_code)
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
