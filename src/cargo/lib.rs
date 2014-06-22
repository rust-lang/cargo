#![crate_id="cargo"]
#![crate_type="rlib"]

#![feature(macro_rules,phase)]

extern crate debug;
extern crate term;
extern crate url;
extern crate serialize;
extern crate semver;
extern crate toml = "github.com/mneumann/rust-toml#toml";

#[phase(plugin, link)]
extern crate hammer;

#[phase(plugin, link)]
extern crate log;

#[cfg(test)]
extern crate hamcrest;

use serialize::{Decoder, Encoder, Decodable, Encodable, json};
use std::io;
use std::io::stderr;
use std::io::stdio::stderr_raw;
use hammer::{FlagDecoder, FlagConfig, UsageDecoder, HammerError};

use core::{Shell, ShellConfig};
use term::color::{RED, BLACK};

pub use util::{CargoError, CliError, CliResult, human};

macro_rules! some(
    ($e:expr) => (
        match $e {
            Some(e) => e,
            None => return None
        }
    )
)

macro_rules! try (
    ($expr:expr) => ({
        use util::CargoError;
        match $expr.map_err(|err| err.to_error()) {
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

trait FlagParse : FlagConfig {
    fn decode_flags(d: &mut FlagDecoder) -> Result<Self, HammerError>;
}

impl<T: FlagConfig + Decodable<FlagDecoder, HammerError>> FlagParse for T {
    fn decode_flags(d: &mut FlagDecoder) -> Result<T, HammerError> {
        Decodable::decode(d)
    }
}

trait RepresentsFlags : FlagParse + Decodable<UsageDecoder, HammerError> {}
impl<T: FlagParse + Decodable<UsageDecoder, HammerError>> RepresentsFlags for T {}

trait RepresentsJSON : Decodable<json::Decoder, json::DecoderError> {}
impl<T: Decodable<json::Decoder, json::DecoderError>> RepresentsJSON for T {}

#[deriving(Decodable)]
pub struct NoFlags;

hammer_config!(NoFlags)

#[deriving(Decodable)]
pub struct GlobalFlags {
    verbose: bool,
    help: bool,
    rest: Vec<String>
}

hammer_config!(GlobalFlags |c| {
    c.short("verbose", 'v').short("help", 'h')
})

pub fn execute_main<'a,
                    T: RepresentsFlags,
                    U: RepresentsJSON,
                    V: Encodable<json::Encoder<'a>, io::IoError>>(
                        exec: fn(T, U) -> CliResult<Option<V>>)
{
    fn call<'a,
            T: RepresentsFlags,
            U: RepresentsJSON,
            V: Encodable<json::Encoder<'a>, io::IoError>>(
                exec: fn(T, U) -> CliResult<Option<V>>,
                args: &[String])
        -> CliResult<Option<V>>
    {
        let flags = try!(flags_from_args::<T>(args));
        let json = try!(json_from_stdin::<U>());

        exec(flags, json)
    }

    match global_flags() {
        Err(e) => handle_error(e, true),
        Ok(val) => process_executed(call(exec, val.rest.as_slice()), val)
    }
}

pub fn execute_main_without_stdin<'a,
                                  T: RepresentsFlags,
                                  V: Encodable<json::Encoder<'a>, io::IoError>>(
                                      exec: fn(T) -> CliResult<Option<V>>)
{
    fn call<'a,
            T: RepresentsFlags,
            V: Encodable<json::Encoder<'a>, io::IoError>>(
                exec: fn(T) -> CliResult<Option<V>>,
                args: &[String])
        -> CliResult<Option<V>>
    {
        let flags = try!(flags_from_args::<T>(args));

        exec(flags)
    }

    match global_flags() {
        Err(e) => handle_error(e, true),
        Ok(val) => {
            if val.help {
                let (desc, options) = hammer::usage::<T>(true);

                desc.map(|d| println!("{}\n", d));

                println!("Options:\n");

                print!("{}", options);

                let (_, options) = hammer::usage::<GlobalFlags>(false);
                print!("{}", options);
            } else {
                process_executed(call(exec, val.rest.as_slice()), val)
            }
        }
    }
}

pub fn process_executed<'a,
                        T: Encodable<json::Encoder<'a>, io::IoError>>(
                            result: CliResult<Option<T>>,
                            flags: GlobalFlags)
{
    match result {
        Err(e) => handle_error(e, flags.verbose),
        Ok(encodable) => {
            encodable.map(|encodable| {
                let encoded = json::Encoder::str_encode(&encodable);
                println!("{}", encoded);
            });
        }
    }
}

pub fn handle_error(err: CliError, verbose: bool) {
    log!(4, "handle_error; err={}", err);


    let CliError { error, exit_code, unknown, .. } = err;

    let tty = stderr_raw().isatty();
    let stderr = box stderr() as Box<Writer>;

    let config = ShellConfig { color: true, verbose: false, tty: tty };
    let mut shell = Shell::create(stderr, config);

    if unknown {
        let _ = shell.say("An unknown error occurred", RED);
    } else {
        let _ = shell.say(error.to_str(), RED);
    }

    if unknown && !verbose {
        let _ = shell.say("\nTo learn more, run the command again with --verbose.", BLACK);
    }

    if verbose {
        handle_cause(error, &mut shell);
    }

    std::os::set_exit_status(exit_code as int);
}

fn handle_cause(err: &CargoError, shell: &mut Shell) {
    let _ = shell.say("\nCaused by:", BLACK);
    let _ = shell.say(format!("  {}", err.description()), BLACK);

    err.cause().map(|e| handle_cause(e, shell));
}

fn args() -> Vec<String> {
    std::os::args()
}

fn flags_from_args<T: RepresentsFlags>(args: &[String]) -> CliResult<T> {
    let mut decoder = FlagDecoder::new::<T>(args);
    FlagParse::decode_flags(&mut decoder).map_err(|e: HammerError| {
        CliError::new(e.message, 1)
    })
}

fn global_flags() -> CliResult<GlobalFlags> {
    let mut decoder = FlagDecoder::new::<GlobalFlags>(args().tail());
    Decodable::decode(&mut decoder).map_err(|e: HammerError| {
        CliError::new(e.message, 1)
    })
}

fn json_from_stdin<T: RepresentsJSON>() -> CliResult<T> {
    let mut reader = io::stdin();
    let input = try!(reader.read_to_str().map_err(|_| {
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
