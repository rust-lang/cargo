#![deny(unused)]
#![cfg_attr(test, deny(warnings))]

#[cfg(test)] extern crate hamcrest;
#[macro_use] extern crate log;
extern crate crates_io as registry;
extern crate crossbeam;
extern crate curl;
extern crate docopt;
extern crate filetime;
extern crate flate2;
extern crate fs2;
extern crate git2;
extern crate glob;
extern crate libc;
extern crate libgit2_sys;
extern crate num_cpus;
extern crate regex;
extern crate rustc_serialize;
extern crate semver;
extern crate tar;
extern crate tempdir;
extern crate term;
extern crate time;
extern crate toml;
extern crate url;

use std::env;
use std::error::Error;
use std::io::prelude::*;
use std::io;
use rustc_serialize::{Decodable, Encodable};
use rustc_serialize::json::{self, Json};
use docopt::Docopt;

use core::{Shell, MultiShell, ShellConfig, Verbosity, ColorConfig};
use core::shell::Verbosity::{Verbose};
use core::shell::ColorConfig::{Auto};
use term::color::{BLACK, RED};

pub use util::{CargoError, CliError, CliResult, human, Config, ChainError};

macro_rules! bail {
    ($($fmt:tt)*) => (
        return Err(::util::human(&format_args!($($fmt)*)))
    )
}

pub mod core;
pub mod ops;
pub mod sources;
pub mod util;

pub fn execute_main<T, U, V>(
                        exec: fn(T, U, &Config) -> CliResult<Option<V>>,
                        options_first: bool,
                        usage: &str)
    where V: Encodable, T: Decodable, U: Decodable
{
    process::<V, _>(|rest, shell| {
        call_main(exec, shell, usage, rest, options_first)
    });
}

pub fn call_main<T, U, V>(
            exec: fn(T, U, &Config) -> CliResult<Option<V>>,
            shell: &Config,
            usage: &str,
            args: &[String],
            options_first: bool) -> CliResult<Option<V>>
    where V: Encodable, T: Decodable, U: Decodable
{
    let flags = try!(flags_from_args::<T>(usage, args, options_first));
    let json = try!(json_from_stdin::<U>());

    exec(flags, json, shell)
}

pub fn execute_main_without_stdin<T, V>(
                                      exec: fn(T, &Config) -> CliResult<Option<V>>,
                                      options_first: bool,
                                      usage: &str)
    where V: Encodable, T: Decodable
{
    process::<V, _>(|rest, shell| {
        call_main_without_stdin(exec, shell, usage, rest, options_first)
    });
}

pub fn call_main_without_stdin<T, V>(
            exec: fn(T, &Config) -> CliResult<Option<V>>,
            shell: &Config,
            usage: &str,
            args: &[String],
            options_first: bool) -> CliResult<Option<V>>
    where V: Encodable, T: Decodable
{
    let flags = try!(flags_from_args::<T>(usage, args, options_first));
    exec(flags, shell)
}

fn process<V, F>(mut callback: F)
    where F: FnMut(&[String], &Config) -> CliResult<Option<V>>,
          V: Encodable
{
    let mut config = None;
    let result = (|| {
        config = Some(try!(Config::default()));
        let args: Vec<_> = try!(env::args_os().map(|s| {
            s.into_string().map_err(|s| {
                human(format!("invalid unicode in argument: {:?}", s))
            })
        }).collect());
        callback(&args, config.as_ref().unwrap())
    })();
    let mut verbose_shell = shell(Verbose, Auto);
    let mut shell = config.as_ref().map(|s| s.shell());
    let shell = shell.as_mut().map(|s| &mut **s).unwrap_or(&mut verbose_shell);
    process_executed(result, shell)
}

pub fn process_executed<T>(result: CliResult<Option<T>>, shell: &mut MultiShell)
    where T: Encodable
{
    match result {
        Err(e) => handle_error(e, shell),
        Ok(Some(encodable)) => {
            let encoded = json::encode(&encodable).unwrap();
            println!("{}", encoded);
        }
        Ok(None) => {}
    }
}

pub fn shell(verbosity: Verbosity, color_config: ColorConfig) -> MultiShell {
    enum Output {
        Stdout,
        Stderr,
    }

    let tty = isatty(Output::Stderr);
    let stderr = Box::new(io::stderr());

    let config = ShellConfig { color_config: color_config, tty: tty };
    let err = Shell::create(stderr, config);

    let tty = isatty(Output::Stdout);
    let stdout = Box::new(io::stdout());

    let config = ShellConfig { color_config: color_config, tty: tty };
    let out = Shell::create(stdout, config);

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

// `output` print variant error strings to either stderr or stdout.
// For fatal errors, print to stderr;
// and for others, e.g. docopt version info, print to stdout.
fn output(err: String, shell: &mut MultiShell, fatal: bool) {
    let (std_shell, color, message) = if fatal {
        (shell.err(), RED, Some("error:"))
    } else {
        (shell.out(), BLACK, None)
    };
    let _ = match message{
        Some(text) => std_shell.say_status(text, err.to_string(), color, false),
        None => std_shell.say(err, color)
    };
}

pub fn handle_error(err: CliError, shell: &mut MultiShell) {
    debug!("handle_error; err={:?}", err);

    let CliError { error, exit_code, unknown } = err;
    let fatal = exit_code != 0; // exit_code == 0 is non-fatal error

    let hide = unknown && shell.get_verbose() != Verbose;
    if hide {
        let _ = shell.err().say_status("error:", "An unknown error occurred",
                                       RED, false);
    } else {
        output(error.to_string(), shell, fatal);
    }
    if !handle_cause(&error, shell) || hide {
        let _ = shell.err().say("\nTo learn more, run the command again \
                                 with --verbose.".to_string(), BLACK);
    }

    std::process::exit(exit_code);
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

pub fn version() -> String {
    format!("cargo {}", match option_env!("CFG_VERSION") {
        Some(s) => s.to_string(),
        None => format!("{}.{}.{}{}",
                        env!("CARGO_PKG_VERSION_MAJOR"),
                        env!("CARGO_PKG_VERSION_MINOR"),
                        env!("CARGO_PKG_VERSION_PATCH"),
                        option_env!("CARGO_PKG_VERSION_PRE").unwrap_or(""))
    })
}

fn flags_from_args<T>(usage: &str, args: &[String],
                          options_first: bool) -> CliResult<T>
    where T: Decodable
{
    let docopt = Docopt::new(usage).unwrap()
                                   .options_first(options_first)
                                   .argv(args.iter().map(|s| &s[..]))
                                   .help(true);
    docopt.decode().map_err(|e| {
        let code = if e.fatal() {1} else {0};
        CliError::from_error(human(e.to_string()), code)
    })
}

fn json_from_stdin<T: Decodable>() -> CliResult<T> {
    let mut reader = io::stdin();
    let mut input = String::new();
    try!(reader.read_to_string(&mut input).map_err(|_| {
        CliError::new("Standard in did not exist or was not UTF-8", 1)
    }));

    let json = try!(Json::from_str(&input).map_err(|_| {
        CliError::new("Could not parse standard in as JSON", 1)
    }));
    let mut decoder = json::Decoder::new(json);

    Decodable::decode(&mut decoder).map_err(|_| {
        CliError::new("Could not process standard in as input", 1)
    })
}
