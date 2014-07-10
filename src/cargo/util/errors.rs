use std::io::process::{ProcessOutput, ProcessExit, ExitStatus, ExitSignal};
use std::io::IoError;
use std::fmt;
use std::fmt::{Show, Formatter, FormatError};
use std::str;

use TomlError = toml::Error;

pub trait CargoError: Send {
    fn description(&self) -> String;
    fn detail(&self) -> Option<String> { None }
    fn cause<'a>(&'a self) -> Option<&'a CargoError + Send> { None }
    fn is_human(&self) -> bool { false }

    fn to_error<E: FromError<Self>>(self) -> E {
        FromError::from_error(self)
    }

    fn box_error(self) -> Box<CargoError + Send> {
        box self as Box<CargoError + Send>
    }

    fn concrete(&self) -> ConcreteCargoError {
        ConcreteCargoError {
            description: self.description(),
            detail: self.detail(),
            cause: self.cause().map(|c| box c.concrete() as Box<CargoError + Send>),
            is_human: self.is_human()
        }
    }

    fn with_cause<E: CargoError + Send>(self, cause: E) -> Box<CargoError + Send> {
        let mut concrete = self.concrete();
        concrete.cause = Some(cause.box_error());
        box concrete as Box<CargoError + Send>
    }

    fn mark_human(self) -> Box<CargoError + Send> {
        let mut concrete = self.concrete();
        concrete.is_human = true;
        box concrete as Box<CargoError + Send>
    }
}

pub trait FromError<E> {
    fn from_error(error: E) -> Self;
}

impl<E: CargoError + Send> FromError<E> for Box<CargoError + Send> {
    fn from_error(error: E) -> Box<CargoError + Send> {
        error.box_error()
    }
}

macro_rules! from_error (
    ($ty:ty) => {
        impl FromError<$ty> for $ty {
            fn from_error(error: $ty) -> $ty {
                error
            }
        }
    }
)

impl Show for Box<CargoError + Send> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "{}", self.description()));
        Ok(())
    }
}

impl CargoError for Box<CargoError + Send> {
    fn description(&self) -> String {
        (*self).description()
    }

    fn detail(&self) -> Option<String> {
        (*self).detail()
    }

    fn cause<'a>(&'a self) -> Option<&'a CargoError + Send> {
        (*self).cause()
    }

    fn is_human(&self) -> bool {
        (*self).is_human()
    }

    fn box_error(self) -> Box<CargoError + Send> {
        self
    }
}

pub type CargoResult<T> = Result<T, Box<CargoError + Send>>;

pub trait BoxError<T> {
    fn box_error(self) -> CargoResult<T>;
}

pub trait ChainError<T> {
    fn chain_error<E: CargoError + Send>(self, callback: || -> E) -> CargoResult<T> ;
}

impl<'a, T> ChainError<T> for ||:'a -> CargoResult<T> {
    fn chain_error<E: CargoError + Send>(self, callback: || -> E) -> CargoResult<T> {
        self().map_err(|err| callback().with_cause(err))
    }
}

impl<T, E: CargoError + Send> BoxError<T> for Result<T, E> {
    fn box_error(self) -> CargoResult<T> {
        self.map_err(|err| err.box_error())
    }
}

impl<T, E: CargoError + Send> ChainError<T> for Result<T, E> {
    fn chain_error<E: CargoError + Send>(self, callback: || -> E) -> CargoResult<T>  {
        self.map_err(|err| callback().with_cause(err))
    }
}

impl CargoError for IoError {
    fn description(&self) -> String { self.to_string() }
}

from_error!(IoError)

impl CargoError for TomlError {
    fn description(&self) -> String { self.to_string() }
}

from_error!(TomlError)

impl CargoError for FormatError {
    fn description(&self) -> String {
        "formatting failed".to_string()
    }
}

from_error!(FormatError)

pub struct ProcessError {
    pub msg: String,
    pub exit: Option<ProcessExit>,
    pub output: Option<ProcessOutput>,
    pub detail: Option<String>,
    pub cause: Option<Box<CargoError + Send>>
}

from_error!(ProcessError)

