use std::fmt;
use std::fmt::{Show,Formatter};
use std::io::IoError;

/*
 * Deprecated and will be phased out. Use util::result instead
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
            &CargoInternalError(ref err) => write!(f, "{}", err),
            &CargoCLIError(ref err) => write!(f, "{}", err)
        }
    }
}

pub struct CLIError {
    pub msg: String,
    pub detail: Option<String>,
    pub exit_code: uint
}

impl CLIError {
    pub fn new<T: Show, U: Show>(msg: T, detail: Option<U>,
                                 exit_code: uint) -> CLIError {
        let detail = detail.map(|d| d.to_string());
        CLIError { msg: msg.to_string(), detail: detail, exit_code: exit_code }
    }
}

impl Show for CLIError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

pub enum InternalError {
    StringConversionError(String, &'static str),
    MissingManifest(Path, String),
    WrappedIoError(IoError),
    PathError(String),
    Described(String),
    Other
}

impl Show for InternalError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            &StringConversionError(ref string, ref type_name) => {
                write!(f, "Couldn't convert `{}` into {}", string, type_name)
            },
            &MissingManifest(ref path, ref file) => {
                write!(f, "Couldn't find a {} in the project (`{}` or any \
                           parent directory", file, path.display())
            },
            &WrappedIoError(ref io_error) => {
                write!(f, "{}", io_error)
            },
            &PathError(ref s) | &Described(ref s) => {
                write!(f, "{}", s)
            },
            &Other => write!(f, "Other internal error")
        }
    }
}

impl CargoError {
    pub fn cli(msg: String, detail: Option<String>,
               exit_code: uint) -> CargoError {
        CargoCLIError(CLIError::new(msg, detail, exit_code))
    }

    pub fn internal(error: InternalError) -> CargoError {
        CargoInternalError(error)
    }

    pub fn described<T: Show>(description: T) -> CargoError {
        CargoInternalError(Described(description.to_string()))
    }

    pub fn other() -> CargoError {
        CargoInternalError(Other)
    }

    pub fn cli_error(self) -> CLIError {
        match self {
            CargoInternalError(err) =>
                CLIError::new("An unexpected error occurred", Some(err), 100),
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
