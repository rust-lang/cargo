use std::fmt;
use std::fmt::{Show,Formatter};
use std::io;
use std::io::IoError;
use std::io::process::{ProcessOutput,ProcessExit};
use core::errors::{CLIError,CLIResult};
use toml;

/*
 * CargoResult should be used in libcargo. CargoCliResult should be used in the
 * various executables.
 */

pub type CargoResult<T> = Result<T, CargoError>;

pub fn other_error(desc: &'static str) -> CargoError {
    CargoError {
        kind: OtherCargoError,
        desc: StaticDescription(desc),
        detail: None,
        cause: None
    }
}

pub fn io_error(err: IoError) -> CargoError {
    let desc = err.desc;

    CargoError {
        kind: IoError(err),
        desc: StaticDescription(desc),
        detail: None,
        cause: None
    }
}

pub fn process_error(detail: String, exit: ProcessExit, output: Option<ProcessOutput>) -> CargoError {
    CargoError {
        kind: ProcessError(exit, output),
        desc: BoxedDescription(detail),
        detail: None,
        cause: None
    }
}

pub fn human_error<T: ToStr, U: ToStr>(desc: T, detail: U, cause: CargoError) -> CargoError {
    CargoError {
        kind: HumanReadableError,
        desc: BoxedDescription(desc.to_str()),
        detail: Some(detail.to_str()),
        cause: Some(box cause)
    }
}

pub fn simple_human<T: Show>(desc: T) -> CargoError {
    CargoError {
        kind: HumanReadableError,
        desc: BoxedDescription(desc.to_str()),
        detail: None,
        cause: None
    }
}

pub fn toml_error(desc: &'static str, error: toml::Error) -> CargoError {
    CargoError {
        kind: TomlError(error),
        desc: StaticDescription(desc),
        detail: None,
        cause: None
    }
}

#[deriving(PartialEq,Clone)]
pub struct CargoError {
    pub kind: CargoErrorKind,
    desc: CargoErrorDescription,
    detail: Option<String>,
    cause: Option<Box<CargoError>>
}

impl Show for CargoError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self.desc {
            StaticDescription(string) => write!(f, "{}", string),
            BoxedDescription(ref string) => write!(f, "{}", string)
        };

        write!(f, "; kind={}", self.kind)
    }
}

#[deriving(PartialEq,Show,Clone)]
enum CargoErrorDescription {
    StaticDescription(&'static str),
    BoxedDescription(String)
}

impl CargoError {
    pub fn get_desc<'a>(&'a self) -> &'a str {
        match self.desc {
            StaticDescription(desc) => desc,
            BoxedDescription(ref desc) => desc.as_slice()
        }
    }

    pub fn get_detail<'a>(&'a self) -> Option<&'a str> {
        self.detail.as_ref().map(|s| s.as_slice())
    }

    pub fn with_detail<T: Show>(mut self, detail: T) -> CargoError {
        self.detail = Some(detail.to_str());
        self
    }

    pub fn to_cli(self, exit_code: uint) -> CLIError {
        match self {
            CargoError { kind: HumanReadableError, desc: BoxedDescription(desc), detail: detail, .. } => {
                CLIError::new(desc, detail, exit_code)
            },
            CargoError { kind: InternalError, desc: StaticDescription(desc), detail: None, .. } => {
                CLIError::new("An unexpected error occurred", Some(desc), exit_code)
            },
            CargoError { kind: InternalError, desc: StaticDescription(desc), detail: Some(detail), .. } => {
                CLIError::new("An unexpected error occurred", Some(format!("{}\n{}", desc, detail)), exit_code)
            },
            _ => {
                CLIError::new("An unexpected error occurred", None::<&str>, exit_code)
            }
        }
    }
}

#[deriving(PartialEq)]
pub enum CargoErrorKind {
    HumanReadableError,
    InternalError,
    ProcessError(ProcessExit, Option<ProcessOutput>),
    IoError(io::IoError),
    TomlError(toml::Error),
    OtherCargoError
}

impl Show for CargoErrorKind {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            &ProcessError(ref exit, _) => write!(f, "ProcessError({})", exit),
            &HumanReadableError => write!(f, "HumanReadableError"),
            &InternalError => write!(f, "InternalError"),
            &IoError(ref err) => write!(f, "IoError({})", err),
            &TomlError(ref err) => write!(f, "TomlError({})", err),
            &OtherCargoError => write!(f, "OtherCargoError")
        }
    }
}

impl Clone for CargoErrorKind {
    fn clone(&self) -> CargoErrorKind {
        match self {
            &ProcessError(ref exit, ref output) => {
                ProcessError(exit.clone(), output.as_ref().map(|output| ProcessOutput {
                    status: output.status.clone(), output: output.output.clone(), error: output.error.clone()
                }))
            },
            &HumanReadableError => HumanReadableError,
            &InternalError => InternalError,
            &IoError(ref err) => IoError(err.clone()),
            &TomlError(ref err) => TomlError(err.clone()),
            &OtherCargoError => OtherCargoError
        }
    }
}

type CargoCliResult<T> = Result<T, CargoCliError>;

#[deriving(Show,Clone)]
pub struct CargoCliError {
    kind: CargoCliErrorKind,
    exit_status: uint,
    desc: &'static str,
    detail: Option<String>,
    cause: Option<CargoError>
}

#[deriving(Show,Clone)]
pub enum CargoCliErrorKind {
    OtherCargoCliError
}

pub trait Wrap {
    fn wrap(self, desc: &'static str) -> Self;
}

impl<T> Wrap for Result<T, CargoError> {
    fn wrap(self, desc: &'static str) -> Result<T, CargoError> {
        match self {
            Ok(x) => Ok(x),
            Err(e) => {
                Err(CargoError {
                    kind: e.kind.clone(),
                    desc: StaticDescription(desc),
                    detail: None,
                    cause: Some(box e)
                })
            }
        }
    }
}

pub trait Require<T> {
    fn require(self, err: CargoError) -> CargoResult<T>;
}

impl<T> Require<T> for Option<T> {
    fn require(self, err: CargoError) -> CargoResult<T> {
        match self {
            Some(x) => Ok(x),
            None => Err(err)
        }
    }
}

pub trait ToCLI<T> {
    fn to_cli(self, exit_code: uint) -> CLIResult<T>;
}

impl<T> ToCLI<T> for Result<T, CargoError> {
    fn to_cli(self, exit_code: uint) -> CLIResult<T> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => Err(err.to_cli(exit_code))
        }
    }
}
