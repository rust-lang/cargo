use std::collections::HashMap;
use std::iter::AdditiveIterator;
use term::color::YELLOW;

use core::{Package, PackageId, Resolve};
use util::{Config, TaskPool, DependencyQueue, Fresh, Dirty, Freshness};
use util::CargoResult;

use super::job::Job;

pub struct JobQueue<'a, 'b> {
    pool: TaskPool,
    queue: DependencyQueue<'a, (&'a Package, Job)>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    active: HashMap<&'a PackageId, uint>,
    config: &'b mut Config<'b>,
}

type Message = (PackageId, Freshness, CargoResult<Vec<Job>>);

impl<'a, 'b> JobQueue<'a, 'b> {
    pub fn new(config: &'b mut Config<'b>,
               resolve: &'a Resolve,
               jobs: Vec<(&'a Package, Freshness, Job)>) -> JobQueue<'a, 'b> {
        let (tx, rx) = channel();
        let mut queue = DependencyQueue::new();
        for &(pkg, _, _) in jobs.iter() {
            queue.register(pkg);
        }
        for (pkg, fresh, job) in jobs.move_iter() {
            let mut deps = resolve.deps(pkg.get_package_id())
                                  .move_iter().flat_map(|a| a);
            queue.enqueue(pkg, deps.collect(), fresh, (pkg, job));
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
                    Some((id, Fresh, (pkg, _))) => {
                        assert!(self.active.insert(id, 1u));
                        try!(self.config.shell().status("Fresh", pkg));
                        self.tx.send((id.clone(), Fresh, Ok(Vec::new())));
                    }
                    Some((id, Dirty, (pkg, job))) => {
                        assert!(self.active.insert(id, 1));
                        try!(self.config.shell().status("Compiling", pkg));
                        let my_tx = self.tx.clone();
                        let id = id.clone();
                        self.pool.execute(proc() my_tx.send((id, Dirty, job.run())));
                    }
                    None => break,
                }
            }

            // Now that all possible work has been scheduled, wait for a piece
            // of work to finish. If any package fails to build then we stop
            // scheduling work as quickly as possibly.
            let (id, fresh, result) = self.rx.recv();
            let id = self.active.iter().map(|(&k, _)| k).find(|&k| k == &id)
                         .unwrap();
            *self.active.get_mut(&id) -= 1;
            match result {
                Ok(v) => {
                    for job in v.move_iter() {
                        *self.active.get_mut(&id) += 1;
                        let my_tx = self.tx.clone();
                        let my_id = id.clone();
                        self.pool.execute(proc() {
                            my_tx.send((my_id, fresh, job.run()));
                        });
                    }
                    if *self.active.get(&id) == 0 {
                        self.active.remove(&id);
                        self.queue.finish(id, fresh);
                    }
                }
                Err(e) => {
                    if *self.active.get(&id) == 0 {
                        self.active.remove(&id);
                    }
                    if self.active.len() > 0 && self.config.jobs() > 1 {
                        try!(self.config.shell().say(
                                    "Build failed, waiting for other \
                                     jobs to finish...", YELLOW));
                        let amt = self.active.iter().map(|(_, v)| *v).sum();
                        for _ in self.rx.iter().take(amt) {}
                    }
                    return Err(e)
                }
            }
        }

        log!(5, "rustc jobs completed");

        Ok(())
    }
}
