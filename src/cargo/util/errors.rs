use std::io::process::{Command,ProcessOutput,ProcessExit,ExitStatus,ExitSignal};
use std::io::IoError;
use std::fmt;
use std::fmt::{Show, Formatter};

use TomlError = toml::Error;

pub trait CargoError {
    fn description(&self) -> String;
    fn detail(&self) -> Option<String> { None }
    fn cause<'a>(&'a self) -> Option<&'a CargoError> { None }
    fn is_human(&self) -> bool { false }

    fn concrete(&self) -> ConcreteCargoError {
        ConcreteCargoError {
            description: self.description(),
            detail: self.detail(),
            cause: self.cause().map(|c| box c.concrete() as Box<CargoError>),
            is_human: self.is_human()
        }
    }
}

impl Show for Box<CargoError> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "{}", self.description()));
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
}

pub type CargoResult<T> = Result<T, Box<CargoError>>;

impl CargoError for &'static str {
    fn description(&self) -> String { self.to_str() }
    fn is_human(&self) -> bool { true }
}

impl CargoError for String {
    fn description(&self) -> String { self.to_str() }
    fn is_human(&self) -> bool { true }
}

impl CargoError for IoError {
    fn description(&self) -> String { self.to_str() }
}

impl CargoError for TomlError {
    fn description(&self) -> String { self.to_str() }
}

pub struct ProcessError {
    pub command: String,
    pub exit: Option<ProcessExit>,
    pub output: Option<ProcessOutput>,
    pub detail: Option<String>,
    pub cause: Option<Box<CargoError>>
}

impl Show for ProcessError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let exit = match self.exit {
            Some(ExitStatus(i)) | Some(ExitSignal(i)) => i.to_str(),
            None => "never executed".to_str()
        };
        write!(f, "process failed: `{}` (status={})", self.command, exit)
    }
}

impl CargoError for ProcessError {
    fn description(&self) -> String {
        let exit = match self.exit {
            Some(ExitStatus(i)) | Some(ExitSignal(i)) => i.to_str(),
            None => "never executed".to_str()
        };
        format!("Executing `{}` failed (status={})", self.command, exit)
    }

    fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    fn cause<'a>(&'a self) -> Option<&'a CargoError> {
        self.cause.as_ref().map(|c| { let err: &CargoError = *c; err })
    }
}

struct ConcreteCargoError {
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

impl CliError {
    pub fn new<E: CargoError + 'static>(error: E, code: uint) -> CliError {
        let error = box error as Box<CargoError>;
        CliError::from_boxed(error, code)
    }

    pub fn from_boxed(error: Box<CargoError>, code: uint) -> CliError {
        let error = if error.is_human() {
            error
        } else {
            chain(error, "An unknown error occurred")
        };

        CliError { error: error, exit_code: code }
    }
}

pub fn process_error<S: Str>(msg: S, command: &Command, status: Option<&ProcessExit>, output: Option<&ProcessOutput>) -> ProcessError {
    ProcessError {
        command: command.to_str(),
        exit: status.map(|o| o.clone()),
        output: output.map(|o| o.clone()),
        detail: None,
        cause: None
    }
}

pub fn internal_error<S1: Str, S2: Str>(error: S1, detail: S2) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.as_slice().to_str(),
        detail: Some(detail.as_slice().to_str()),
        cause: None,
        is_human: false
    } as Box<CargoError>
}

pub fn error<S1: Str>(error: S1) -> Box<CargoError> {
    box ConcreteCargoError {
        description: error.as_slice().to_str(),
        detail: None,
        cause: None,
        is_human: false
    } as Box<CargoError>
}

pub fn human<E: CargoError>(error: E) -> Box<CargoError> {
    let mut concrete = error.concrete();
    concrete.is_human = true;
    box concrete as Box<CargoError>
}

pub fn chain<E: CargoError>(original: Box<CargoError>, update: E) -> Box<CargoError> {
    let mut concrete = update.concrete();
    concrete.cause = Some(original);
    box concrete as Box<CargoError>
}

pub fn box_error<S: CargoError + 'static>(err: S) -> Box<CargoError> {
    box err as Box<CargoError>
}
