#![deny(unused)]
#![cfg_attr(test, deny(warnings))]
#![recursion_limit="128"]

#[cfg(test)] extern crate hamcrest;
#[macro_use] extern crate error_chain;
#[macro_use] extern crate log;
#[macro_use] extern crate scoped_tls;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
extern crate atty;
extern crate crates_io as registry;
extern crate crossbeam;
extern crate curl;
extern crate docopt;
extern crate filetime;
extern crate flate2;
extern crate fs2;
extern crate git2;
extern crate glob;
extern crate hex;
extern crate home;
extern crate ignore;
extern crate jobserver;
extern crate libc;
extern crate libgit2_sys;
extern crate num_cpus;
extern crate same_file;
extern crate semver;
extern crate serde;
extern crate serde_ignored;
extern crate shell_escape;
extern crate tar;
extern crate tempdir;
extern crate termcolor;
extern crate toml;
extern crate url;

use std::fmt;
use std::error::Error;

use error_chain::ChainedError;
use serde::Deserialize;
use serde::ser;
use docopt::Docopt;

use core::Shell;
use core::shell::Verbosity::Verbose;

pub use util::{CargoError, CargoErrorKind, CargoResult, CliError, CliResult, Config};

pub const CARGO_ENV: &'static str = "CARGO";

macro_rules! bail {
    ($($fmt:tt)*) => (
        return Err(::util::errors::CargoError::from(format_args!($($fmt)*).to_string()))
    )
}

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
    // Information about the git repository we may have been built from.
    pub commit_info: Option<CommitInfo>,
    // The release channel we were built for.
    pub release_channel: String,
}

pub struct VersionInfo {
    pub major: String,
    pub minor: String,
    pub patch: String,
    pub pre_release: Option<String>,
    // Information that's only available when we were built with
    // configure/make, rather than cargo itself.
    pub cfg_info: Option<CfgInfo>,
}

impl fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "cargo {}.{}.{}",
               self.major, self.minor, self.patch)?;
        if let Some(channel) = self.cfg_info.as_ref().map(|ci| &ci.release_channel) {
            if channel != "stable" {
                write!(f, "-{}", channel)?;
                let empty = String::from("");
                write!(f, "{}", self.pre_release.as_ref().unwrap_or(&empty))?;
            }
        };

        if let Some(ref cfg) = self.cfg_info {
            if let Some(ref ci) = cfg.commit_info {
                write!(f, " ({} {})",
                       ci.short_commit_hash, ci.commit_date)?;
            }
        };
        Ok(())
    }
}

pub fn call_main_without_stdin<'de, Flags: Deserialize<'de>>(
            exec: fn(Flags, &Config) -> CliResult,
            config: &Config,
            usage: &str,
            args: &[String],
            options_first: bool) -> CliResult
{
    let docopt = Docopt::new(usage).unwrap()
        .options_first(options_first)
        .argv(args.iter().map(|s| &s[..]))
        .help(true);

    let flags = docopt.deserialize().map_err(|e| {
        let code = if e.fatal() {1} else {0};
        CliError::new(e.to_string().into(), code)
    })?;

    exec(flags, config)
}

pub fn print_json<T: ser::Serialize>(obj: &T) {
    let encoded = serde_json::to_string(&obj).unwrap();
    println!("{}", encoded);
}

pub fn exit_with_error(err: CliError, shell: &mut Shell) -> ! {
    debug!("exit_with_error; err={:?}", err);

    let CliError { error, exit_code, unknown } = err;
    // exit_code == 0 is non-fatal error, e.g. docopt version info
    let fatal = exit_code != 0;

    let hide = unknown && shell.verbosity() != Verbose;

    if let Some(error) = error {
        if hide {
            drop(shell.error("An unknown error occurred"))
        } else if fatal {
            drop(shell.error(&error))
        } else {
            drop(writeln!(shell.err(), "{}", error))
        }

        if !handle_cause(error, shell) || hide {
            drop(writeln!(shell.err(), "\nTo learn more, run the command again \
                                        with --verbose."));
        }
    }

    std::process::exit(exit_code)
}

pub fn handle_error(err: CargoError, shell: &mut Shell) {
    debug!("handle_error; err={:?}", &err);

    let _ignored_result = shell.error(&err);
    handle_cause(err, shell);
}

fn handle_cause<E, EKind>(cargo_err: E, shell: &mut Shell) -> bool
    where E: ChainedError<ErrorKind=EKind> + 'static
{
    fn print(error: String, shell: &mut Shell) {
        drop(writeln!(shell.err(), "\nCaused by:"));
        drop(writeln!(shell.err(), "  {}", error));
    }

    //Error inspection in non-verbose mode requires inspecting the
    //error kind to avoid printing Internal errors. The downcasting
    //machinery requires &(Error + 'static), but the iterator (and
    //underlying `cause`) return &Error. Because the borrows are
    //constrained to this handling method, and because the original
    //error object is constrained to be 'static, we're casting away
    //the borrow's actual lifetime for purposes of downcasting and
    //inspecting the error chain
    unsafe fn extend_lifetime(r: &Error) -> &(Error + 'static) {
        std::mem::transmute::<&Error, &Error>(r)
    }

    let verbose = shell.verbosity();

    if verbose == Verbose {
        //The first error has already been printed to the shell
        //Print all remaining errors
        for err in cargo_err.iter().skip(1) {
            print(err.to_string(), shell);
        }
    } else {
        //The first error has already been printed to the shell
        //Print remaining errors until one marked as Internal appears
        for err in cargo_err.iter().skip(1) {
            let err = unsafe { extend_lifetime(err) };
            if let Some(&CargoError(CargoErrorKind::Internal(..), ..)) =
                err.downcast_ref::<CargoError>() {
                return false;
            }

            print(err.to_string(), shell);
        }
    }

    true
}


pub fn version() -> VersionInfo {
    macro_rules! env_str {
        ($name:expr) => { env!($name).to_string() }
    }
    macro_rules! option_env_str {
        ($name:expr) => { option_env!($name).map(|s| s.to_string()) }
    }
    match option_env!("CFG_RELEASE_CHANNEL") {
        // We have environment variables set up from configure/make.
        Some(_) => {
            let commit_info =
                option_env!("CFG_COMMIT_HASH").map(|s| {
                    CommitInfo {
                        commit_hash: s.to_string(),
                        short_commit_hash: option_env_str!("CFG_SHORT_COMMIT_HASH").unwrap(),
                        commit_date: option_env_str!("CFG_COMMIT_DATE").unwrap(),
                    }
                });
            VersionInfo {
                major: env_str!("CARGO_PKG_VERSION_MAJOR"),
                minor: env_str!("CARGO_PKG_VERSION_MINOR"),
                patch: env_str!("CARGO_PKG_VERSION_PATCH"),
                pre_release: option_env_str!("CARGO_PKG_VERSION_PRE"),
                cfg_info: Some(CfgInfo {
                    release_channel: option_env_str!("CFG_RELEASE_CHANNEL").unwrap(),
                    commit_info: commit_info,
                }),
            }
        },
        // We are being compiled by Cargo itself.
        None => {
            VersionInfo {
                major: env_str!("CARGO_PKG_VERSION_MAJOR"),
                minor: env_str!("CARGO_PKG_VERSION_MINOR"),
                patch: env_str!("CARGO_PKG_VERSION_PATCH"),
                pre_release: option_env_str!("CARGO_PKG_VERSION_PRE"),
                cfg_info: None,
            }
        }
    }
}
