#![crate_name="cargo"]
#![crate_type="rlib"]

#![feature(macro_rules, phase, default_type_params, unboxed_closures)]
#![feature(slicing_syntax)]
#![deny(unused)]
#![cfg_attr(test, deny(warnings))]

extern crate libc;
extern crate "rustc-serialize" as rustc_serialize;
extern crate regex;
extern crate term;
extern crate time;
#[phase(plugin, link)] extern crate log;

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

use std::os;
use std::error::Error;
use std::io::stdio::{stdout_raw, stderr_raw};
use std::io::{mod, stdout, stderr};
use rustc_serialize::{Decoder, Encoder, Decodable, Encodable};
use rustc_serialize::json;
use docopt::Docopt;

use core::{Shell, MultiShell, ShellConfig};
use term::color::{BLACK, RED};

pub use util::{CargoError, CliError, CliResult, human};

macro_rules! some {
    ($e:expr) => (
        match $e {
            Some(e) => e,
            None => return None
        }
    )
}

// Added so that the try! macro below can refer to cargo::util, while
// other external importers of this macro can use it as well.
//
// "Hygiene strikes again" - @acrichton
mod cargo {
    pub use super::util;
}

pub mod core;
pub mod ops;
pub mod sources;
pub mod util;

pub trait RepresentsJSON : Decodable<json::Decoder, json::DecoderError> {}
impl<T: Decodable<json::Decoder, json::DecoderError>> RepresentsJSON for T {}

pub fn execute_main<'a,
                    T: Decodable<docopt::Decoder, docopt::Error>,
                    U: RepresentsJSON,
                    V: Encodable<json::Encoder<'a>, io::IoError>>(
                        exec: fn(T, U, &mut MultiShell) -> CliResult<Option<V>>,
                        options_first: bool,
                        usage: &str) {
    process::<V>(|rest, shell| call_main(exec, shell, usage, rest, options_first));
}

pub fn call_main<'a,
        T: Decodable<docopt::Decoder, docopt::Error>,
        U: RepresentsJSON,
        V: Encodable<json::Encoder<'a>, io::IoError>>(
            exec: fn(T, U, &mut MultiShell) -> CliResult<Option<V>>,
            shell: &mut MultiShell,
            usage: &str,
            args: &[String],
            options_first: bool) -> CliResult<Option<V>> {
    let flags = try!(flags_from_args::<T>(usage, args, options_first));
    let json = try!(json_from_stdin::<U>());

    exec(flags, json, shell)
}

pub fn execute_main_without_stdin<'a,
                                  T: Decodable<docopt::Decoder, docopt::Error>,
                                  V: Encodable<json::Encoder<'a>, io::IoError>>(
                                      exec: fn(T, &mut MultiShell) -> CliResult<Option<V>>,
                                      options_first: bool,
                                      usage: &str) {
    process::<V>(|rest, shell| call_main_without_stdin(exec, shell, usage, rest,
                                                       options_first));
}

pub fn execute_main_with_args_and_without_stdin<'a,
                                  T: Decodable<docopt::Decoder, docopt::Error>,
                                  V: Encodable<json::Encoder<'a>, io::IoError>>(
                                      exec: fn(T, &mut MultiShell) -> CliResult<Option<V>>,
                                      options_first: bool,
                                      usage: &str,
                                      args: &[String]) {
    let mut shell = shell(true);

    process_executed(
        call_main_without_stdin(exec, &mut shell, usage, args, options_first),
        &mut shell)
}

pub fn call_main_without_stdin<'a,
                               T: Decodable<docopt::Decoder, docopt::Error>,
                               V: Encodable<json::Encoder<'a>, io::IoError>>(
            exec: fn(T, &mut MultiShell) -> CliResult<Option<V>>,
            shell: &mut MultiShell,
            usage: &str,
            args: &[String],
            options_first: bool) -> CliResult<Option<V>> {
    let flags = try!(flags_from_args::<T>(usage, args, options_first));
    exec(flags, shell)
}

fn process<'a, V: Encodable<json::Encoder<'a>, io::IoError>>(
               callback: |&[String], &mut MultiShell| -> CliResult<Option<V>>) {
    let mut shell = shell(true);
    process_executed(callback(os::args().as_slice(), &mut shell), &mut shell)
}

