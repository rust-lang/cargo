//! A load-balancing task pool.
//!
//! This differs in implementation from std::sync::TaskPool in that each job is
//! up for grabs by any of the child tasks in the pool.
//!
//! This should be upstreamed at some point.

use std::sync::{Arc, Mutex};

pub struct TaskPool {
    tx: SyncSender<proc():Send>,
}

impl TaskPool {
    pub fn new(tasks: uint) -> TaskPool {
        assert!(tasks > 0);
        let (tx, rx) = sync_channel(tasks);

        let state = Arc::new(Mutex::new(rx));

        for _ in range(0, tasks) {
            let state = state.clone();
            spawn(proc() worker(&*state));
        }

        return TaskPool { tx: tx };

        fn worker(rx: &Mutex<Receiver<proc():Send>>) {
            loop {
                let job = rx.lock().recv_opt();
                match job {
                    Ok(job) => job(),
                    Err(..) => break,
                }
            }
        }
    }

    pub fn execute(&self, job: proc():Send) {
        self.tx.send(job);
    }
}
