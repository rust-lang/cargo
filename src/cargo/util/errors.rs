use std::io::process::{Command,ProcessOutput,ProcessExit,ExitStatus,ExitSignal};
use std::io::IoError;
use std::fmt;
use std::fmt::{Show, Formatter, FormatError};
use std::str;

use TomlError = toml::Error;

pub trait CargoError {
    fn description(&self) -> String;
    fn detail(&self) -> Option<String> { None }
    fn cause<'a>(&'a self) -> Option<&'a CargoError> { None }
    fn is_human(&self) -> bool { false }

    fn to_error<E: FromError<Self>>(self) -> E {
        FromError::from_error(self)
    }

    fn box_error(self) -> Box<CargoError> {
        box self as Box<CargoError>
    }

    fn concrete(&self) -> ConcreteCargoError {
        ConcreteCargoError {
            description: self.description(),
            detail: self.detail(),
            cause: self.cause().map(|c| box c.concrete() as Box<CargoError>),
            is_human: self.is_human()
        }
    }

    fn with_cause<E: CargoError>(self, cause: E) -> Box<CargoError> {
        let mut concrete = self.concrete();
        concrete.cause = Some(cause.box_error());
        box concrete as Box<CargoError>
    }

    fn mark_human(self) -> Box<CargoError> {
        let mut concrete = self.concrete();
        concrete.is_human = true;
        box concrete as Box<CargoError>
    }
}

pub trait FromError<E> {
    fn from_error(error: E) -> Self;
}

impl<E: CargoError> FromError<E> for Box<CargoError> {
    fn from_error(error: E) -> Box<CargoError> {
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

impl Show for Box<CargoError> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        cargo_try!(write!(f, "{}", self.description()));
        Ok(())
    }
}

impl CargoError for Box<CargoError> {
    fn description(&self) -> String {
        (*self).description()
    }

    fn detail(&self) -> Option<String> {
        (*self).detail()
    }

    fn cause<'a>(&'a self) -> Option<&'a CargoError> {
        (*self).cause()
    }

    fn is_human(&self) -> bool {
        (*self).is_human()
    }

    fn box_error(self) -> Box<CargoError> {
        self
    }
}

pub type CargoResult<T> = Result<T, Box<CargoError>>;

pub trait BoxError<T> {
    fn box_error(self) -> CargoResult<T>;
}

pub trait ChainError<T> {
    fn chain_error<E: CargoError>(self, callback: || -> E) -> CargoResult<T> ;
}

impl<'a, T> ChainError<T> for ||:'a -> CargoResult<T> {
    fn chain_error<E: CargoError>(self, callback: || -> E) -> CargoResult<T> {
        self().map_err(|err| callback().with_cause(err))
    }
}

impl<T, E: CargoError> BoxError<T> for Result<T, E> {
    fn box_error(self) -> CargoResult<T> {
        self.map_err(|err| err.box_error())
    }
}

impl<T, E: CargoError> ChainError<T> for Result<T, E> {
    fn chain_error<E: CargoError>(self, callback: || -> E) -> CargoResult<T>  {
        self.map_err(|err| callback().with_cause(err))
    }
}

impl CargoError for IoError {
    fn description(&self) -> String { self.to_str() }
}

from_error!(IoError)

impl CargoError for TomlError {
    fn description(&self) -> String { self.to_str() }
}

from_error!(TomlError)

impl CargoError for FormatError {
    fn description(&self) -> String {
        "formatting failed".to_str()
    }
}

from_error!(FormatError)

pub struct ProcessError {
    pub msg: String,
    pub command: String,
    pub exit: Option<ProcessExit>,
    pub output: Option<ProcessOutput>,
    pub detail: Option<String>,
    pub cause: Option<Box<CargoError>>
}

from_error!(ProcessError)

impl Show for ProcessError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let exit = match self.exit {
            Some(ExitStatus(i)) | Some(ExitSignal(i)) => i.to_str(),
            None => "never executed".to_str()
        };
        cargo_try!(write!(f, "{} (status={})", self.msg, exit));
        match self.output {
            Some(ref out) => {
                match str::from_utf8(out.output.as_slice()) {
                    Some(s) if s.trim().len() > 0 => {
                        cargo_try!(write!(f, "\n--- stdout\n{}", s));
                    }
                    Some(..) | None => {}
                }
                match str::from_utf8(out.error.as_slice()) {
                    Some(s) if s.trim().len() > 0 => {
                        cargo_try!(write!(f, "\n--- stderr\n{}", s));
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
    fn description(&self) -> String { self.to_str() }

    fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    fn cause<'a>(&'a self) -> Option<&'a CargoError> {
        self.cause.as_ref().map(|c| { let err: &CargoError = *c; err })
    }

    fn with_cause<E: CargoError>(mut self, err: E) -> Box<CargoError> {
        self.cause = Some(err.box_error());
        box self as Box<CargoError>
    }
}

pub struct ConcreteCargoError {
    description: String,
    detail: Option<String>,
    cause: Option<Box<CargoError>>,
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

    fn cause<'a>(&'a self) -> Option<&'a CargoError> {
        self.cause.as_ref().map(|c| { let err: &CargoError = *c; err })
    }

    fn with_cause<E: CargoError>(mut self, err: E) -> Box<CargoError> {
        self.cause = Some(err.box_error());
        box self as Box<CargoError>
    }

    fn mark_human(mut self) -> Box<CargoError> {
        self.is_human = true;
        box self as Box<CargoError>
    }

    fn is_human(&self) -> bool {
        self.is_human
    }
}

pub type CliResult<T> = Result<T, CliError>;

#[deriving(Show)]
pub struct CliError {
    pub error: Box<CargoError>,
    pub exit_code: uint
}

impl CargoError for CliError {
    fn description(&self) -> String {
        self.error.to_str()
    }
}

from_error!(CliError)

impl CliError {
    pub fn new<S: Str>(error: S, code: uint) -> CliError {
        let error = human(error.as_slice().to_str());
        CliError::from_boxed(error, code)
    }

    pub fn from_error<E: CargoError + 'static>(error: E, code: uint) -> CliError {
        let error = box error as Box<CargoError>;
        CliError::from_boxed(error, code)
    }

    pub fn from_boxed(error: Box<CargoError>, code: uint) -> CliError {
        let error = if error.is_human() {
            error
        } else {
            human("An unknown error occurred").with_cause(error)
        };

        CliError { error: error, exit_code: code }
    }
}

pub fn process_error<S: Str>(msg: S,
                             command: &Command,
                             status: Option<&ProcessExit>,
                             output: Option<&ProcessOutput>) -> ProcessError {
    ProcessError {
        msg: msg.as_slice().to_str(),
        command: command.to_str(),
        exit: status.map(|o| o.clone()),
        output: output.map(|o| o.clone()),
        detail: None,
        cause: None
    }
}

pub fn internal_error<S1: Str, S2: Str>(error: S1,
                                        detail: S2) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.as_slice().to_str(),
        detail: Some(detail.as_slice().to_str()),
        cause: None,
        is_human: false
    } as Box<CargoError>
}

pub fn internal<S1: Str>(error: S1) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.as_slice().to_str(),
        detail: None,
        cause: None,
        is_human: false
    } as Box<CargoError>
}

pub fn human<S: Str>(error: S) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.as_slice().to_str(),
        detail: None,
        cause: None,
        is_human: true
    } as Box<CargoError>
}
