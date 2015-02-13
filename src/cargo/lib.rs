#![deny(unused)]
#![feature(collections, hash, io, libc, os, path, std_misc, unicode, env, core)]
#![cfg_attr(test, deny(warnings))]

extern crate libc;
extern crate "rustc-serialize" as rustc_serialize;
extern crate regex;
extern crate term;
extern crate time;
#[macro_use] extern crate log;

extern crate curl;
extern crate docopt;
extern crate flate2;
extern crate git2;
extern crate glob;
extern crate semver;
extern crate tar;
extern crate toml;
extern crate url;
#[cfg(test)] extern crate hamcrest;

extern crate registry;

use std::env;
use std::error::Error;
use std::old_io::stdio::{stdout_raw, stderr_raw};
use std::old_io::{self, stdout, stderr};
use rustc_serialize::{Decodable, Encodable};
use rustc_serialize::json::{self, Json};
use docopt::Docopt;

use core::{Shell, MultiShell, ShellConfig};
use term::color::{BLACK, RED};

pub use util::{CargoError, CliError, CliResult, human, Config, ChainError};

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
    let mut shell = shell(true);
    process_executed((|| {
        let config = try!(Config::new(&mut shell));
        let args: Vec<_> = try!(env::args_os().map(|s| {
            s.into_string().map_err(|s| {
                human(format!("invalid unicode in argument: {:?}", s))
            })
        }).collect());
        callback(&args, &config)
    })(), &mut shell)
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
        _ => {}
    }
}

pub fn shell(verbose: bool) -> MultiShell {
    let tty = stderr_raw().isatty();
    let stderr = Box::new(stderr()) as Box<Writer + Send>;

    let config = ShellConfig { color: true, verbose: verbose, tty: tty };
    let err = Shell::create(stderr, config);

    let tty = stdout_raw().isatty();
    let stdout = Box::new(stdout()) as Box<Writer + Send>;

    let config = ShellConfig { color: true, verbose: verbose, tty: tty };
    let out = Shell::create(stdout, config);

    MultiShell::new(out, err, verbose)
}


// `output` print variant error strings to either stderr or stdout.
// For fatal errors, print to stderr;
// and for others, e.g. docopt version info, print to stdout.
fn output(err: String, shell: &mut MultiShell, fatal: bool) {
    let std_shell = if fatal {shell.err()} else {shell.out()};
    let color = if fatal {RED} else {BLACK};
    let _ = std_shell.say(err, color);
}

pub fn handle_error(err: CliError, shell: &mut MultiShell) {
    debug!("handle_error; err={:?}", err);

    let CliError { error, exit_code, unknown } = err;
    let fatal = exit_code != 0; // exit_code == 0 is non-fatal error


    let hide = unknown && !shell.get_verbose();
    if hide {
        let _ = shell.err().say("An unknown error occurred", RED);
    } else {
        output(error.to_string(), shell, fatal);
    }
    if !handle_cause(&error, shell) || hide {
        let _ = shell.err().say("\nTo learn more, run the command again \
                                 with --verbose.".to_string(), BLACK);
    }

    std::env::set_exit_status(exit_code);
}

fn handle_cause(mut cargo_err: &CargoError, shell: &mut MultiShell) -> bool {
    let verbose = shell.get_verbose();
    let mut err;
    loop {
        cargo_err = match cargo_err.cargo_cause() {
            Some(cause) => cause,
            None => { err = cargo_err.cause(); break }
        };
        if !verbose && !cargo_err.is_human() { return false }
        print(cargo_err.to_string(), shell);
    }
    loop {
        let cause = match err { Some(err) => err, None => return true };
        if !verbose { return false }
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

fn flags_from_args<'a, T>(usage: &str, args: &[String],
                          options_first: bool) -> CliResult<T>
    where T: Decodable
{
    let docopt = Docopt::new(usage).unwrap()
                                   .options_first(options_first)
                                   .argv(args.iter().map(|s| s.as_slice()))
                                   .help(true)
                                   .version(Some(version()));
    docopt.decode().map_err(|e| {
        let code = if e.fatal() {1} else {0};
        let desc = match e {
            docopt::Error::WithProgramUsage(_, s) => s,
            ref e if e.fatal() => e.description().to_string(),
            e => e.to_string(),
        };
        CliError::from_error(human(desc), code)
    })
}

fn json_from_stdin<T: Decodable>() -> CliResult<T> {
    let mut reader = old_io::stdin();
    let input = try!(reader.read_to_string().map_err(|_| {
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
