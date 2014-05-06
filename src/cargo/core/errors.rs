use std::fmt;
use std::fmt::{Show,Formatter};
use std::io::IoError;

/**
 * There are two kinds of errors returned by Cargo functions:
 *
 * * CargoCLIError, which represents a failure that can be directly presented
 *   to the user.
 * * CargoInternalError, which represents an internal failure that must be
 *   converted into a CargoCLIError before it may be presented to the user.
 *
 * These two kinds of errors are wrapped up in a `CargoError` enum.
 *
 * Cargo functions must first convert all other kinds of errors (such as
 * IoError) into one of the Cargo errors before returning.
 *
 * This module includes several convenience functions for working with these
 * different kinds of errors:
 *
 * `to_result::<E1, E2>(|E1| -> E2) -> E2` converts any kind of error into
 * another kind of error. It can be used together with `try!`, as in:
 *
 *     try!(mkdir(path).to_result(|err| {
 *         let msg = format!("Couldn't make directory {}", path.display());
 *         CargoError::cli(msg, 12)
 *     })
 *
 * `to_result::<Option<T>, E>(|| -> E) -> E` converts a `None` value into
 * another kind of error. It can also be used together with `try!`:
 *
 *     try!(from_str(val).to_result(|| {
 *         CargoError::internal(StringConversionError(val))
 *     })
 */

pub type CargoResult<T> = Result<T, CargoError>;
pub type CLIResult<T> = Result<T, CLIError>;

pub enum CargoError {
    CargoInternalError(InternalError),
    CargoCLIError(CLIError)
}

impl Show for CargoError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            &CargoInternalError(ref err) => write!(f.buf, "{}", err),
            &CargoCLIError(ref err) => write!(f.buf, "{}", err)
        }
    }
}

pub struct CLIError {
    pub msg: ~str,
    pub detail: Option<~str>,
    pub exit_code: uint
}

impl CLIError {
    pub fn new<T: ToStr>(msg: T, detail: Option<~str>, exit_code: uint) -> CLIError {
        CLIError { msg: msg.to_str(), detail: detail, exit_code: exit_code }
    }
}

impl Show for CLIError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f.buf, "{}", self.msg)
    }
}

pub enum InternalError {
    StringConversionError(~str, &'static str),
    MissingManifest(Path, ~str),
    WrappedIoError(IoError),
    PathError(~str),
    Described(~str),
    Other
}

impl Show for InternalError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            &StringConversionError(ref string, ref type_name) => {
                write!(f.buf, "Couldn't convert `{}` into {}", string, type_name)
            },
            &MissingManifest(ref path, ref file) => {
                write!(f.buf, "Couldn't find a {} in the project (`{}` or any parent directory", file, path.display())
            },
            &WrappedIoError(ref io_error) => {
                write!(f.buf, "{}", io_error)
            },
            &PathError(ref s) | &Described(ref s) => {
                write!(f.buf, "{}", s)
            },
            &Other => write!(f.buf, "Other internal error")
        }
    }
}

impl CargoError {
    pub fn cli(msg: ~str, detail: Option<~str>, exit_code: uint) -> CargoError {
        CargoCLIError(CLIError::new(msg, detail, exit_code))
    }

    pub fn internal(error: InternalError) -> CargoError {
        CargoInternalError(error)
    }

    pub fn described<T: ToStr>(description: T) -> CargoError {
        CargoInternalError(Described(description.to_str()))
    }

    pub fn other() -> CargoError {
        CargoInternalError(Other)
    }

    pub fn cli_error(self) -> CLIError {
        match self {
            CargoInternalError(err) =>
                CLIError::new("An unexpected error occurred", Some(err.to_str()), 100),
            CargoCLIError(err) => err
        }
    }
}

pub trait ToResult<T,E1,E2> {
    fn to_result(self, callback: |E1| -> E2) -> Result<T,E2>;
}

impl<T,E1,E2> ToResult<T,E1,E2> for Result<T,E1> {
    fn to_result(self, callback: |E1| -> E2) -> Result<T,E2> {
        match self {
            Ok(val) => Ok(val),
            Err(e) => Err(callback(e))
        }
    }
}

impl<T,E> ToResult<T,Option<T>,E> for Option<T> {
    fn to_result(self, callback: |Option<T>| -> E) -> Result<T,E> {
        match self {
            Some(val) => Ok(val),
            None => Err(callback(self))
        }
    }
}
