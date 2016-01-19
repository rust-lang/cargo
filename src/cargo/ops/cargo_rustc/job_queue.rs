use std::collections::HashSet;
use std::collections::hash_map::HashMap;
use std::fmt;
use std::sync::mpsc::{channel, Sender, Receiver};

use crossbeam::{self, Scope};
use term::color::YELLOW;

use core::{PackageId, Target, Profile};
use util::{Config, DependencyQueue, Fresh, Dirty, Freshness};
use util::{CargoResult, Dependency, profile, internal};

use super::{Context, Kind, Unit};
use super::job::Job;

/// A management structure of the entire dependency graph to compile.
///
/// This structure is backed by the `DependencyQueue` type and manages the
/// actual compilation step of each package. Packages enqueue units of work and
/// then later on the entire graph is processed and compiled.
pub struct JobQueue<'a> {
    jobs: usize,
    queue: DependencyQueue<Key<'a>, Vec<(Job, Freshness)>>,
    tx: Sender<Message<'a>>,
    rx: Receiver<Message<'a>>,
    active: usize,
    pending: HashMap<Key<'a>, PendingBuild>,
    compiled: HashSet<&'a PackageId>,
    documented: HashSet<&'a PackageId>,
    counts: HashMap<&'a PackageId, usize>,
}

/// A helper structure for metadata about the state of a building package.
struct PendingBuild {
    /// Number of jobs currently active
    amt: usize,
    /// Current freshness state of this package. Any dirty target within a
    /// package will cause the entire package to become dirty.
    fresh: Freshness,
}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct Key<'a> {
    pkg: &'a PackageId,
    target: &'a Target,
    profile: &'a Profile,
    kind: Kind,
}

struct Message<'a> {
    key: Key<'a>,
    result: CargoResult<()>,
}

impl<'a> JobQueue<'a> {
    pub fn new<'cfg>(cx: &Context<'a, 'cfg>) -> JobQueue<'a> {
        let (tx, rx) = channel();
        JobQueue {
            jobs: cx.jobs() as usize,
            queue: DependencyQueue::new(),
            tx: tx,
            rx: rx,
            active: 0,
            pending: HashMap::new(),
            compiled: HashSet::new(),
            documented: HashSet::new(),
            counts: HashMap::new(),
        }
    }

    pub fn enqueue(&mut self, cx: &Context<'a, 'a>,
                   unit: &Unit<'a>, job: Job, fresh: Freshness) {
        let key = Key::new(unit);
        self.queue.queue(cx, Fresh, key, Vec::new()).push((job, fresh));
        *self.counts.entry(key.pkg).or_insert(0) += 1;
    }

    /// Execute all jobs necessary to build the dependency graph.
    ///
    /// This function will spawn off `config.jobs()` workers to build all of the
    /// necessary dependencies, in order. Freshness is propagated as far as
    /// possible along each dependency chain.
    pub fn execute(&mut self, config: &Config) -> CargoResult<()> {
        let _p = profile::start("executing the job graph");

        crossbeam::scope(|scope| {
            self.drain_the_queue(config, scope)
        })
    }

