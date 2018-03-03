#![deny(unused)]
#![cfg_attr(test, deny(warnings))]
#![recursion_limit="128"]

// Currently, Cargo does not use clippy for its source code.
// But if someone runs it they should know that
// @alexcrichton disagree with clippy on some style things
#![cfg_attr(feature = "cargo-clippy", allow(explicit_iter_loop))]

#[macro_use] extern crate failure;
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
extern crate lazycell;
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
#[cfg(target_os = "macos")]
extern crate core_foundation;

use std::fmt;

use serde::de::DeserializeOwned;
use serde::ser;
use docopt::Docopt;
use failure::Error;

use core::Shell;
use core::shell::Verbosity::Verbose;

pub use util::{CargoError, CargoResult, CliError, CliResult, Config};
pub use util::errors::Internal;

pub const CARGO_ENV: &str = "CARGO";

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
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
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

pub fn call_main_without_stdin<Flags: DeserializeOwned>(
            exec: fn(Flags, &mut Config) -> CliResult,
            config: &mut Config,
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
        CliError::new(e.into(), code)
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
            println!("{}", error);
        }

        if !handle_cause(&error, shell) || hide {
            drop(writeln!(shell.err(), "\nTo learn more, run the command again \
                                        with --verbose."));
        }
    }

    std::process::exit(exit_code)
}

pub fn handle_error(err: CargoError, shell: &mut Shell) {
    debug!("handle_error; err={:?}", &err);

    let _ignored_result = shell.error(&err);
    handle_cause(&err, shell);
}

fn handle_cause(cargo_err: &Error, shell: &mut Shell) -> bool {
    fn print(error: String, shell: &mut Shell) {
        drop(writeln!(shell.err(), "\nCaused by:"));
        drop(writeln!(shell.err(), "  {}", error));
    }

    let verbose = shell.verbosity();

    if verbose == Verbose {
        // The first error has already been printed to the shell
        // Print all remaining errors
        for err in cargo_err.causes().skip(1) {
            print(err.to_string(), shell);
        }
    } else {
        // The first error has already been printed to the shell
        // Print remaining errors until one marked as Internal appears
        for err in cargo_err.causes().skip(1) {
            if err.downcast_ref::<Internal>().is_some() {
                return false;
            }

            print(err.to_string(), shell);
        }
    }

    true
}

pub fn version() -> VersionInfo {
    macro_rules! option_env_str {
        ($name:expr) => { option_env!($name).map(|s| s.to_string()) }
    }

    // So this is pretty horrible...
    // There are two versions at play here:
    //   - version of cargo-the-binary, which you see when you type `cargo --version`
    //   - version of cargo-the-library, which you download from crates.io for use
    //     in your projects.
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
            let commit_info =
                option_env!("CFG_COMMIT_HASH").map(|s| {
                    CommitInfo {
                        commit_hash: s.to_string(),
                        short_commit_hash: option_env_str!("CFG_SHORT_COMMIT_HASH").unwrap(),
                        commit_date: option_env_str!("CFG_COMMIT_DATE").unwrap(),
                    }
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
        },
        // We are being compiled by Cargo itself.
        None => {
            VersionInfo {
                major,
                minor,
                patch,
                pre_release: option_env_str!("CARGO_PKG_VERSION_PRE"),
                cfg_info: None,
            }
        }
    }
}
