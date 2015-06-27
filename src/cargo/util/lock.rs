use file_lock::{FileLock, LockKind, AccessMode, ParseError};
use file_lock::flock::Error as FileLockError;

use std::path::PathBuf;

use util::{Config, CargoError, CargoResult};

impl From<FileLockError> for Box<CargoError> {
    fn from(t: FileLockError) -> Self {
        Box::new(t)
    }
}

impl From<ParseError> for Box<CargoError> {
    fn from(t: ParseError) -> Self {
        Box::new(t)
    }
}

impl CargoError for FileLockError {}
impl CargoError for ParseError {}

pub struct CargoLock<'cfg> {
    config: &'cfg Config, 
    inner: FileLock,
}

impl<'cfg> CargoLock<'cfg> {
    pub fn new_exclusive(path: PathBuf, config: &'cfg Config) -> CargoLock<'cfg> {
        CargoLock {
            config: config,
            inner: FileLock::new(path, AccessMode::Write)
        }
    }

    pub fn new_shared(path: PathBuf, config: &'cfg Config) -> CargoLock<'cfg> {
        CargoLock {
            config: config,
            inner: FileLock::new(path, AccessMode::Read)
        }
    }

    pub fn lock(&mut self) -> CargoResult<()> {
        let kind: LockKind = try!(try!(self.config.get_string("build.lock-kind"))
                                                  .map(|t| t.0)
                                                  .unwrap_or_else(|| LockKind::NonBlocking
                                                                                .as_ref()
                                                                                .to_owned())
                                .parse());
        Ok(try!(self.inner.any_lock(kind)))
    }
}
