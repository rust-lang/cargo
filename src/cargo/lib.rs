#![crate_id="cargo"]
#![crate_type="rlib"]

#![allow(deprecated_owned_vector)]
#![feature(macro_rules,phase)]

extern crate collections;
extern crate url;
extern crate hammer;
extern crate serialize;
extern crate semver;
extern crate toml;

#[phase(syntax, link)]
extern crate log;

#[cfg(test)]
extern crate hamcrest;

use serialize::{Decoder,Encoder,Decodable,Encodable,json};
use std::io;
use hammer::{FlagDecoder,FlagConfig,HammerError};
pub use core::errors::{CLIError,CLIResult,ToResult};

macro_rules! some(
  ($e:expr) => (
    match $e {
      Some(e) => e,
      None => return None
    }
  ))

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

pub fn execute_main<'a, T: RepresentsFlags, U: RepresentsJSON, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T, U) -> CLIResult<Option<V>>) {
    fn call<'a, T: RepresentsFlags, U: RepresentsJSON, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T, U) -> CLIResult<Option<V>>) -> CLIResult<Option<V>> {
        let flags = try!(flags_from_args::<T>());
        let json = try!(json_from_stdin::<U>());

        exec(flags, json)
    }

    process_executed(call(exec))
}

pub fn execute_main_without_stdin<'a, T: RepresentsFlags, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T) -> CLIResult<Option<V>>) {
    fn call<'a, T: RepresentsFlags, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T) -> CLIResult<Option<V>>) -> CLIResult<Option<V>> {
        let flags = try!(flags_from_args::<T>());

        exec(flags)
    }

    process_executed(call(exec));
}

pub fn process_executed<'a, T: Encodable<json::Encoder<'a>, io::IoError>>(result: CLIResult<Option<T>>) {
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

pub fn handle_error(err: CLIError) {
    log!(4, "handle_error; err={}", err);

    let CLIError { msg, exit_code, .. } = err;
    let _ = write!(&mut std::io::stderr(), "{}", msg);
    //detail.map(|d| write!(&mut std::io::stderr(), ":\n{}", d));

    std::os::set_exit_status(exit_code as int);
}

fn args() -> Vec<String> {
    std::os::args()
}

fn flags_from_args<T: RepresentsFlags>() -> CLIResult<T> {
    let mut decoder = FlagDecoder::new::<T>(args().tail());
    Decodable::decode(&mut decoder).to_result(|e: HammerError| CLIError::new(e.message, None::<&str>, 1))
}

fn json_from_stdin<T: RepresentsJSON>() -> CLIResult<T> {
    let mut reader = io::stdin();
    let input = try!(reader.read_to_str().to_result(|_| CLIError::new("Standard in did not exist or was not UTF-8", None::<&str>, 1)));

    let json = try!(json::from_str(input.as_slice()).to_result(|_| CLIError::new("Could not parse standard in as JSON", Some(input.clone()), 1)));
    let mut decoder = json::Decoder::new(json);

    Decodable::decode(&mut decoder).to_result(|e: json::DecoderError| CLIError::new("Could not process standard in as input", Some(e), 1))
}
