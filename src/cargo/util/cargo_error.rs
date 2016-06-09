use std::error::Error;
use std::fmt;
use std::str;

pub type CargoResult<T> = Result<T, Box<CargoError>>;

// =============================================================================
// CargoError trait

pub trait CargoError: Error + Send + 'static {
    fn is_human(&self) -> bool { false }
    fn cargo_cause(&self) -> Option<&CargoError>{ None }
}

impl Error for Box<CargoError> {
    fn description(&self) -> &str { (**self).description() }
    fn cause(&self) -> Option<&Error> { (**self).cause() }
}

impl CargoError for Box<CargoError> {
    fn is_human(&self) -> bool { (**self).is_human() }
    fn cargo_cause(&self) -> Option<&CargoError> { (**self).cargo_cause() }
}

// =============================================================================
// Chaining errors

pub trait ChainError<T> {
    fn chain_error<E, F>(self, callback: F) -> CargoResult<T>
                         where E: CargoError, F: FnOnce() -> E;
}

#[derive(Debug)]
struct ChainedError<E> {
    error: E,
    cause: Box<CargoError>,
}

impl<'a, T, F> ChainError<T> for F where F: FnOnce() -> CargoResult<T> {
    fn chain_error<E, C>(self, callback: C) -> CargoResult<T>
                         where E: CargoError, C: FnOnce() -> E {
        self().chain_error(callback)
    }
}

impl<T, E: CargoError + 'static> ChainError<T> for Result<T, E> {
    fn chain_error<E2: 'static, C>(self, callback: C) -> CargoResult<T>
                         where E2: CargoError, C: FnOnce() -> E2 {
        self.map_err(move |err| {
            Box::new(ChainedError {
                error: callback(),
                cause: Box::new(err),
            }) as Box<CargoError>
        })
    }
}

impl<T> ChainError<T> for Box<CargoError> {
    fn chain_error<E2, C>(self, callback: C) -> CargoResult<T>
                         where E2: CargoError, C: FnOnce() -> E2 {
        Err(Box::new(ChainedError {
            error: callback(),
            cause: self,
        }))
    }
}

impl<T> ChainError<T> for Option<T> {
    fn chain_error<E: 'static, C>(self, callback: C) -> CargoResult<T>
                         where E: CargoError, C: FnOnce() -> E {
        match self {
            Some(t) => Ok(t),
            None => Err(Box::new(callback())),
        }
    }
}

impl<E: Error> Error for ChainedError<E> {
    fn description(&self) -> &str { self.error.description() }
}

impl<E: fmt::Display> fmt::Display for ChainedError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl<E: CargoError> CargoError for ChainedError<E> {
    fn is_human(&self) -> bool { self.error.is_human() }
    fn cargo_cause(&self) -> Option<&CargoError> { Some(&*self.cause) }
}

// =============================================================================
// HumanError errors

#[derive(Debug)]
struct HumanError(Box<CargoError>);

impl Error for HumanError {
    fn description(&self) -> &str { self.0.description() }
    fn cause(&self) -> Option<&Error> { self.0.cause() }
}

impl fmt::Display for HumanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl CargoError for HumanError {
    fn is_human(&self) -> bool { true }
    fn cargo_cause(&self) -> Option<&CargoError> { self.0.cargo_cause() }
}

impl From<HumanError> for Box<CargoError> {
    fn from(t: HumanError) -> Box<CargoError> { Box::new(t) }
}

// =============================================================================
// InternalError errors

#[derive(Debug)]
struct InternalError(Box<CargoError>);

impl Error for InternalError {
    fn description(&self) -> &str { self.0.description() }
    fn cause(&self) -> Option<&Error> { self.0.cause() }
}

impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl CargoError for InternalError {
    fn is_human(&self) -> bool { false }
    fn cargo_cause(&self) -> Option<&CargoError> { self.0.cargo_cause() }
}

impl From<InternalError> for Box<CargoError> {
    fn from(t: InternalError) -> Box<CargoError> { Box::new(t) }
}

// =============================================================================
// Concrete errors

struct ConcreteError(Box<Error+Send>);

impl fmt::Display for ConcreteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for ConcreteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for ConcreteError {
    fn description(&self) -> &str { &self.0.description() }
    fn cause(&self) -> Option<&Error> { Some(&*self.0) }
}

impl CargoError for ConcreteError {
    fn is_human(&self) -> bool { false }
}

// =============================================================================
// Strings as errors

#[derive(Debug)]
pub struct StringError(String);

impl Error for StringError {
    fn description(&self) -> &str { &self.0 }
    fn cause(&self) -> Option<&Error> { None }
}

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl CargoError for StringError {
    fn is_human(&self) -> bool { true }
    fn cargo_cause(&self) -> Option<&CargoError> { None }
}

impl From<StringError> for Box<CargoError> {
    fn from(t: StringError) -> Box<CargoError> { Box::new(t) }
}

// =============================================================================
// Stuff

#[macro_export]
macro_rules! from_error {
    ($($p:ty,)*) => (
        $(impl From<$p> for Box<CargoError> {
            fn from(t: $p) -> Box<CargoError> { Box::new(t) }
        })*
    )
}

pub fn internal<S: fmt::Display>(error: S) -> Box<CargoError> {
    Box::new(InternalError(Box::new(StringError(error.to_string()))))
}

pub fn caused_internal<S, E>(error: S, cause: E) -> Box<CargoError>
    where S: fmt::Display,
          E: Error + Send + 'static
{
    Box::new(ChainedError {
        error: internal(error),
        cause: Box::new(ConcreteError(Box::new(cause)))
    })
}

pub fn human<S: fmt::Display>(error: S) -> Box<CargoError> {
    Box::new(HumanError(Box::new(StringError(error.to_string()))))
}

pub fn caused_human<S, E>(error: S, cause: E) -> Box<CargoError>
    where S: fmt::Display,
          E: Error + Send + 'static
{
    Box::new(HumanError(Box::new(ChainedError {
        error: StringError(error.to_string()),
        cause: Box::new(ConcreteError(Box::new(cause)))
    })))
}
