#[crate_id="cargo"];
#[crate_type="rlib"];

extern crate serialize;
use serialize::{Decoder};
use std::fmt;
use std::fmt::{Show,Formatter};

mod util;

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
