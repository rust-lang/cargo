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

use serialize::{Decoder,Encoder,Decodable,Encodable,json};
use std::io;
use hammer::{FlagDecoder,FlagConfig,HammerError};
pub use util::{CliError, CliResult, human};

macro_rules! some(
  ($e:expr) => (
    match $e {
      Some(e) => e,
      None => return None
    }
  ))

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

pub fn execute_main<'a, T: RepresentsFlags, U: RepresentsJSON, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T, U) -> CliResult<Option<V>>) {
    fn call<'a, T: RepresentsFlags, U: RepresentsJSON, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T, U) -> CliResult<Option<V>>) -> CliResult<Option<V>> {
        let flags = try!(flags_from_args::<T>());
        let json = try!(json_from_stdin::<U>());

        exec(flags, json)
    }

    process_executed(call(exec))
}

pub fn execute_main_without_stdin<'a, T: RepresentsFlags, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T) -> CliResult<Option<V>>) {
    fn call<'a, T: RepresentsFlags, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T) -> CliResult<Option<V>>) -> CliResult<Option<V>> {
        let flags = try!(flags_from_args::<T>());

        exec(flags)
    }

    process_executed(call(exec));
}

pub fn process_executed<'a, T: Encodable<json::Encoder<'a>, io::IoError>>(result: CliResult<Option<T>>) {
    match result {
        Err(e) => handle_error(e),
        Ok(encodable) => {
            encodable.map(|encodable| {
                let encoded = json::Encoder::str_encode(&encodable);
                println!("{}", encoded);
            });
        }
    }
}

pub fn handle_error(err: CliError) {
    log!(4, "handle_error; err={}", err);

    let CliError { error, exit_code, .. } = err;
    let _ = write!(&mut std::io::stderr(), "{}", error);
    // TODO: Cause chains
    //detail.map(|d| write!(&mut std::io::stderr(), ":\n{}", d));

    std::os::set_exit_status(exit_code as int);
}

fn args() -> Vec<String> {
    std::os::args()
}

fn flags_from_args<T: RepresentsFlags>() -> CliResult<T> {
    let mut decoder = FlagDecoder::new::<T>(args().tail());
    Decodable::decode(&mut decoder).map_err(|e: HammerError| CliError::new(e.message, 1))
}

fn json_from_stdin<T: RepresentsJSON>() -> CliResult<T> {
    let mut reader = io::stdin();
    let input = try!(reader.read_to_str().map_err(|_| CliError::new("Standard in did not exist or was not UTF-8", 1)));

    let json = try!(json::from_str(input.as_slice()).map_err(|_| CliError::new("Could not parse standard in as JSON", 1)));
    let mut decoder = json::Decoder::new(json);

    Decodable::decode(&mut decoder).map_err(|_| CliError::new("Could not process standard in as input", 1))
}
