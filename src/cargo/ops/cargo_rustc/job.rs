use std::io::IoResult;

use core::MultiShell;
use util::{CargoResult, Fresh, Dirty, Freshness};

pub struct Job { dirty: Work, fresh: Work, desc: String }

pub type Work = proc():Send -> CargoResult<()>;

impl Job {
    /// Create a new job representing a unit of work.
    pub fn new(dirty: proc():Send -> CargoResult<()>,
               fresh: proc():Send -> CargoResult<()>,
               desc: String) -> Job {
        Job { dirty: dirty, fresh: fresh, desc: desc }
    }

    /// Create a new job which will run `fresh` if the job is fresh and
    /// otherwise not run `dirty`.
    ///
    /// Retains the same signature as `new` for compatibility. This job does not
    /// describe itself to the console.
    pub fn noop(_dirty: proc():Send -> CargoResult<()>,
                fresh: proc():Send -> CargoResult<()>,
                _desc: String) -> Job {
        Job { dirty: proc() Ok(()), fresh: fresh, desc: String::new() }
    }

    /// Consumes this job by running it, returning the result of the
    /// computation.
    pub fn run(self, fresh: Freshness) -> CargoResult<()> {
        match fresh {
            Fresh => (self.fresh)(),
            Dirty => (self.dirty)(),
        }
    }

    pub fn describe(&self, shell: &mut MultiShell) -> IoResult<()> {
        if self.desc.len() > 0 {
            try!(shell.status("Running", self.desc.as_slice()));
        }
        Ok(())
    }
}
