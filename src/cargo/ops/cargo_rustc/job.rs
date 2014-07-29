use util::{CargoResult, Fresh, Dirty, Freshness};

pub struct Job { dirty: Work, fresh: Work }

pub type Work = proc():Send -> CargoResult<()>;

impl Job {
    /// Create a new job representing a unit of work.
    pub fn new(dirty: proc():Send -> CargoResult<()>,
               fresh: proc():Send -> CargoResult<()>) -> Job {
        Job { dirty: dirty, fresh: fresh }
    }

    /// Consumes this job by running it, returning the result of the
    /// computation.
    pub fn run(self, fresh: Freshness) -> CargoResult<()> {
        match fresh {
            Fresh => (self.fresh)(),
            Dirty => (self.dirty)(),
        }
    }
}