pub fn process_executed<'a,
                        T: Encodable<json::Encoder<'a>, io::IoError>>(
                            result: CliResult<Option<T>>,
                            shell: &mut MultiShell) {
    match result {
        Err(e) => handle_error(e, shell),
        Ok(Some(encodable)) => {
            let encoded = json::encode(&encodable);
            println!("{}", encoded);
        }
        _ => {}
    }
}

pub fn shell(verbose: bool) -> MultiShell {
    let tty = stderr_raw().isatty();
    let stderr = box stderr() as Box<Writer + Send>;

    let config = ShellConfig { color: true, verbose: verbose, tty: tty };
    let err = Shell::create(stderr, config);

    let tty = stdout_raw().isatty();
    let stdout = box stdout() as Box<Writer + Send>;

    let config = ShellConfig { color: true, verbose: verbose, tty: tty };
    let out = Shell::create(stdout, config);

    MultiShell::new(out, err, verbose)
}


// `output` print variant error strings to either stderr or stdout.
// For fatal errors, print to stderr;
// and for others, e.g. docopt version info, print to stdout.
fn output(caption: Option<String>, detail: Option<String>,
          shell: &mut MultiShell, fatal: bool) {
    let std_shell = if fatal {shell.err()} else {shell.out()};
    if let Some(caption) = caption {
        let color = if fatal {RED} else {BLACK};
        let _ = std_shell.say(caption, color);
    }
    if let Some(detail) = detail {
        let _ = std_shell.say(detail, BLACK); // always black
    }
}

pub fn handle_error(err: CliError, shell: &mut MultiShell) {
    log!(4, "handle_error; err={}", err);

    let CliError { error, exit_code, unknown } = err;
    let verbose = shell.get_verbose();
    let fatal = exit_code != 0; // exit_code == 0 is non-fatal error

    if unknown {
        output(Some("An unknown error occurred".to_string()), None, shell, fatal);
    } else if error.to_string().len() > 0 {
        output(Some(error.to_string()), None, shell, fatal);
    }

    if error.cause().is_some() || unknown {
        if !verbose {
            output(None,
                   Some("\nTo learn more, run the command again with --verbose.".to_string()),
                   shell, fatal);
        }
    }

    if verbose {
        if unknown {
            output(Some(error.to_string()), None, shell, fatal);
        }
        if let Some(detail) = error.detail() {
            output(None, Some(detail), shell, fatal);
        }
        if let Some(err) = error.cause() {
            let _ = handle_cause(err, shell);
        }
    }

    std::os::set_exit_status(exit_code as int);
}

fn handle_cause(mut err: &Error, shell: &mut MultiShell) {
    loop {
        let _ = shell.err().say("\nCaused by:", BLACK);
        let _ = shell.err().say(format!("  {}", err.description()), BLACK);

        match err.cause() {
            Some(e) => err = e,
            None => break,
        }
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
                          where T: Decodable<docopt::Decoder, docopt::Error> {
    struct CargoDocoptError { err: docopt::Error }
    impl Error for CargoDocoptError {
        fn description(&self) -> &str {
            match self.err {
                docopt::Error::WithProgramUsage(..) => "",
                ref e if e.fatal() => self.err.description(),
                _ => "",
            }
        }

        fn detail(&self) -> Option<String> {
            match self.err {
                docopt::Error::WithProgramUsage(_, ref usage) => {
                    Some(usage.clone())
                }
                ref e if e.fatal() => None,
                ref e => Some(e.to_string())
            }
        }
    }
    impl CargoError for CargoDocoptError {
        fn is_human(&self) -> bool { true }
    }

    let docopt = Docopt::new(usage).unwrap()
                                   .options_first(options_first)
                                   .argv(args.iter().map(|s| s.as_slice()))
                                   .help(true)
                                   .version(Some(version()));
    docopt.decode().map_err(|e| {
        let code = if e.fatal() {1} else {0};
        CliError::from_error(CargoDocoptError { err: e }, code)
    })
}

fn json_from_stdin<T: RepresentsJSON>() -> CliResult<T> {
    let mut reader = io::stdin();
    let input = try!(reader.read_to_string().map_err(|_| {
        CliError::new("Standard in did not exist or was not UTF-8", 1)
    }));

    let json = try!(json::from_str(input.as_slice()).map_err(|_| {
        CliError::new("Could not parse standard in as JSON", 1)
    }));
    let mut decoder = json::Decoder::new(json);

    Decodable::decode(&mut decoder).map_err(|_| {
        CliError::new("Could not process standard in as input", 1)
    })
}
