use std::collections::HashSet;
use std::collections::hash_map::{HashMap, Occupied, Vacant};
use std::sync::TaskPool;
use term::color::YELLOW;

use core::{Package, PackageId, Resolve, PackageSet};
use util::{Config, DependencyQueue, Fresh, Dirty, Freshness};
use util::{CargoResult, Dependency, profile};

use super::job::Job;

/// A management structure of the entire dependency graph to compile.
///
/// This structure is backed by the `DependencyQueue` type and manages the
/// actual compilation step of each package. Packages enqueue units of work and
/// then later on the entire graph is processed and compiled.
pub struct JobQueue<'a, 'b> {
    pool: TaskPool,
    queue: DependencyQueue<(&'a PackageId, Stage),
                           (&'a Package, Vec<(Job, Freshness)>)>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    resolve: &'a Resolve,
    packages: &'a PackageSet,
    active: uint,
    pending: HashMap<(&'a PackageId, Stage), PendingBuild>,
    state: HashMap<&'a PackageId, Freshness>,
    ignored: HashSet<&'a PackageId>,
    printed: HashSet<&'a PackageId>,
}

/// A helper structure for metadata about the state of a building package.
struct PendingBuild {
    /// Number of jobs currently active
    amt: uint,
    /// Current freshness state of this package. Any dirty target within a
    /// package will cause the entire package to become dirty.
    fresh: Freshness,
}

/// Current stage of compilation for an individual package.
///
/// This is the second layer of keys on the dependency queue to track the state
/// of where a particular package is in the compilation pipeline. Each of these
/// stages has a network of dependencies among them, outlined by the
/// `Dependency` implementation found below.
///
/// Each build step for a package is registered with one of these stages, and
/// each stage has a vector of work to perform in parallel.
#[deriving(Hash, PartialEq, Eq, Clone, PartialOrd, Ord, Show)]
pub enum Stage {
    Start,
    BuildCustomBuild,
    RunCustomBuild,
    Libraries,
    Binaries,
    LibraryTests,
    BinaryTests,
}

type Message = (PackageId, Stage, Freshness, CargoResult<()>);

impl<'a, 'b> JobQueue<'a, 'b> {
    pub fn new(resolve: &'a Resolve, packages: &'a PackageSet,
               config: &Config) -> JobQueue<'a, 'b> {
        let (tx, rx) = channel();
        JobQueue {
            pool: TaskPool::new(config.jobs()),
            queue: DependencyQueue::new(),
            tx: tx,
            rx: rx,
            resolve: resolve,
            packages: packages,
            active: 0,
            pending: HashMap::new(),
            state: HashMap::new(),
            ignored: HashSet::new(),
            printed: HashSet::new(),
        }
    }

    pub fn enqueue(&mut self, pkg: &'a Package, stage: Stage,
                   jobs: Vec<(Job, Freshness)>) {
        // Record the freshness state of this package as dirty if any job is
        // dirty or fresh otherwise
        let fresh = jobs.iter().fold(Fresh, |f1, &(_, f2)| f1.combine(f2));
        match self.state.entry(pkg.get_package_id()) {
            Occupied(mut entry) => { *entry.get_mut() = entry.get().combine(fresh); }
            Vacant(entry) => { entry.set(fresh); }
        };

        // Add the package to the dependency graph
        self.queue.enqueue(&(self.resolve, self.packages), Fresh,
                           (pkg.get_package_id(), stage),
                           (pkg, jobs));
    }

    pub fn ignore(&mut self, pkg: &'a Package) {
        self.ignored.insert(pkg.get_package_id());
    }

    /// Execute all jobs necessary to build the dependency graph.
    ///
    /// This function will spawn off `config.jobs()` workers to build all of the
    /// necessary dependencies, in order. Freshness is propagated as far as
    /// possible along each dependency chain.
    pub fn execute(&mut self, config: &Config) -> CargoResult<()> {
        let _p = profile::start("executing the job graph");

        // Iteratively execute the dependency graph. Each turn of this loop will
        // schedule as much work as possible and then wait for one job to finish,
        // possibly scheduling more work afterwards.
        while self.queue.len() > 0 {
            loop {
                match self.queue.dequeue() {
                    Some((fresh, (_, stage), (pkg, jobs))) => {
                        info!("start: {} {}", pkg, stage);
                        try!(self.run(pkg, stage, fresh, jobs, config));
                    }
                    None => break,
                }
            }

            // Now that all possible work has been scheduled, wait for a piece
            // of work to finish. If any package fails to build then we stop
            // scheduling work as quickly as possibly.
            let (id, stage, fresh, result) = self.rx.recv();
            info!("  end: {} {}", id, stage);
            let id = *self.state.keys().find(|&k| *k == &id).unwrap();
            self.active -= 1;
            match result {
                Ok(()) => {
                    let state = &mut self.pending[(id, stage)];
                    state.amt -= 1;
                    state.fresh = state.fresh.combine(fresh);
                    if state.amt == 0 {
                        self.queue.finish(&(id, stage), state.fresh);
                    }
                }
                Err(e) => {
                    if self.active > 0 {
                        try!(config.shell().say(
                                    "Build failed, waiting for other \
                                     jobs to finish...", YELLOW));
                        for _ in self.rx.iter().take(self.active) {}
                    }
                    return Err(e)
                }
            }
        }

        log!(5, "rustc jobs completed");

        Ok(())
    }

    /// Execute a stage of compilation for a package.
    ///
    /// The input freshness is from `dequeue()` and indicates the combined
    /// freshness of all upstream dependencies. This function will schedule all
    /// work in `jobs` to be executed.
    fn run(&mut self, pkg: &'a Package, stage: Stage, fresh: Freshness,
           jobs: Vec<(Job, Freshness)>, config: &Config) -> CargoResult<()> {
        let njobs = jobs.len();
        let amt = if njobs == 0 {1} else {njobs};
        let id = pkg.get_package_id().clone();

        // While the jobs are all running, we maintain some metadata about how
        // many are running, the current state of freshness (of all the combined
        // jobs), and the stage to pass to finish() later on.
        self.active += amt;
        self.pending.insert((pkg.get_package_id(), stage), PendingBuild {
            amt: amt,
            fresh: fresh,
        });

        let mut total_fresh = fresh.combine(self.state[pkg.get_package_id()]);
        let mut running = Vec::new();
        for (job, job_freshness) in jobs.into_iter() {
            let fresh = job_freshness.combine(fresh);
            total_fresh = total_fresh.combine(fresh);
            let my_tx = self.tx.clone();
            let id = id.clone();
            let (desc_tx, desc_rx) = channel();
            self.pool.execute(proc() {
                my_tx.send((id, stage, fresh, job.run(fresh, desc_tx)));
            });
            // only the first message of each job is processed
            match desc_rx.recv_opt() {
                Ok(msg) => running.push(msg),
                Err(..) => {}
            }
        }

        // If no work was scheduled, make sure that a message is actually send
        // on this channel.
        if njobs == 0 {
            self.tx.send((id, stage, fresh, Ok(())));
        }

        // Print out some nice progress information
        //
        // This isn't super trivial becuase we don't want to print loads and
        // loads of information to the console, but we also want to produce a
        // faithful representation of what's happening. This is somewhat nuanced
        // as a package can start compiling *very* early on because of custom
        // build commands and such.
        //
        // In general, we try to print "Compiling" for the first nontrivial task
        // run for a package, regardless of when that is. We then don't print
        // out any more information for a package after we've printed it once.
        let print = !self.ignored.contains(&pkg.get_package_id());
        let print = print && !self.printed.contains(&pkg.get_package_id());
        if print && (stage == Stage::Libraries ||
                     (total_fresh == Dirty && running.len() > 0)) {
            self.printed.insert(pkg.get_package_id());
            match total_fresh {
                Fresh => try!(config.shell().verbose(|c| {
                    c.status("Fresh", pkg)
                })),
                Dirty => try!(config.shell().status("Compiling", pkg))
            }
        }
        for msg in running.iter() {
            try!(config.shell().verbose(|c| c.status("Running", msg)));
        }
        Ok(())
    }
}

impl<'a> Dependency<(&'a Resolve, &'a PackageSet)>
    for (&'a PackageId, Stage)
{
    fn dependencies(&self, &(resolve, packages): &(&'a Resolve, &'a PackageSet))
                    -> Vec<(&'a PackageId, Stage)> {
        // This implementation of `Dependency` is the driver for the structure
        // of the dependency graph of packages to be built. The "key" here is
        // a pair of the package being built and the stage that it's at.
        //
        // Each stage here lists dependencies on the previous stages except for
        // the start state which depends on the ending state of all dependent
        // packages (as determined by the resolve context).
        let (id, stage) = *self;
        let pkg = packages.iter().find(|p| p.get_package_id() == id).unwrap();
        let deps = resolve.deps(id).into_iter().flat_map(|a| a)
                          .filter(|dep| *dep != id)
                          .map(|dep| {
                              (dep, pkg.get_dependencies().iter().find(|d| {
                                  d.get_name() == dep.get_name()
                              }).unwrap())
                          });
        match stage {
            Stage::Start => Vec::new(),

            // Building the build command itself starts off pretty easily,we
            // just need to depend on all of the library stages of our own build
            // dependencies (making them available to us).
            Stage::BuildCustomBuild => {
                let mut base = vec![(id, Stage::Start)];
                base.extend(deps.filter(|&(_, dep)| dep.is_build())
                                .map(|(id, _)| (id, Stage::Libraries)));
                base
            }

            // When running a custom build command, we need to be sure that our
            // own custom build command is actually built, and then we need to
            // wait for all our dependencies to finish their custom build
            // commands themselves (as they may provide input to us).
            Stage::RunCustomBuild => {
                let mut base = vec![(id, Stage::BuildCustomBuild)];
                base.extend(deps.filter(|&(_, dep)| dep.is_transitive())
                                .map(|(id, _)| (id, Stage::RunCustomBuild)));
                base
            }

            // Building a library depends on our own custom build command plus
            // all our transitive dependencies.
            Stage::Libraries => {
                let mut base = vec![(id, Stage::RunCustomBuild)];
                base.extend(deps.filter(|&(_, dep)| dep.is_transitive())
                                .map(|(id, _)| (id, Stage::Libraries)));
                base
            }

            // Binaries only depend on libraries being available. Note that they
            // do not depend on dev-dependencies.
            Stage::Binaries => vec![(id, Stage::Libraries)],

            // Tests depend on all dependencies (including dev-dependencies) in
            // addition to the library stage for this package. Note, however,
            // that library tests only need to depend the custom build command
            // being run, not the libraries themselves.
            Stage::BinaryTests | Stage::LibraryTests => {
                let mut base = if stage == Stage::BinaryTests {
                    vec![(id, Stage::Libraries)]
                } else {
                    vec![(id, Stage::RunCustomBuild)]
                };
                base.extend(deps.map(|(id, _)| (id, Stage::Libraries)));
                base
            }
        }
    }
}
