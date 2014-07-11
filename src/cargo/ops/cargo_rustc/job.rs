use util::CargoResult;
use std::sync::{Arc, Mutex};

pub struct Job {
    work: proc():Send -> CargoResult<Vec<Job>>,
}

impl Job {
    /// Create a new job representing a unit of work.
    pub fn new(work: proc():Send -> CargoResult<Vec<Job>>) -> Job {
        Job { work: work }
    }

    /// Creates a new job which will execute all of `jobs` and then return the
    /// work `after` if they all succeed sequentially.
    pub fn all(jobs: Vec<Job>, after: Vec<Job>) -> Job {
        Job::new(proc() {
            for job in jobs.move_iter() {
                try!(job.run());
            }
            Ok(after)
        })
    }

    /// Maps a list of jobs to a new list of jobs which will run `after` once
    /// all the jobs have completed.
    pub fn after(jobs: Vec<Job>, after: Job) -> Vec<Job> {
        if jobs.len() == 0 { return vec![after] }

        struct State { job: Option<Job>, remaining: uint }

        let lock = Arc::new(Mutex::new(State {
            job: Some(after),
            remaining: jobs.len(),
        }));

        jobs.move_iter().map(|job| {
            let my_lock = lock.clone();
            Job::new(proc() {
                try!(job.run());
                let mut state = my_lock.lock();
                state.remaining -= 1;
                Ok(if state.remaining == 0 {
                    vec![state.job.take().unwrap()]
                } else {
                    Vec::new()
                })
            })
        }).collect()
    }

    /// Consumes this job by running it, returning the result of the
    /// computation.
    pub fn run(self) -> CargoResult<Vec<Job>> {
        (self.work)()
    }
}
