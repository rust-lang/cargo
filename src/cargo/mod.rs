#[crate_id="cargo"];
#[crate_type="rlib"];

#[allow(deprecated_owned_vector)];

extern crate serialize;
extern crate hammer;

use serialize::{Decoder,Encoder,Decodable,Encodable,json};
use std::io;
use std::fmt;
use std::fmt::{Show,Formatter};
use hammer::{FlagDecoder,FlagConfig};

pub mod util;

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct Manifest {
    project: ~Project,
    root: ~str,
    lib: ~[LibTarget],
    bin: ~[ExecTarget]
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct ExecTarget {
    name: ~str,
    path: ~str
}

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct LibTarget {
    name: ~str,
    path: ~str
}

//pub type LibTarget = Target;
//pub type ExecTarget = Target;

#[deriving(Decodable,Encodable,Eq,Clone,Ord)]
pub struct Project {
    name: ~str,
    version: ~str,
    authors: ~[~str]
}

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

pub trait ToCargoError<T> {
    fn to_cargo_error(self, message: ~str, exit_code: uint) -> Result<T, CargoError>;
}

impl<T,U> ToCargoError<T> for Result<T,U> {
    fn to_cargo_error(self, message: ~str, exit_code: uint) -> Result<T, CargoError> {
        match self {
            Err(_) => Err(CargoError{ message: message, exit_code: exit_code }),
            Ok(val) => Ok(val)
        }
    }
}

impl<T> ToCargoError<T> for Option<T> {
    fn to_cargo_error(self, message: ~str, exit_code: uint) -> CargoResult<T> {
        match self {
            None => Err(CargoError{ message: message, exit_code: exit_code }),
            Some(val) => Ok(val)
        }
    }
}

trait RepresentsFlags : FlagConfig + Decodable<FlagDecoder> {}
impl<T: FlagConfig + Decodable<FlagDecoder>> RepresentsFlags for T {}

trait RepresentsJSON : Decodable<json::Decoder> {}
impl <T: Decodable<json::Decoder>> RepresentsJSON for T {}

#[deriving(Decodable)]
pub struct NoFlags;

impl FlagConfig for NoFlags {}

pub fn execute_main<'a, T: RepresentsFlags, U: RepresentsJSON, V: Encodable<json::Encoder<'a>>>(exec: fn(T, U) -> CargoResult<Option<V>>) {
    fn call<'a, T: RepresentsFlags, U: RepresentsJSON, V: Encodable<json::Encoder<'a>>>(exec: fn(T, U) -> CargoResult<Option<V>>) -> CargoResult<Option<V>> {
        let flags = try!(flags_from_args::<T>());
        let json = try!(json_from_stdin::<U>());

        exec(flags, json)
    }

    process_executed(call(exec))
}

pub fn execute_main_without_stdin<'a, T: RepresentsFlags, V: Encodable<json::Encoder<'a>>>(exec: fn(T) -> CargoResult<Option<V>>) {
    fn call<'a, T: RepresentsFlags, V: Encodable<json::Encoder<'a>>>(exec: fn(T) -> CargoResult<Option<V>>) -> CargoResult<Option<V>> {
        let flags = try!(flags_from_args::<T>());

        exec(flags)
    }

    process_executed(call(exec))
}

fn process_executed<'a, T: Encodable<json::Encoder<'a>>>(result: CargoResult<Option<T>>) {
    match result {
        Err(e) => {
            let _ = write!(&mut std::io::stderr(), "{}", e.message);
            std::os::set_exit_status(e.exit_code as int);
        },
        Ok(encodable) => {
            encodable.map(|encodable| {
                let encoded: ~str = json::Encoder::str_encode(&encodable);
                println!("{}", encoded);
            });
        }
    }
}

fn flags_from_args<T: RepresentsFlags>() -> CargoResult<T> {
    let mut decoder = FlagDecoder::new::<T>(std::os::args().tail());
    let flags: T = Decodable::decode(&mut decoder);

    match decoder.error {
        Some(err) => Err(CargoError::new(err, 1)),
        None => Ok(flags)
    }
}

fn json_from_stdin<T: RepresentsJSON>() -> CargoResult<T> {
    let mut reader = io::stdin();
    let input = try!(reader.read_to_str().to_cargo_error(~"Cannot read stdin to a string", 1));

    let json = try!(json::from_str(input).to_cargo_error(format!("Cannot parse json: {}", input), 1));
    let mut decoder = json::Decoder::new(json);

    Ok(Decodable::decode(&mut decoder))
}
