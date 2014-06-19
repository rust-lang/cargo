#![crate_id="cargo"]
#![crate_type="rlib"]

#![feature(macro_rules,phase)]

extern crate debug;
extern crate term;
extern crate url;
extern crate serialize;
extern crate semver;
extern crate hammer;
extern crate toml = "github.com/mneumann/rust-toml#toml";

#[phase(plugin, link)]
extern crate log;

#[cfg(test)]
extern crate hamcrest;

use serialize::{Decoder, Encoder, Decodable, Encodable, json};
use std::io;
use hammer::{FlagDecoder, FlagConfig, HammerError, FlagConfiguration};

pub use util::{CargoError, CliError, CliResult, human};

macro_rules! some(
    ($e:expr) => (
        match $e {
            Some(e) => e,
            None => return None
        }
    )
)

macro_rules! cargo_try (
    ($expr:expr) => ({
        use util::CargoError;
        try!($expr.map_err(|err| err.to_error()))
    })
)

pub mod core;
pub mod ops;
pub mod sources;
pub mod util;

trait RepresentsFlags : FlagConfig + Decodable<FlagDecoder, HammerError> {}
impl<T: FlagConfig + Decodable<FlagDecoder, HammerError>> RepresentsFlags for T {}

trait RepresentsJSON : Decodable<json::Decoder, json::DecoderError> {}
impl <T: Decodable<json::Decoder, json::DecoderError>> RepresentsJSON for T {}

#[deriving(Decodable)]
pub struct NoFlags;

impl FlagConfig for NoFlags {}

#[deriving(Decodable)]
pub struct GlobalFlags {
    verbose: bool,
    rest: Vec<String>
}

impl FlagConfig for GlobalFlags {
    fn config(_: Option<GlobalFlags>, c: FlagConfiguration) -> FlagConfiguration {
        c.short("verbose", 'v')
    }
}

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
        Ok(val) => process_executed(call(exec, val.rest.as_slice()), val)
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

    let CliError { error, exit_code, .. } = err;
    let _ = write!(&mut std::io::stderr(), "{}", error);

    if verbose {
        error.cause().map(handle_cause);
    }

    std::os::set_exit_status(exit_code as int);
}

fn handle_cause(err: &CargoError) {
    println!("\nCaused by:");
    println!("  {}", err.description());

    err.cause().map(handle_cause);
}

fn args() -> Vec<String> {
    std::os::args()
}

fn flags_from_args<T: RepresentsFlags>(args: &[String]) -> CliResult<T> {
    let mut decoder = FlagDecoder::new::<T>(args);
    Decodable::decode(&mut decoder).map_err(|e: HammerError| {
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
