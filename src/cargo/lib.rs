#![deny(unused)]
#![cfg_attr(test, deny(warnings))]

#[cfg(test)] extern crate hamcrest;
#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
extern crate crates_io as registry;
extern crate crossbeam;
extern crate curl;
extern crate docopt;
extern crate filetime;
extern crate flate2;
extern crate fs2;
extern crate git2;
extern crate glob;
extern crate handlebars;
extern crate libc;
extern crate libgit2_sys;
extern crate num_cpus;
extern crate rustc_serialize;
extern crate semver;
extern crate serde;
extern crate serde_ignored;
extern crate shell_escape;
extern crate tar;
extern crate tempdir;
extern crate term;
extern crate toml;
extern crate url;

use std::io;
use std::fmt;
use rustc_serialize::Decodable;
use serde::ser;
use docopt::Docopt;

use core::{Shell, MultiShell, ShellConfig, Verbosity, ColorConfig};
use core::shell::Verbosity::{Verbose};
use term::color::{BLACK};

pub use util::{CargoError, CargoResult, CliError, CliResult, human, Config, ChainError};

pub const CARGO_ENV: &'static str = "CARGO";

macro_rules! bail {
    ($($fmt:tt)*) => (
        return Err(::util::human(&format_args!($($fmt)*)))
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
    // The date that the build was performed.
    pub build_date: String,
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
            match cfg.commit_info {
                Some(ref ci) => {
                    write!(f, " ({} {})",
                           ci.short_commit_hash, ci.commit_date)?;
                },
                None => {
                    write!(f, " (built {})",
                           cfg.build_date)?;
                }
            }
        };
        Ok(())
    }
}

pub fn call_main_without_stdin<Flags: Decodable>(
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

    let flags = docopt.decode().map_err(|e| {
        let code = if e.fatal() {1} else {0};
        CliError::new(human(e.to_string()), code)
    })?;

    exec(flags, config)
}

pub fn print_json<T: ser::Serialize>(obj: &T) {
    let encoded = serde_json::to_string(&obj).unwrap();
    println!("{}", encoded);
}

pub fn shell(verbosity: Verbosity, color_config: ColorConfig) -> MultiShell {
    enum Output {
        Stdout,
        Stderr,
    }

    let tty = isatty(Output::Stderr);

    let config = ShellConfig { color_config: color_config, tty: tty };
    let err = Shell::create(|| Box::new(io::stderr()), config);

    let tty = isatty(Output::Stdout);

    let config = ShellConfig { color_config: color_config, tty: tty };
    let out = Shell::create(|| Box::new(io::stdout()), config);

    return MultiShell::new(out, err, verbosity);

    #[cfg(unix)]
    fn isatty(output: Output) -> bool {
        let fd = match output {
            Output::Stdout => libc::STDOUT_FILENO,
            Output::Stderr => libc::STDERR_FILENO,
        };

        unsafe { libc::isatty(fd) != 0 }
    }
    #[cfg(windows)]
    fn isatty(output: Output) -> bool {
        extern crate kernel32;
        extern crate winapi;

        let handle = match output {
            Output::Stdout => winapi::winbase::STD_OUTPUT_HANDLE,
            Output::Stderr => winapi::winbase::STD_ERROR_HANDLE,
        };

        unsafe {
            let handle = kernel32::GetStdHandle(handle);
            let mut out = 0;
            kernel32::GetConsoleMode(handle, &mut out) != 0
        }
    }
}

pub fn exit_with_error(err: CliError, shell: &mut MultiShell) -> ! {
    debug!("exit_with_error; err={:?}", err);

    let CliError { error, exit_code, unknown } = err;
    // exit_code == 0 is non-fatal error, e.g. docopt version info
    let fatal = exit_code != 0;

    let hide = unknown && shell.get_verbose() != Verbose;

    if let Some(error) = error {
        let _ignored_result = if hide {
            shell.error("An unknown error occurred")
        } else if fatal {
            shell.error(&error)
        } else {
            shell.say(&error, BLACK)
        };

        if !handle_cause(&error, shell) || hide {
            let _ = shell.err().say("\nTo learn more, run the command again \
                                     with --verbose.".to_string(), BLACK);
        }
    }

    std::process::exit(exit_code)
}

pub fn handle_error(err: &CargoError, shell: &mut MultiShell) {
    debug!("handle_error; err={:?}", err);

    let _ignored_result = shell.error(err);
    handle_cause(err, shell);
}

fn handle_cause(mut cargo_err: &CargoError, shell: &mut MultiShell) -> bool {
    let verbose = shell.get_verbose();
    let mut err;
    loop {
        cargo_err = match cargo_err.cargo_cause() {
            Some(cause) => cause,
            None => { err = cargo_err.cause(); break }
        };
        if verbose != Verbose && !cargo_err.is_human() { return false }
        print(cargo_err.to_string(), shell);
    }
    loop {
        let cause = match err { Some(err) => err, None => return true };
        if verbose != Verbose { return false }
        print(cause.to_string(), shell);
        err = cause.cause();
    }

    fn print(error: String, shell: &mut MultiShell) {
        let _ = shell.err().say("\nCaused by:", BLACK);
        let _ = shell.err().say(format!("  {}", error), BLACK);
    }
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
                    build_date: option_env_str!("CFG_BUILD_DATE").unwrap(),
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
