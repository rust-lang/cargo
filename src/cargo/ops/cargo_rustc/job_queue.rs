use std::collections::HashSet;
use std::collections::hash_map::HashMap;
use std::fmt;
use std::io;
use std::mem;
use std::sync::mpsc::{channel, Sender, Receiver};

use crossbeam::{self, Scope};
use jobserver::{Acquired, HelperThread};

use core::{PackageId, Target, Profile};
use util::{Config, DependencyQueue, Fresh, Dirty, Freshness};
use util::{CargoResult, ProcessBuilder, profile, internal, CargoResultExt};
use {handle_error};

use super::{Context, Kind, Unit};
use super::job::Job;

/// A management structure of the entire dependency graph to compile.
///
/// This structure is backed by the `DependencyQueue` type and manages the
/// actual compilation step of each package. Packages enqueue units of work and
/// then later on the entire graph is processed and compiled.
pub struct JobQueue<'a> {
    queue: DependencyQueue<Key<'a>, Vec<(Job, Freshness)>>,
    tx: Sender<Message<'a>>,
    rx: Receiver<Message<'a>>,
    active: usize,
    pending: HashMap<Key<'a>, PendingBuild>,
    compiled: HashSet<&'a PackageId>,
    documented: HashSet<&'a PackageId>,
    counts: HashMap<&'a PackageId, usize>,
    is_release: bool,
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
    tx: Sender<Message<'a>>,
}

enum Message<'a> {
    Run(String),
    Stdout(String),
    Stderr(String),
    Token(io::Result<Acquired>),
    Finish(Key<'a>, CargoResult<()>),
}

impl<'a> JobState<'a> {
    pub fn running(&self, cmd: &ProcessBuilder) {
        let _ = self.tx.send(Message::Run(cmd.to_string()));
    }

    pub fn stdout(&self, out: &str) {
        let _ = self.tx.send(Message::Stdout(out.to_string()));
    }

    pub fn stderr(&self, err: &str) {
        let _ = self.tx.send(Message::Stderr(err.to_string()));
    }
}

