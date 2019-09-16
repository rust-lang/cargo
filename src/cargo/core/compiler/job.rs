use std::fmt;
use std::mem;

use super::job_queue::JobState;
use crate::util::CargoResult;

pub struct Job {
    work: Work,
    fresh: Freshness,
}

/// Each proc should send its description before starting.
/// It should send either once or close immediately.
pub struct Work {
    inner: Box<dyn FnOnce(&JobState<'_>) -> CargoResult<()> + Send>,
}

impl Work {
    pub fn new<F>(f: F) -> Work
    where
        F: FnOnce(&JobState<'_>) -> CargoResult<()> + Send + 'static,
    {
        Work { inner: Box::new(f) }
    }

    pub fn noop() -> Work {
        Work::new(|_| Ok(()))
    }

    pub fn call(self, tx: &JobState<'_>) -> CargoResult<()> {
        (self.inner)(tx)
    }

    pub fn then(self, next: Work) -> Work {
        Work::new(move |state| {
            self.call(state)?;
            next.call(state)
        })
    }
}

impl Job {
    /// Creates a new job representing a unit of work.
    pub fn new(work: Work, fresh: Freshness) -> Job {
        Job { work, fresh }
    }

    /// Consumes this job by running it, returning the result of the
    /// computation.
    pub fn run(self, state: &JobState<'_>) -> CargoResult<()> {
        self.work.call(state)
    }

    /// Returns whether this job was fresh/dirty, where "fresh" means we're
    /// likely to perform just some small bookkeeping where "dirty" means we'll
    /// probably do something slow like invoke rustc.
    pub fn freshness(&self) -> Freshness {
        self.fresh
    }

    pub fn before(&mut self, next: Work) {
        let prev = mem::replace(&mut self.work, Work::noop());
        self.work = next.then(prev);
    }
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Job {{ ... }}")
    }
}

/// Indication of the freshness of a package.
///
/// A fresh package does not necessarily need to be rebuilt (unless a dependency
/// was also rebuilt), and a dirty package must always be rebuilt.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Freshness {
    Fresh,
    Dirty,
}
