use std::io;

pub type CargoResult<T> = Result<T, CargoError>;

#[deriving(Show)]
pub struct CargoError {
    kind: CargoErrorKind,
    desc: &'static str,
    detail: Option<~str>
}

#[deriving(Show)]
pub enum CargoErrorKind {
    InternalError,
    IoError(io::IoError),
    OtherCargoError
}

type CargoCliResult<T> = Result<T, CargoCliError>;

#[deriving(Show)]
pub struct CargoCliError {
    kind: CargoCliErrorKind,
    exit_status: uint,
    desc: &'static str,
    detail: Option<~str>,
    cause: Option<CargoError>
}

#[deriving(Show)]
pub enum CargoCliErrorKind {
    OtherCargoCliError
}