impl<'a> JobQueue<'a> {
    pub fn new<'cfg>(cx: &Context<'a, 'cfg>) -> JobQueue<'a> {
        let (tx, rx) = channel();
        JobQueue {
            queue: DependencyQueue::new(),
            tx: tx,
            rx: rx,
            active: 0,
            pending: HashMap::new(),
            compiled: HashSet::new(),
            documented: HashSet::new(),
            counts: HashMap::new(),
            is_release: cx.build_config.release,
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

        // We need to give a handle to the send half of our message queue to the
        // jobserver helper thread. Unfortunately though we need the handle to be
        // `'static` as that's typically what's required when spawning a
        // thread!
        //
        // To work around this we transmute the `Sender` to a static lifetime.
        // we're only sending "longer living" messages and we should also
        // destroy all references to the channel before this function exits as
        // the destructor for the `helper` object will ensure the associated
        // thread is no longer running.
        //
        // As a result, this `transmute` to a longer lifetime should be safe in
        // practice.
        let tx = self.tx.clone();
        let tx = unsafe {
            mem::transmute::<Sender<Message<'a>>, Sender<Message<'static>>>(tx)
        };
        let helper = cx.jobserver.clone().into_helper_thread(move |token| {
            drop(tx.send(Message::Token(token)));
        }).chain_err(|| {
            "failed to create helper thread for jobserver management"
        })?;

        crossbeam::scope(|scope| {
            self.drain_the_queue(cx, scope, &helper)
        })
    }

    fn drain_the_queue(&mut self,
                       cx: &mut Context,
                       scope: &Scope<'a>,
                       jobserver_helper: &HelperThread)
                       -> CargoResult<()> {
        use std::time::Instant;

        let mut tokens = Vec::new();
        let mut queue = Vec::new();
        trace!("queue: {:#?}", self.queue);

        // Iteratively execute the entire dependency graph. Each turn of the
        // loop starts out by scheduling as much work as possible (up to the
        // maximum number of parallel jobs we have tokens for). A local queue
        // is maintained separately from the main dependency queue as one
        // dequeue may actually dequeue quite a bit of work (e.g. 10 binaries
        // in one project).
        //
        // After a job has finished we update our internal state if it was
        // successful and otherwise wait for pending work to finish if it failed
        // and then immediately return.
        let mut error = None;
        let start_time = Instant::now();
        loop {
            // Dequeue as much work as we can, learning about everything
            // possible that can run. Note that this is also the point where we
            // start requesting job tokens. Each job after the first needs to
            // request a token.
            while let Some((fresh, key, jobs)) = self.queue.dequeue() {
                let total_fresh = jobs.iter().fold(fresh, |fresh, &(_, f)| {
                    f.combine(fresh)
                });
                self.pending.insert(key, PendingBuild {
                    amt: jobs.len(),
                    fresh: total_fresh,
                });
                for (job, f) in jobs {
                    queue.push((key, job, f.combine(fresh)));
                    if self.active + queue.len() > 0 {
                        jobserver_helper.request_token();
                    }
                }
            }

            // Now that we've learned of all possible work that we can execute
            // try to spawn it so long as we've got a jobserver token which says
            // we're able to perform some parallel work.
            while error.is_none() && self.active < tokens.len() + 1 && !queue.is_empty() {
                let (key, job, fresh) = queue.remove(0);
                self.run(key, fresh, job, cx.config, scope)?;
            }

            // If after all that we're not actually running anything then we're
            // done!
            if self.active == 0 {
                break
            }

            // And finally, before we block waiting for the next event, drop any
            // excess tokens we may have accidentally acquired. Due to how our
            // jobserver interface is architected we may acquire a token that we
            // don't actually use, and if this happens just relinquish it back
            // to the jobserver itself.
            tokens.truncate(self.active - 1);

            match self.rx.recv().unwrap() {
                Message::Run(cmd) => {
                    cx.config.shell().verbose(|c| c.status("Running", &cmd))?;
                }
                Message::Stdout(out) => {
                    if cx.config.extra_verbose() {
                        println!("{}", out);
                    }
                }
                Message::Stderr(err) => {
                    if cx.config.extra_verbose() {
                        writeln!(cx.config.shell().err(), "{}", err)?;
                    }
                }
                Message::Finish(key, result) => {
                    info!("end: {:?}", key);
                    self.active -= 1;
                    if self.active > 0 {
                        assert!(tokens.len() > 0);
                        drop(tokens.pop());
                    }
                    match result {
                        Ok(()) => self.finish(key, cx)?,
                        Err(e) => {
                            let msg = "The following warnings were emitted during compilation:";
                            self.emit_warnings(Some(msg), key, cx)?;

                            if self.active > 0 {
                                error = Some("build failed".into());
                                handle_error(e, &mut *cx.config.shell());
                                cx.config.shell().warn(
                                            "build failed, waiting for other \
                                             jobs to finish...")?;
                            }
                            else {
                                error = Some(e);
                            }
                        }
                    }
                }
                Message::Token(acquired_token) => {
                    tokens.push(acquired_token.chain_err(|| {
                        "failed to acquire jobserver token"
                    })?);
                }
            }
        }

        let build_type = if self.is_release { "release" } else { "dev" };
        let profile = cx.lib_profile();
        let mut opt_type = String::from(if profile.opt_level == "0" { "unoptimized" }
                                        else { "optimized" });
        if profile.debuginfo.is_some() {
            opt_type = opt_type + " + debuginfo";
        }
        let duration = start_time.elapsed();
        let time_elapsed = format!("{}.{1:.2} secs",
                                   duration.as_secs(),
                                   duration.subsec_nanos() / 10000000);
        if self.queue.is_empty() {
            let message = format!("{} [{}] target(s) in {}",
                                  build_type,
                                  opt_type,
                                  time_elapsed);
            cx.config.shell().status("Finished", message)?;
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
        let doit = move || {
            let res = job.run(fresh, &JobState {
                tx: my_tx.clone(),
            });
            my_tx.send(Message::Finish(key, res)).unwrap();
        };
        match fresh {
            Freshness::Fresh => doit(),
            Freshness::Dirty => { scope.spawn(doit); }
        }

        // Print out some nice progress information
        self.note_working_on(config, &key, fresh)?;

        Ok(())
    }

    fn emit_warnings(&self, msg: Option<&str>, key: Key<'a>, cx: &mut Context) -> CargoResult<()> {
        let output = cx.build_state.outputs.lock().unwrap();
        if let Some(output) = output.get(&(key.pkg.clone(), key.kind)) {
            if let Some(msg) = msg {
                if !output.warnings.is_empty() {
                    writeln!(cx.config.shell().err(), "{}\n", msg)?;
                }
            }

            for warning in output.warnings.iter() {
                cx.config.shell().warn(warning)?;
            }

            if !output.warnings.is_empty() && msg.is_some() {
                // Output an empty line.
                writeln!(cx.config.shell().err(), "")?;
            }
        }

        Ok(())
    }

    fn finish(&mut self, key: Key<'a>, cx: &mut Context) -> CargoResult<()> {
        if key.profile.run_custom_build && cx.show_warnings(key.pkg) {
            self.emit_warnings(None, key, cx)?;
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
                    if !key.profile.test {
                        self.documented.insert(key.pkg);
                        config.shell().status("Documenting", key.pkg)?;
                    }
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
