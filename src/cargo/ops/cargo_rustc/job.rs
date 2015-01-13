use std::sync::mpsc::Sender;

use util::{CargoResult, Fresh, Dirty, Freshness};

pub struct Job { dirty: Work, fresh: Work }

/// Each proc should send its description before starting.
/// It should send either once or close immediatly.
pub struct Work {
    inner: Box<FnBox<Sender<String>, CargoResult<()>> + Send>,
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
                  where F: FnOnce(Sender<String>) -> CargoResult<()> + Send {
        Work { inner: Box::new(f) }
    }

    pub fn noop() -> Work {
        Work::new(|_| Ok(()))
    }

    pub fn call(self, tx: Sender<String>) -> CargoResult<()> {
        self.inner.call_box(tx)
    }
}

impl Job {
    /// Create a new job representing a unit of work.
    pub fn new(dirty: Work,
               fresh: Work) -> Job {
        Job { dirty: dirty, fresh: fresh }
    }

    /// Create a new job which will run `fresh` if the job is fresh and
    /// otherwise not run `dirty`.
    ///
    /// Retains the same signature as `new` for compatibility. This job does not
    /// describe itself to the console.
    pub fn noop(_dirty: Work,
                fresh: Work) -> Job {
        Job { dirty: Work::noop(), fresh: fresh }
    }

    /// Consumes this job by running it, returning the result of the
    /// computation.
    pub fn run(self, fresh: Freshness, tx: Sender<String>) -> CargoResult<()> {
        match fresh {
            Fresh => self.fresh.call(tx),
            Dirty => self.dirty.call(tx),
        }
    }
}
