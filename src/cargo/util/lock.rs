use file_lock::{FileLock, LockKind, AccessMode, ParseError};
use file_lock::flock::Error as FileLockError;

use std::path::PathBuf;
use std::fs;

use util::{Config, CargoError, CargoResult, human};

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
    pub fn new(path: PathBuf, config: &'cfg Config) -> CargoLock<'cfg> {
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
        const CONFIG_KEY: &'static str = "build.lock-kind";
        let kind = match try!(self.config.get_string(CONFIG_KEY)).map(|t| t.0) {
            None => LockKind::NonBlocking,
            Some(kind_string) => match kind_string.parse() {
                Ok(kind) => kind,
                Err(_) => return Err(human(format!("Failed to parse value '{}' at \
                                                   configuration key '{}'.\
                                                   Must be one of '{}' and '{}'",
                                                   kind_string, CONFIG_KEY,
                                                   LockKind::NonBlocking.as_ref(), 
                                                   LockKind::Blocking.as_ref())))
            }
        };
        // NOTE(ST): This could fail if cargo is run concurrently for the first time
        // The only way to prevent it would be to take a lock in a directory which exists.
        // This is why we don't try! here, but hope the directory exists when we 
        // try to create the lock file
        fs::create_dir_all(self.inner.path().parent().unwrap()).ok();
        Ok(try!(self.inner.any_lock(kind)))
    }
}
