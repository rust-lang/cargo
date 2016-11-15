use std::fmt;

use util::{CargoResult, Fresh, Dirty, Freshness};
use super::job_queue::JobState;

pub struct Job { dirty: Work, fresh: Work }

/// Each proc should send its description before starting.
/// It should send either once or close immediately.
pub struct Work {
    inner: Box<for <'a, 'b> FnBox<&'a JobState<'b>, CargoResult<()>> + Send>,
}

trait FnBox<A, R> {
    fn call_box(self: Box<Self>, a: A) -> R;
}

impl<A, R, F: FnOnce(A) -> R> FnBox<A, R> for F {
    fn call_box(self: Box<F>, a: A) -> R {
        (*self)(a)
    }
}

impl Work {
    pub fn new<F>(f: F) -> Work
        where F: FnOnce(&JobState) -> CargoResult<()> + Send + 'static
    {
        Work { inner: Box::new(f) }
    }

    pub fn noop() -> Work {
        Work::new(|_| Ok(()))
    }

    pub fn call(self, tx: &JobState) -> CargoResult<()> {
        self.inner.call_box(tx)
    }

    pub fn then(self, next: Work) -> Work {
        Work::new(move |state| {
            self.call(state)?;
            next.call(state)
        })
    }
}

impl Job {
    /// Create a new job representing a unit of work.
    pub fn new(dirty: Work, fresh: Work) -> Job {
        Job { dirty: dirty, fresh: fresh }
    }

    /// Consumes this job by running it, returning the result of the
    /// computation.
    pub fn run(self, fresh: Freshness, state: &JobState) -> CargoResult<()> {
        match fresh {
            Fresh => self.fresh.call(state),
            Dirty => self.dirty.call(state),
        }
    }
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Job {{ ... }}")
    }
}
