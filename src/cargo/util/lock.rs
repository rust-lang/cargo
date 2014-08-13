use term;
use ipc::Semaphore;

use util::{CargoResult, ChainError, internal};
use core::MultiShell;

#[must_use]
pub struct Guard {
    sem: Semaphore,
}

pub fn global_lock(shell: &mut MultiShell) -> CargoResult<Guard> {
    let sem = try!(Semaphore::new("cargo-lock", 1).chain_error(|| {
        internal("failed to create the OS semaphore")
    }));

    if !sem.try_acquire() {
        try!(shell.say("Waiting for another cargo process to exit...",
                       term::color::YELLOW));
        sem.acquire()
    }
    Ok(Guard { sem: sem })
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.sem.release();
    }
}