    fn drain_the_queue(&mut self, config: &Config, scope: &Scope<'a>)
                       -> CargoResult<()> {
        let mut queue = Vec::new();
        trace!("queue: {:#?}", self.queue);

        // Iteratively execute the entire dependency graph. Each turn of the
        // loop starts out by scheduling as much work as possible (up to the
        // maximum number of parallel jobs). A local queue is maintained
        // separately from the main dependency queue as one dequeue may actually
        // dequeue quite a bit of work (e.g. 10 binaries in one project).
        //
        // After a job has finished we update our internal state if it was
        // successful and otherwise wait for pending work to finish if it failed
        // and then immediately return.
        loop {
            while self.active < self.jobs {
                if !queue.is_empty() {
                    let (key, job, fresh) = queue.remove(0);
                    try!(self.run(key, fresh, job, config, scope));
                } else if let Some((fresh, key, jobs)) = self.queue.dequeue() {
                    let total_fresh = jobs.iter().fold(fresh, |fresh, &(_, f)| {
                        f.combine(fresh)
                    });
                    self.pending.insert(key, PendingBuild {
                        amt: jobs.len(),
                        fresh: total_fresh,
                    });
                    queue.extend(jobs.into_iter().map(|(job, f)| {
                        (key, job, f.combine(fresh))
                    }));
                } else {
                    break
                }
            }
            if self.active == 0 {
                break
            }

            // Now that all possible work has been scheduled, wait for a piece
            // of work to finish. If any package fails to build then we stop
            // scheduling work as quickly as possibly.
            let msg = self.rx.recv().unwrap();
            info!("end: {:?}", msg.key);
            self.active -= 1;
            match msg.result {
                Ok(()) => {
                    let state = self.pending.get_mut(&msg.key).unwrap();
                    state.amt -= 1;
                    if state.amt == 0 {
                        self.queue.finish(&msg.key, state.fresh);
                    }
                }
                Err(e) => {
                    if self.active > 0 {
                        try!(config.shell().say(
                                    "Build failed, waiting for other \
                                     jobs to finish...", YELLOW));
                        for _ in self.rx.iter().take(self.active as usize) {}
                    }
                    return Err(e)
                }
            }
        }

        if self.queue.is_empty() {
            Ok(())
        } else {
            debug!("queue: {:#?}", self.queue);
            Err(internal("finished with jobs still left in the queue"))
        }
    }

    /// Executes a job in the `scope` given, pushing the spawned thread's
    /// handled onto `threads`.
    fn run(&mut self,
           key: Key<'a>,
           fresh: Freshness,
           job: Job,
           config: &Config,
           scope: &Scope<'a>) -> CargoResult<()> {
        info!("start: {:?}", key);

        self.active += 1;
        *self.counts.get_mut(key.pkg).unwrap() -= 1;

        let my_tx = self.tx.clone();
        let (desc_tx, desc_rx) = channel();
        scope.spawn(move || {
            my_tx.send(Message {
                key: key,
                result: job.run(fresh, desc_tx),
            }).unwrap();
        });

        // Print out some nice progress information
        try!(self.note_working_on(config, &key, fresh));

        // only the first message of each job is processed
        if let Ok(msg) = desc_rx.recv() {
            try!(config.shell().verbose(|c| c.status("Running", &msg)));
        }
        Ok(())
    }

    // This isn't super trivial because we don't want to print loads and
    // loads of information to the console, but we also want to produce a
    // faithful representation of what's happening. This is somewhat nuanced
    // as a package can start compiling *very* early on because of custom
    // build commands and such.
    //
    // In general, we try to print "Compiling" for the first nontrivial task
    // run for a package, regardless of when that is. We then don't print
    // out any more information for a package after we've printed it once.
    fn note_working_on(&mut self, config: &Config, key: &Key<'a>,
                       fresh: Freshness) -> CargoResult<()> {
        if (self.compiled.contains(key.pkg) && !key.profile.doc) ||
            (self.documented.contains(key.pkg) && key.profile.doc) {
            return Ok(())
        }

        match fresh {
            // Any dirty stage which runs at least one command gets printed as
            // being a compiled package
            Dirty => {
                if key.profile.doc {
                    self.documented.insert(key.pkg);
                    try!(config.shell().status("Documenting", key.pkg));
                } else {
                    self.compiled.insert(key.pkg);
                    try!(config.shell().status("Compiling", key.pkg));
                }
            }
            Fresh if self.counts[key.pkg] == 0 => {
                self.compiled.insert(key.pkg);
                try!(config.shell().verbose(|c| c.status("Fresh", key.pkg)));
            }
            Fresh => {}
        }
        Ok(())
    }
}

impl<'a> Dependency for Key<'a> {
    type Context = Context<'a, 'a>;

    fn dependencies(&self, cx: &Context<'a, 'a>) -> Vec<Key<'a>> {
        let unit = Unit {
            pkg: cx.get_package(self.pkg),
            target: self.target,
            profile: self.profile,
            kind: self.kind,
        };
        cx.dep_targets(&unit).iter().filter_map(|unit| {
            // Binaries aren't actually needed to *compile* tests, just to run
            // them, so we don't include this dependency edge in the job graph.
            if self.target.is_test() && unit.target.is_bin() {
                None
            } else {
                Some(Key::new(unit))
            }
        }).collect()
    }
}

impl<'a> Key<'a> {
    fn new(unit: &Unit<'a>) -> Key<'a> {
        Key {
            pkg: unit.pkg.package_id(),
            target: unit.target,
            profile: unit.profile,
            kind: unit.kind,
        }
    }
}

impl<'a> fmt::Debug for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} => {}/{} => {:?}", self.pkg, self.target, self.profile,
               self.kind)
    }
}