impl Show for ProcessError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let exit = match self.exit {
            Some(ExitStatus(i)) | Some(ExitSignal(i)) => i.to_string(),
            None => "never executed".to_string()
        };
        try!(write!(f, "{} (status={})", self.msg, exit));
        match self.output {
            Some(ref out) => {
                match str::from_utf8(out.output.as_slice()) {
                    Some(s) if s.trim().len() > 0 => {
                        try!(write!(f, "\n--- stdout\n{}", s));
                    }
                    Some(..) | None => {}
                }
                match str::from_utf8(out.error.as_slice()) {
                    Some(s) if s.trim().len() > 0 => {
                        try!(write!(f, "\n--- stderr\n{}", s));
                    }
                    Some(..) | None => {}
                }
            }
            None => {}
        }
        Ok(())
    }
}

impl CargoError for ProcessError {
    fn description(&self) -> String { self.to_string() }

    fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    fn cause<'a>(&'a self) -> Option<&'a CargoError + Send> {
        self.cause.as_ref().map(|c| { let err: &CargoError + Send = *c; err })
    }

    fn with_cause<E: CargoError + Send>(mut self,
                                        err: E) -> Box<CargoError + Send> {
        self.cause = Some(err.box_error());
        box self as Box<CargoError + Send>
    }
}

pub struct ConcreteCargoError {
    description: String,
    detail: Option<String>,
    cause: Option<Box<CargoError + Send>>,
    is_human: bool
}

impl Show for ConcreteCargoError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl CargoError for ConcreteCargoError {
    fn description(&self) -> String {
        self.description.clone()
    }

    fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    fn cause<'a>(&'a self) -> Option<&'a CargoError + Send> {
        self.cause.as_ref().map(|c| { let err: &CargoError + Send = *c; err })
    }

    fn with_cause<E: CargoError + Send>(mut self,
                                        err: E) -> Box<CargoError + Send> {
        self.cause = Some(err.box_error());
        box self as Box<CargoError + Send>
    }

    fn mark_human(mut self) -> Box<CargoError + Send> {
        self.is_human = true;
        box self as Box<CargoError + Send>
    }

    fn is_human(&self) -> bool {
        self.is_human
    }
}

pub type CliResult<T> = Result<T, CliError>;

#[deriving(Show)]
pub struct CliError {
    pub error: Box<CargoError + Send>,
    pub unknown: bool,
    pub exit_code: uint
}

impl CargoError for CliError {
    fn description(&self) -> String {
        self.error.to_string()
    }
}

from_error!(CliError)

impl CliError {
    pub fn new<S: Str>(error: S, code: uint) -> CliError {
        let error = human(error.as_slice().to_string());
        CliError::from_boxed(error, code)
    }

    pub fn from_error<E: CargoError + 'static>(error: E, code: uint) -> CliError {
        let error = box error as Box<CargoError + Send>;
        CliError::from_boxed(error, code)
    }

    pub fn from_boxed(error: Box<CargoError + Send>, code: uint) -> CliError {
        let human = error.is_human();
        CliError { error: error, exit_code: code, unknown: !human }
    }
}

pub fn process_error<S: Str>(msg: S,
                             cause: Option<IoError>,
                             status: Option<&ProcessExit>,
                             output: Option<&ProcessOutput>) -> ProcessError {
    ProcessError {
        msg: msg.as_slice().to_string(),
        exit: status.map(|o| o.clone()),
        output: output.map(|o| o.clone()),
        detail: None,
        cause: cause.map(|c| box c as Box<CargoError + Send>)
    }
}

pub fn internal_error<S1: Str, S2: Str>(error: S1,
                                        detail: S2) -> Box<CargoError + Send> {
    box ConcreteCargoError {
        description: error.as_slice().to_string(),
        detail: Some(detail.as_slice().to_string()),
        cause: None,
        is_human: false
    } as Box<CargoError + Send>
}

pub fn internal<S: Show>(error: S) -> Box<CargoError + Send> {
    box ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: false
    } as Box<CargoError + Send>
}

pub fn human<S: Show>(error: S) -> Box<CargoError + Send> {
    box ConcreteCargoError {
        description: error.to_string(),
        detail: None,
        cause: None,
        is_human: true
    } as Box<CargoError + Send>
}
