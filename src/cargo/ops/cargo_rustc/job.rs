use util::{CargoResult, Fresh, Dirty, Freshness};

pub struct Job { dirty: Work, fresh: Work }

/// Each proc should send its description before starting.
/// It should send either once or close immediatly.
pub type Work = proc(Sender<String>):Send -> CargoResult<()>;

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
        Job { dirty: proc(_) Ok(()), fresh: fresh }
    }

    /// Consumes this job by running it, returning the result of the
    /// computation.
    pub fn run(self, fresh: Freshness, tx: Sender<String>) -> CargoResult<()> {
        match fresh {
            Fresh => (self.fresh)(tx),
            Dirty => (self.dirty)(tx),
        }
    }
}
