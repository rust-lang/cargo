use file_lock::filename::Mode;
use file_lock::filename::Lock as FileLock;

use std::path::PathBuf;
use std::fs;
use std::thread::sleep_ms;

use util::{CargoResult, caused_human};

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
        // NOTE(ST): This could fail if cargo is run concurrently for the first time
        // The only way to prevent it would be to take a lock in a directory which exists.
        // This is why we don't try! here, but hope the directory exists when we 
        // try to create the lock file
        {
            let lock_dir = self.inner.path().parent().unwrap();
            if let Err(_) = fs::create_dir_all(lock_dir) {
                // We might compete to create one or more directories here
                // Give the competing process some time to finish. Then we will
                // retry, hoping it the creation works (maybe just because the )
                // directory is available already.
                // TODO(ST): magic numbers, especially in sleep, will fail at some point ... .
                sleep_ms(100);
                if let Err(io_err) = fs::create_dir_all(lock_dir) {
                    // Fail permanently if it still didn't work ... 
                    return Err(caused_human(format!("Failed to create parent directory of \
                                                     lock-file at '{}'", 
                                                     lock_dir.display()), io_err));
                }
            }
        }
        debug!("About to acquire file lock: '{}'", self.inner.path().display());
        Ok(try!(self.inner.any_lock(kind)))
    }
}
