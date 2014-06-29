//! A load-balancing task pool.
//!
//! This differs in implementation from std::sync::TaskPool in that each job is
//! up for grabs by any of the child tasks in the pool.
//!
//! This should be upstreamed at some point.

use std::sync::{Arc, Mutex};

pub struct TaskPool {
    state: Arc<Mutex<State>>,
}

struct State { done: bool, jobs: Vec<proc():Send> }

impl TaskPool {
    pub fn new(tasks: uint) -> TaskPool {
        assert!(tasks > 0);

        let state = Arc::new(Mutex::new(State {
            done: false,
            jobs: Vec::new(),
        }));

        for _ in range(0, tasks) {
            let myjobs = state.clone();
            spawn(proc() worker(&*myjobs));
        }

        return TaskPool { state: state };

        fn worker(mystate: &Mutex<State>) {
            let mut state = mystate.lock();
            while !state.done {
                match state.jobs.pop() {
                    Some(job) => {
                        drop(state);
                        job();
                        state = mystate.lock();
                    }
                    None => state.cond.wait(),
                }
            }
        }
    }

    pub fn execute(&self, job: proc():Send) {
        let mut state = self.state.lock();
        state.jobs.push(job);
        state.cond.signal();
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        let mut state = self.state.lock();
        state.done = true;
        state.cond.broadcast();
        drop(state);
    }
}
