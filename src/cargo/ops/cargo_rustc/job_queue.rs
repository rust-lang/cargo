use std::collections::HashSet;
use std::collections::hash_map::HashMap;
use std::fmt;
use std::io::Write;
use std::sync::mpsc::{channel, Sender, Receiver};

use crossbeam::{self, Scope};
use term::color::YELLOW;

use core::{PackageId, Target, Profile};
use util::{Config, DependencyQueue, Fresh, Dirty, Freshness};
use util::{CargoResult, ProcessBuilder, profile, internal};

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
    tx: Sender<(Key<'a>, Message)>,
    rx: Receiver<(Key<'a>, Message)>,
    active: usize,
    pending: HashMap<Key<'a>, PendingBuild>,
    compiled: HashSet<&'a PackageId>,
    documented: HashSet<&'a PackageId>,
    counts: HashMap<&'a PackageId, usize>,
    is_release: bool,
    is_doc_all: bool,
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

pub struct JobState<'a> {
    tx: Sender<(Key<'a>, Message)>,
    key: Key<'a>,
}

enum Message {
    Run(String),
    Stdout(String),
    Stderr(String),
    Finish(CargoResult<()>),
}

impl<'a> JobState<'a> {
    pub fn running(&self, cmd: &ProcessBuilder) {
        let _ = self.tx.send((self.key, Message::Run(cmd.to_string())));
    }

    pub fn stdout(&self, out: &str) {
        let _ = self.tx.send((self.key, Message::Stdout(out.to_string())));
    }

    pub fn stderr(&self, err: &str) {
        let _ = self.tx.send((self.key, Message::Stderr(err.to_string())));
    }
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
            is_release: cx.build_config.release,
            is_doc_all: cx.build_config.doc_all,
        }
    }

    pub fn enqueue<'cfg>(&mut self,
                         cx: &Context<'a, 'cfg>,
                         unit: &Unit<'a>,
                         job: Job,
                         fresh: Freshness) -> CargoResult<()> {
        let key = Key::new(unit);
        let deps = key.dependencies(cx)?;
        self.queue.queue(Fresh, key, Vec::new(), &deps).push((job, fresh));
        *self.counts.entry(key.pkg).or_insert(0) += 1;
        Ok(())
    }

    /// Execute all jobs necessary to build the dependency graph.
    ///
    /// This function will spawn off `config.jobs()` workers to build all of the
    /// necessary dependencies, in order. Freshness is propagated as far as
    /// possible along each dependency chain.
    pub fn execute(&mut self, cx: &mut Context) -> CargoResult<()> {
        let _p = profile::start("executing the job graph");

        crossbeam::scope(|scope| {
            self.drain_the_queue(cx, scope)
        })
    }

    fn drain_the_queue(&mut self, cx: &mut Context, scope: &Scope<'a>)
                       -> CargoResult<()> {
        use std::time::Instant;

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
        let mut error = None;
        let start_time = Instant::now();
        loop {
            while error.is_none() && self.active < self.jobs {
                if !queue.is_empty() {
                    let (key, job, fresh) = queue.remove(0);
                    self.run(key, fresh, job, cx.config, scope)?;
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

            let (key, msg) = self.rx.recv().unwrap();

            match msg {
                Message::Run(cmd) => {
                    cx.config.shell().verbose(|c| c.status("Running", &cmd))?;
                }
                Message::Stdout(out) => {
                    if cx.config.extra_verbose() {
                        writeln!(cx.config.shell().out(), "{}", out)?;
                    }
                }
                Message::Stderr(err) => {
                    if cx.config.extra_verbose() {
                        writeln!(cx.config.shell().err(), "{}", err)?;
                    }
                }
                Message::Finish(result) => {
                    info!("end: {:?}", key);
                    self.active -= 1;
                    match result {
                        Ok(()) => self.finish(key, cx)?,
                        Err(e) => {
                            if self.active > 0 {
                                cx.config.shell().say(
                                            "Build failed, waiting for other \
                                             jobs to finish...", YELLOW)?;
                            }
                            if error.is_none() {
                                error = Some(e);
                            }
                        }
                    }
                }
            }
        }

        let build_type = if self.is_release { "release" } else { "debug" };
        let profile = cx.lib_profile();
        let mut opt_type = String::from(if profile.opt_level == "0" { "unoptimized" }
                                        else { "optimized" });
        if profile.debuginfo {
            opt_type = opt_type + " + debuginfo";
        }
        let duration = start_time.elapsed();
        let time_elapsed = format!("{}.{1:.2} secs",
                                   duration.as_secs(),
                                   duration.subsec_nanos() / 10000000);
        if self.queue.is_empty() {
            if !self.is_doc_all {
                cx.config.shell().status("Finished", format!("{} [{}] target(s) in {}",
                                                                  build_type,
                                                                  opt_type,
                                                                  time_elapsed))?;
            }
            Ok(())
        } else if let Some(e) = error {
            Err(e)
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
        scope.spawn(move || {
            let res = job.run(fresh, &JobState {
                tx: my_tx.clone(),
                key: key,
            });
            my_tx.send((key, Message::Finish(res))).unwrap();
        });

        // Print out some nice progress information
        self.note_working_on(config, &key, fresh)?;

        Ok(())
    }

    fn finish(&mut self, key: Key<'a>, cx: &mut Context) -> CargoResult<()> {
        if key.profile.run_custom_build && cx.show_warnings(key.pkg) {
            let output = cx.build_state.outputs.lock().unwrap();
            if let Some(output) = output.get(&(key.pkg.clone(), key.kind)) {
                for warning in output.warnings.iter() {
                    cx.config.shell().warn(warning)?;
                }
            }
        }
        let state = self.pending.get_mut(&key).unwrap();
        state.amt -= 1;
        if state.amt == 0 {
            self.queue.finish(&key, state.fresh);
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
    fn note_working_on(&mut self,
                       config: &Config,
                       key: &Key<'a>,
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
                    config.shell().status("Documenting", key.pkg)?;
                } else {
                    self.compiled.insert(key.pkg);
                    config.shell().status("Compiling", key.pkg)?;
                }
            }
            Fresh if self.counts[key.pkg] == 0 => {
                self.compiled.insert(key.pkg);
                config.shell().verbose(|c| c.status("Fresh", key.pkg))?;
            }
            Fresh => {}
        }
        Ok(())
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

    fn dependencies<'cfg>(&self, cx: &Context<'a, 'cfg>)
                          -> CargoResult<Vec<Key<'a>>> {
        let unit = Unit {
            pkg: cx.get_package(self.pkg)?,
            target: self.target,
            profile: self.profile,
            kind: self.kind,
        };
        let targets = cx.dep_targets(&unit)?;
        Ok(targets.iter().filter_map(|unit| {
            // Binaries aren't actually needed to *compile* tests, just to run
            // them, so we don't include this dependency edge in the job graph.
            if self.target.is_test() && unit.target.is_bin() {
                None
            } else {
                Some(Key::new(unit))
            }
        }).collect())
    }
}

impl<'a> fmt::Debug for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} => {}/{} => {:?}", self.pkg, self.target, self.profile,
               self.kind)
    }
}
