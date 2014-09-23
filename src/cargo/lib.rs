#![crate_name="cargo"]
#![crate_type="rlib"]

#![feature(macro_rules, phase)]
#![feature(default_type_params)]
#![deny(warnings)]

extern crate libc;
extern crate regex;
extern crate serialize;
extern crate term;
extern crate time;
#[phase(plugin)] extern crate regex_macros;
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

use std::os;
use std::io::stdio::{stdout_raw, stderr_raw};
use std::io::{mod, stdout, stderr};
use serialize::{Decoder, Encoder, Decodable, Encodable, json};
use docopt::FlagParser;

use core::{Shell, MultiShell, ShellConfig};
use term::color::{BLACK};

pub use util::{CargoError, CliError, CliResult, human};

macro_rules! some(
    ($e:expr) => (
        match $e {
            Some(e) => e,
            None => return None
        }
    )
)

// Added so that the try! macro below can refer to cargo::util, while
// other external importers of this macro can use it as well.
//
// "Hygiene strikes again" - @acrichton
mod cargo {
    pub use super::util;
}

#[macro_export]
macro_rules! try (
    ($expr:expr) => ({
        use cargo::util::FromError;
        match $expr.map_err(FromError::from_error) {
            Ok(val) => val,
            Err(err) => return Err(err)
        }
    })
)

macro_rules! raw_try (
    ($expr:expr) => ({
        match $expr {
            Ok(val) => val,
            Err(err) => return Err(err)
        }
    })
)

pub mod core;
pub mod ops;
pub mod sources;
pub mod util;

pub trait RepresentsJSON : Decodable<json::Decoder, json::DecoderError> {}
impl<T: Decodable<json::Decoder, json::DecoderError>> RepresentsJSON for T {}

pub fn execute_main<'a,
                    T: FlagParser,
                    U: RepresentsJSON,
                    V: Encodable<json::Encoder<'a>, io::IoError>>(
                        exec: fn(T, U, &mut MultiShell) -> CliResult<Option<V>>,
                        options_first: bool) {
    // see comments below
    off_the_main_thread(proc() {
        process::<V>(|rest, shell| call_main(exec, shell, rest, options_first));
    });
}

pub fn call_main<'a,
        T: FlagParser,
        U: RepresentsJSON,
        V: Encodable<json::Encoder<'a>, io::IoError>>(
            exec: fn(T, U, &mut MultiShell) -> CliResult<Option<V>>,
            shell: &mut MultiShell,
            args: &[String],
            options_first: bool) -> CliResult<Option<V>> {
    let flags = try!(flags_from_args::<T>(args, options_first));
    let json = try!(json_from_stdin::<U>());

    exec(flags, json, shell)
}

pub fn execute_main_without_stdin<'a,
                                  T: FlagParser,
                                  V: Encodable<json::Encoder<'a>, io::IoError>>(
                                      exec: fn(T, &mut MultiShell) -> CliResult<Option<V>>,
                                      options_first: bool) {
    // see comments below
    off_the_main_thread(proc() {
        process::<V>(|rest, shell| call_main_without_stdin(exec, shell, rest,
                                                           options_first));
    });
}

pub fn call_main_without_stdin<'a,
                               T: FlagParser,
                               V: Encodable<json::Encoder<'a>, io::IoError>>(
            exec: fn(T, &mut MultiShell) -> CliResult<Option<V>>,
            shell: &mut MultiShell,
            args: &[String],
            options_first: bool) -> CliResult<Option<V>> {
    let flags = try!(flags_from_args::<T>(args, options_first));
    exec(flags, shell)
}

fn process<'a, V: Encodable<json::Encoder<'a>, io::IoError>>(
               callback: |&[String], &mut MultiShell| -> CliResult<Option<V>>) {
    let mut shell = shell(true);
    let mut args = os::args();
    args.remove(0);
    process_executed(callback(args.as_slice(), &mut shell), &mut shell)
}

pub fn process_executed<'a,
                        T: Encodable<json::Encoder<'a>, io::IoError>>(
                            result: CliResult<Option<T>>,
                            shell: &mut MultiShell) {
    match result {
        Err(e) => handle_error(e, shell),
        Ok(encodable) => {
            encodable.map(|encodable| {
                let encoded = json::encode(&encodable);
                println!("{}", encoded);
            });
        }
    }
}

pub fn shell(verbose: bool) -> MultiShell<'static> {
    let tty = stderr_raw().isatty();
    let stderr = box stderr() as Box<Writer>;

    let config = ShellConfig { color: true, verbose: verbose, tty: tty };
    let err = Shell::create(stderr, config);

    let tty = stdout_raw().isatty();
    let stdout = box stdout() as Box<Writer>;

    let config = ShellConfig { color: true, verbose: verbose, tty: tty };
    let out = Shell::create(stdout, config);

    MultiShell::new(out, err, verbose)
}

pub fn handle_error(err: CliError, shell: &mut MultiShell) {
    log!(4, "handle_error; err={}", err);

    let CliError { error, exit_code, unknown } = err;

    if unknown {
        let _ = shell.error("An unknown error occurred");
    } else if error.to_string().len() > 0 {
        let _ = shell.error(error.to_string());
    }

    if error.cause().is_some() || unknown {
        let _ = shell.concise(|shell| {
            shell.err().say("\nTo learn more, run the command again with --verbose.", BLACK)
        });
    }

    let _ = shell.verbose(|shell| {
        if unknown {
            let _ = shell.error(error.to_string());
        }
        error.detail().map(|detail| {
            let _ = shell.err().say(format!("{}", detail), BLACK);
        });
        error.cause().map(|err| {
            let _ = handle_cause(err, shell);
        });
        Ok(())
      });

    std::os::set_exit_status(exit_code as int);
}

fn handle_cause(err: &CargoError, shell: &mut MultiShell) {
    let _ = shell.err().say("\nCaused by:", BLACK);
    let _ = shell.err().say(format!("  {}", err.description()), BLACK);

    err.cause().map(|e| handle_cause(e, shell));
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

fn flags_from_args<T: FlagParser>(args: &[String],
                                  options_first: bool) -> CliResult<T> {
    let args = args.iter().map(|a| a.as_slice()).collect::<Vec<&str>>();
    let config = docopt::Config {
        options_first: options_first,
        help: true,
        version: Some(version()),
    };
    FlagParser::parse_args(config, args.as_slice()).map_err(|e| {
        let code = if e.fatal() {1} else {0};
        CliError::from_error(e, code)
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

// Seems curious to run cargo off the main thread, right? Well do I have a story
// for you. Turns out rustdoc does a similar thing, and already has a good
// explanation [1] though, so I'll just point you over there.
//
// [1]: https://github.com/rust-lang/rust/blob/85fd37f/src/librustdoc/lib.rs#L92-L122
fn off_the_main_thread(p: proc():Send) {
    let (tx, rx) = channel();
    spawn(proc() { p(); tx.send(()); });
    if rx.recv_opt().is_err() {
        std::os::set_exit_status(std::rt::DEFAULT_ERROR_CODE);
    }
}
