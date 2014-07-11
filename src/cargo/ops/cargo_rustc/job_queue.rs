use std::collections::HashMap;
use term::color::YELLOW;

use core::Package;
use util::{Config, TaskPool, DependencyQueue, Fresh, Dirty, Freshness};
use util::CargoResult;

use super::job::Job;

pub struct JobQueue<'a, 'b> {
    pool: TaskPool,
    queue: DependencyQueue<(&'a Package, Job)>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    active: HashMap<String, uint>,
    config: &'b mut Config<'b>,
}

type Message = (String, Freshness, CargoResult<Vec<Job>>);

impl<'a, 'b> JobQueue<'a, 'b> {
    pub fn new(config: &'b mut Config<'b>,
               jobs: Vec<(&'a Package, Freshness, Job)>) -> JobQueue<'a, 'b> {
        let (tx, rx) = channel();
        let mut queue = DependencyQueue::new();
        for &(pkg, _, _) in jobs.iter() {
            queue.register(pkg);
        }
        for (pkg, fresh, job) in jobs.move_iter() {
            queue.enqueue(pkg, fresh, (pkg, job));
        }

        JobQueue {
            pool: TaskPool::new(config.jobs()),
            queue: queue,
            tx: tx,
            rx: rx,
            active: HashMap::new(),
            config: config,
        }
    }

    /// Execute all jobs necessary to build the dependency graph.
    ///
    /// This function will spawn off `config.jobs()` workers to build all of the
    /// necessary dependencies, in order. Freshness is propagated as far as
    /// possible along each dependency chain.
    pub fn execute(&mut self) -> CargoResult<()> {
        // Iteratively execute the dependency graph. Each turn of this loop will
        // schedule as much work as possible and then wait for one job to finish,
        // possibly scheduling more work afterwards.
        while self.queue.len() > 0 {
            loop {
                match self.queue.dequeue() {
                    Some((name, Fresh, (pkg, _))) => {
                        assert!(self.active.insert(name.clone(), 1u));
                        try!(self.config.shell().status("Fresh", pkg));
                        self.tx.send((name, Fresh, Ok(Vec::new())));
                    }
                    Some((name, Dirty, (pkg, job))) => {
                        assert!(self.active.insert(name.clone(), 1));
                        try!(self.config.shell().status("Compiling", pkg));
                        let my_tx = self.tx.clone();
                        self.pool.execute(proc() my_tx.send((name, Dirty, job.run())));
                    }
                    None => break,
                }
            }

            // Now that all possible work has been scheduled, wait for a piece
            // of work to finish. If any package fails to build then we stop
            // scheduling work as quickly as possibly.
            let (name, fresh, result) = self.rx.recv();
            *self.active.get_mut(&name) -= 1;
            match result {
                Ok(v) => {
                    for job in v.move_iter() {
                        *self.active.get_mut(&name) += 1;
                        let my_tx = self.tx.clone();
                        let my_name = name.clone();
                        self.pool.execute(proc() {
                            my_tx.send((my_name, fresh, job.run()));
                        });
                    }
                    if *self.active.get(&name) == 0 {
                        self.active.remove(&name);
                        self.queue.finish(&name, fresh);
                    }
                }
                Err(e) => {
                    if *self.active.get(&name) == 0 {
                        self.active.remove(&name);
                    }
                    if self.active.len() > 0 && self.config.jobs() > 1 {
                        try!(self.config.shell().say(
                                    "Build failed, waiting for other \
                                     jobs to finish...", YELLOW));
                        for _ in self.rx.iter() {}
                    }
                    return Err(e)
                }
            }
        }

        log!(5, "rustc jobs completed");

        Ok(())
    }
}
