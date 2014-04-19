#![crate_id="cargo"]
#![crate_type="rlib"]

#![allow(deprecated_owned_vector)]
#![feature(macro_rules)]

extern crate collections;
extern crate hammer;
extern crate serialize;
extern crate semver;
extern crate toml;

#[cfg(test)]
extern crate hamcrest;

use serialize::{Decoder,Encoder,Decodable,Encodable,json};
use std::io;
use std::fmt;
use std::fmt::{Show,Formatter};
use hammer::{FlagDecoder,FlagConfig,HammerError};


pub mod core;
pub mod util;
pub mod sources;
pub mod ops;


pub type CargoResult<T> = Result<T, CargoError>;

pub struct CargoError {
    message: ~str,
    exit_code: uint
}

impl CargoError {
    pub fn new(message: ~str, exit_code: uint) -> CargoError {
        CargoError { message: message, exit_code: exit_code }
    }
}

impl Show for CargoError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f.buf, "{}", self.message)
    }
}

pub trait ToCargoErrorMessage<E> {
    fn to_cargo_error_message(self, error: E) -> ~str;
}

impl<E> ToCargoErrorMessage<E> for ~str {
    fn to_cargo_error_message(self, _: E) -> ~str {
        self
    }
}

impl<'a, E> ToCargoErrorMessage<E> for |E|:'a -> ~str {
    fn to_cargo_error_message(self, err: E) -> ~str {
        self(err)
    }
}

pub trait ToCargoError<T, E> {
    fn to_cargo_error<M: ToCargoErrorMessage<E>>(self, to_message: M, exit_code: uint) -> Result<T, CargoError>;
}

impl<T,E> ToCargoError<T, E> for Result<T,E> {
    fn to_cargo_error<M: ToCargoErrorMessage<E>>(self, to_message: M, exit_code: uint) -> Result<T, CargoError> {
        match self {
            Err(err) => Err(CargoError{ message: to_message.to_cargo_error_message(err), exit_code: exit_code }),
            Ok(val) => Ok(val)
        }
    }
}

impl<T> ToCargoError<T, Option<T>> for Option<T> {
    fn to_cargo_error<M: ToCargoErrorMessage<Option<T>>>(self, to_message: M, exit_code: uint) -> Result<T, CargoError> {
        match self {
            None => Err(CargoError{ message: to_message.to_cargo_error_message(None), exit_code: exit_code }),
            Some(val) => Ok(val)
        }
    }
}

trait RepresentsFlags : FlagConfig + Decodable<FlagDecoder, HammerError> {}
impl<T: FlagConfig + Decodable<FlagDecoder, HammerError>> RepresentsFlags for T {}

trait RepresentsJSON : Decodable<json::Decoder, json::Error> {}
impl <T: Decodable<json::Decoder, json::Error>> RepresentsJSON for T {}

#[deriving(Decodable)]
pub struct NoFlags;

impl FlagConfig for NoFlags {}

pub fn execute_main<'a, T: RepresentsFlags, U: RepresentsJSON, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T, U) -> CargoResult<Option<V>>) {
    fn call<'a, T: RepresentsFlags, U: RepresentsJSON, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T, U) -> CargoResult<Option<V>>) -> CargoResult<Option<V>> {
        let flags = try!(flags_from_args::<T>());
        let json = try!(json_from_stdin::<U>());

        exec(flags, json)
    }

    process_executed(call(exec))
}

pub fn execute_main_without_stdin<'a, T: RepresentsFlags, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T) -> CargoResult<Option<V>>) {
    fn call<'a, T: RepresentsFlags, V: Encodable<json::Encoder<'a>, io::IoError>>(exec: fn(T) -> CargoResult<Option<V>>) -> CargoResult<Option<V>> {
        let flags = try!(flags_from_args::<T>());

        exec(flags)
    }

    process_executed(call(exec))
}

pub fn process_executed<'a, T: Encodable<json::Encoder<'a>, io::IoError>>(result: CargoResult<Option<T>>) {
    match result {
        Err(e) => handle_error(e),
        Ok(encodable) => {
            encodable.map(|encodable| {
                let encoded: ~str = json::Encoder::str_encode(&encodable);
                println!("{}", encoded);
            });
        }
    }
}

pub fn handle_error(err: CargoError) {
    let _ = write!(&mut std::io::stderr(), "{}", err.message);
    std::os::set_exit_status(err.exit_code as int);
}

fn flags_from_args<T: RepresentsFlags>() -> CargoResult<T> {
    let mut decoder = FlagDecoder::new::<T>(std::os::args().tail());
    Decodable::decode(&mut decoder).to_cargo_error(|e: HammerError| e.message, 1)
}

fn json_from_stdin<T: RepresentsJSON>() -> CargoResult<T> {
    let mut reader = io::stdin();
    let input = try!(reader.read_to_str().to_cargo_error(~"Cannot read stdin to a string", 1));

    let json = try!(json::from_str(input).to_cargo_error(format!("Cannot parse json: {}", input), 1));
    let mut decoder = json::Decoder::new(json);

    Decodable::decode(&mut decoder).to_cargo_error(|e: json::Error| format!("{}", e), 1)
}
