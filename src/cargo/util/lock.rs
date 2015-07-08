use file_lock::filename::Mode;
use file_lock::filename::Lock as FileLock;

use std::path::PathBuf;
use std::fs;

use util::{CargoResult, ChainError, human};

pub use file_lock::filename::Kind as LockKind;

pub struct CargoLock {
    inner: FileLock,
}

impl CargoLock {
    pub fn new(path: PathBuf) -> CargoLock {
        CargoLock {
            inner: FileLock::new(path, Mode::Write)
        }
    }

    pub fn lock(&mut self, kind: LockKind) -> CargoResult<()> {
        {
            let lock_dir = self.inner.path().parent().unwrap();
            try!(fs::create_dir_all(lock_dir).chain_error(|| { 
                human(format!("Failed to create parent directory of lock-file at '{}'",
                              lock_dir.display()))
            }));
        }
        debug!("About to acquire file lock: '{}'", self.inner.path().display());
        Ok(try!(self.inner.any_lock(kind)))
    }
}
