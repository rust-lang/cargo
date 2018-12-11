use std::collections::hash_map::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::mem;
use std::process::Output;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use crossbeam_utils;
use crossbeam_utils::thread::Scope;
use jobserver::{Acquired, HelperThread};

use crate::core::profiles::Profile;
use crate::core::{PackageId, Target, TargetKind};
use crate::handle_error;
use crate::util;
use crate::util::diagnostic_server::{self, DiagnosticPrinter};
use crate::util::{internal, profile, CargoResult, CargoResultExt, ProcessBuilder};
use crate::util::{Config, DependencyQueue, Dirty, Fresh, Freshness};
use crate::util::{Progress, ProgressStyle};

use super::context::OutputFile;
use super::job::Job;
use super::{BuildContext, BuildPlan, CompileMode, Context, Kind, Unit};

/// A management structure of the entire dependency graph to compile.
///
/// This structure is backed by the `DependencyQueue` type and manages the
/// actual compilation step of each package. Packages enqueue units of work and
/// then later on the entire graph is processed and compiled.
pub struct JobQueue<'a> {
    queue: DependencyQueue<Key<'a>, Vec<(Job, Freshness, bool)>>,
    tx: Sender<Message<'a>>,
    rx: Receiver<Message<'a>>,
    active: Vec<Key<'a>>,
    pending: HashMap<Key<'a>, PendingBuild>,
    compiled: HashSet<PackageId>,
    documented: HashSet<PackageId>,
    counts: HashMap<PackageId, usize>,
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
    pkg: PackageId,
    target: &'a Target,
    profile: Profile,
    kind: Kind,
    mode: CompileMode,
}

impl<'a> Key<'a> {
    fn name_for_progress(&self) -> String {
        let pkg_name = self.pkg.name();
        match self.mode {
            CompileMode::Doc { .. } => format!("{}(doc)", pkg_name),
            CompileMode::RunCustomBuild => format!("{}(build)", pkg_name),
            _ => {
                let annotation = match self.target.kind() {
                    TargetKind::Lib(_) => return pkg_name.to_string(),
                    TargetKind::CustomBuild => return format!("{}(build.rs)", pkg_name),
                    TargetKind::Bin => "bin",
                    TargetKind::Test => "test",
                    TargetKind::Bench => "bench",
                    TargetKind::ExampleBin | TargetKind::ExampleLib(_) => "example",
                };
                format!("{}({})", self.target.name(), annotation)
            }
        }
    }
}

pub struct JobState<'a> {
    tx: Sender<Message<'a>>,
}

enum Message<'a> {
    Run(String),
    BuildPlanMsg(String, ProcessBuilder, Arc<Vec<OutputFile>>),
    Stdout(String),
    Stderr(String),
    FixDiagnostic(diagnostic_server::Message),
    Token(io::Result<Acquired>),
    Finish(Key<'a>, CargoResult<()>),
}

impl<'a> JobState<'a> {
    pub fn running(&self, cmd: &ProcessBuilder) {
        let _ = self.tx.send(Message::Run(cmd.to_string()));
    }

    pub fn build_plan(
        &self,
        module_name: String,
        cmd: ProcessBuilder,
        filenames: Arc<Vec<OutputFile>>,
    ) {
        let _ = self
            .tx
            .send(Message::BuildPlanMsg(module_name, cmd, filenames));
    }

    pub fn capture_output(
        &self,
        cmd: &ProcessBuilder,
        prefix: Option<String>,
        capture_output: bool,
    ) -> CargoResult<Output> {
        let prefix = prefix.unwrap_or_else(String::new);
        cmd.exec_with_streaming(
            &mut |out| {
                let _ = self.tx.send(Message::Stdout(format!("{}{}", prefix, out)));
                Ok(())
            },
            &mut |err| {
                let _ = self.tx.send(Message::Stderr(format!("{}{}", prefix, err)));
                Ok(())
            },
            capture_output,
        )
    }
}

impl<'a> JobQueue<'a> {
    pub fn new<'cfg>(bcx: &BuildContext<'a, 'cfg>) -> JobQueue<'a> {
        let (tx, rx) = channel();
        JobQueue {
            queue: DependencyQueue::new(),
            tx,
            rx,
            active: Vec::new(),
            pending: HashMap::new(),
            compiled: HashSet::new(),
            documented: HashSet::new(),
            counts: HashMap::new(),
            is_release: bcx.build_config.release,
        }
    }

    pub fn enqueue<'cfg>(
        &mut self,
        cx: &Context<'a, 'cfg>,
        unit: &Unit<'a>,
        job: Job,
        fresh: Freshness,
        cached: bool
    ) -> CargoResult<()> {
        let key = Key::new(unit);
        let deps = key.dependencies(cx)?;
        self.queue
            .queue(Fresh, &key, Vec::new(), &deps)
            .push((job, fresh, cached));
        *self.counts.entry(key.pkg).or_insert(0) += 1;
        Ok(())
    }

    /// Execute all jobs necessary to build the dependency graph.
    ///
    /// This function will spawn off `config.jobs()` workers to build all of the
    /// necessary dependencies, in order. Freshness is propagated as far as
    /// possible along each dependency chain.
    pub fn execute(&mut self, cx: &mut Context, plan: &mut BuildPlan) -> CargoResult<()> {
        let _p = profile::start("executing the job graph");
        self.queue.queue_finished();

        // We need to give a handle to the send half of our message queue to the
        // jobserver and (optionally) diagnostic helper thread. Unfortunately
        // though we need the handle to be `'static` as that's typically what's
        // required when spawning a thread!
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
        let tx = unsafe { mem::transmute::<Sender<Message<'a>>, Sender<Message<'static>>>(tx) };
        let tx2 = tx.clone();
        let helper = cx
            .jobserver
            .clone()
            .into_helper_thread(move |token| {
                drop(tx.send(Message::Token(token)));
            })
            .chain_err(|| "failed to create helper thread for jobserver management")?;
        let _diagnostic_server = cx
            .bcx
            .build_config
            .rustfix_diagnostic_server
            .borrow_mut()
            .take()
            .map(move |srv| srv.start(move |msg| drop(tx2.send(Message::FixDiagnostic(msg)))));

        crossbeam_utils::thread::scope(|scope| self.drain_the_queue(cx, plan, scope, &helper))
            .expect("child threads should't panic")
    }

    fn drain_the_queue(
        &mut self,
        cx: &mut Context,
        plan: &mut BuildPlan,
        scope: &Scope<'a>,
        jobserver_helper: &HelperThread,
    ) -> CargoResult<()> {
        let mut tokens = Vec::new();
        let mut queue = Vec::new();
        let build_plan = cx.bcx.build_config.build_plan;
        let mut print = DiagnosticPrinter::new(cx.bcx.config);
        trace!("queue: {:#?}", self.queue);

        // Iteratively execute the entire dependency graph. Each turn of the
        // loop starts out by scheduling as much work as possible (up to the
        // maximum number of parallel jobs we have tokens for). A local queue
        // is maintained separately from the main dependency queue as one
        // dequeue may actually dequeue quite a bit of work (e.g. 10 binaries
        // in one package).
        //
        // After a job has finished we update our internal state if it was
        // successful and otherwise wait for pending work to finish if it failed
        // and then immediately return.
        let mut error = None;
        let mut progress = Progress::with_style("Building", ProgressStyle::Ratio, cx.bcx.config);
        let total = self.queue.len();
        loop {
            // Dequeue as much work as we can, learning about everything
            // possible that can run. Note that this is also the point where we
            // start requesting job tokens. Each job after the first needs to
            // request a token.
            while let Some((fresh, key, jobs)) = self.queue.dequeue() {
                let total_fresh = jobs.iter().fold(fresh, |fresh, &(_, f, _)| f.combine(fresh));
                self.pending.insert(
                    key,
                    PendingBuild {
                        amt: jobs.len(),
                        fresh: total_fresh,
                    },
                );
                for (job, f, cached) in jobs {
                    queue.push((key, job, f.combine(fresh), cached));
                    if !self.active.is_empty() || !queue.is_empty() {
                        jobserver_helper.request_token();
                    }
                }
            }

            // Now that we've learned of all possible work that we can execute
            // try to spawn it so long as we've got a jobserver token which says
            // we're able to perform some parallel work.
            while error.is_none() && self.active.len() < tokens.len() + 1 && !queue.is_empty() {
                let (key, job, fresh, cached) = queue.remove(0);
                self.run(key, fresh, job, cx.bcx.config, scope, build_plan, cached)?;
            }

            // If after all that we're not actually running anything then we're
            // done!
            if self.active.is_empty() {
                break;
            }

            // And finally, before we block waiting for the next event, drop any
            // excess tokens we may have accidentally acquired. Due to how our
            // jobserver interface is architected we may acquire a token that we
            // don't actually use, and if this happens just relinquish it back
            // to the jobserver itself.
            tokens.truncate(self.active.len() - 1);

            let count = total - self.queue.len();
            let active_names = self
                .active
                .iter()
                .map(Key::name_for_progress)
                .collect::<Vec<_>>();
            drop(progress.tick_now(count, total, &format!(": {}", active_names.join(", "))));
            let event = self.rx.recv().unwrap();
            progress.clear();

            match event {
                Message::Run(cmd) => {
                    cx.bcx
                        .config
                        .shell()
                        .verbose(|c| c.status("Running", &cmd))?;
                }
                Message::BuildPlanMsg(module_name, cmd, filenames) => {
                    plan.update(&module_name, &cmd, &filenames)?;
                }
                Message::Stdout(out) => {
                    println!("{}", out);
                }
                Message::Stderr(err) => {
                    let mut shell = cx.bcx.config.shell();
                    shell.print_ansi(err.as_bytes())?;
                    shell.err().write_all(b"\n")?;
                }
                Message::FixDiagnostic(msg) => {
                    print.print(&msg)?;
                }
                Message::Finish(key, result) => {
                    info!("end: {:?}", key);

                    // self.active.remove_item(&key); // <- switch to this when stabilized.
                    let pos = self
                        .active
                        .iter()
                        .position(|k| *k == key)
                        .expect("an unrecorded package has finished compiling");
                    self.active.remove(pos);
                    if !self.active.is_empty() {
                        assert!(!tokens.is_empty());
                        drop(tokens.pop());
                    }
                    match result {
                        Ok(()) => self.finish(key, cx)?,
                        Err(e) => {
                            let msg = "The following warnings were emitted during compilation:";
                            self.emit_warnings(Some(msg), &key, cx)?;

                            if !self.active.is_empty() {
                                error = Some(format_err!("build failed"));
                                handle_error(&e, &mut *cx.bcx.config.shell());
                                cx.bcx.config.shell().warn(
                                    "build failed, waiting for other \
                                     jobs to finish...",
                                )?;
                            } else {
                                error = Some(e);
                            }
                        }
                    }
                }
                Message::Token(acquired_token) => {
                    tokens.push(acquired_token.chain_err(|| "failed to acquire jobserver token")?);
                }
            }
        }
        drop(progress);

        let build_type = if self.is_release { "release" } else { "dev" };
        // NOTE: This may be a bit inaccurate, since this may not display the
        // profile for what was actually built.  Profile overrides can change
        // these settings, and in some cases different targets are built with
        // different profiles.  To be accurate, it would need to collect a
        // list of Units built, and maybe display a list of the different
        // profiles used.  However, to keep it simple and compatible with old
        // behavior, we just display what the base profile is.
        let profile = cx.bcx.profiles.base_profile(self.is_release);
        let mut opt_type = String::from(if profile.opt_level.as_str() == "0" {
            "unoptimized"
        } else {
            "optimized"
        });
        if profile.debuginfo.is_some() {
            opt_type += " + debuginfo";
        }

        let time_elapsed = util::elapsed(cx.bcx.config.creation_time().elapsed());

        if self.queue.is_empty() {
            let message = format!(
                "{} [{}] target(s) in {}",
                build_type, opt_type, time_elapsed
            );
            if !build_plan {
                cx.bcx.config.shell().status("Finished", message)?;
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
    fn run(
        &mut self,
        key: Key<'a>,
        fresh: Freshness,
        job: Job,
        config: &Config,
        scope: &Scope<'a>,
        build_plan: bool,
        cached: bool
    ) -> CargoResult<()> {
        info!("start: {:?}", key);

        self.active.push(key);
        *self.counts.get_mut(&key.pkg).unwrap() -= 1;

        let my_tx = self.tx.clone();
        let doit = move || {
            let res = job.run(fresh, &JobState { tx: my_tx.clone() });
            my_tx.send(Message::Finish(key, res)).unwrap();
        };

        if !build_plan {
            // Print out some nice progress information
            self.note_working_on(config, &key, fresh, cached)?;
        }

        match fresh {
            Freshness::Fresh => doit(),
            Freshness::Dirty => {
                scope.spawn(move |_| doit());
            }
        }

        Ok(())
    }

    fn emit_warnings(&self, msg: Option<&str>, key: &Key<'a>, cx: &mut Context) -> CargoResult<()> {
        let output = cx.build_state.outputs.lock().unwrap();
        let bcx = &mut cx.bcx;
        if let Some(output) = output.get(&(key.pkg, key.kind)) {
            if let Some(msg) = msg {
                if !output.warnings.is_empty() {
                    writeln!(bcx.config.shell().err(), "{}\n", msg)?;
                }
            }

            for warning in output.warnings.iter() {
                bcx.config.shell().warn(warning)?;
            }

            if !output.warnings.is_empty() && msg.is_some() {
                // Output an empty line.
                writeln!(bcx.config.shell().err())?;
            }
        }

        Ok(())
    }

    fn finish(&mut self, key: Key<'a>, cx: &mut Context) -> CargoResult<()> {
        if key.mode.is_run_custom_build() && cx.bcx.show_warnings(key.pkg) {
            self.emit_warnings(None, &key, cx)?;
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
    fn note_working_on(
        &mut self,
        config: &Config,
        key: &Key<'a>,
        fresh: Freshness,
        cached: bool
    ) -> CargoResult<()> {
        if (self.compiled.contains(&key.pkg) && !key.mode.is_doc())
            || (self.documented.contains(&key.pkg) && key.mode.is_doc())
        {
            return Ok(());
        }

        match fresh {
            // Any dirty stage which runs at least one command gets printed as
            // being a compiled package
            Dirty => {
                if key.mode.is_doc() {
                    // Skip Doctest
                    if !key.mode.is_any_test() {
                        self.documented.insert(key.pkg);
                        config.shell().status("Documenting", key.pkg)?;
                    }
                } else {
                    self.compiled.insert(key.pkg);
                    if key.mode.is_check() {
                        config.shell().status("Checking", key.pkg)?;
                    } else {
                        if cached 
                        {
                            config.shell().status("Cached", key.pkg)?;
                        } else {
                            config.shell().status("Compiling", key.pkg)?;
                        }
                    }
                }
            }
            Fresh => {
                // If doctest is last, only print "Fresh" if nothing has been printed.
                if self.counts[&key.pkg] == 0
                    && !(key.mode == CompileMode::Doctest && self.compiled.contains(&key.pkg))
                {
                    self.compiled.insert(key.pkg);
                    config.shell().verbose(|c| c.status("Fresh", key.pkg))?;
                }
            }
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
            mode: unit.mode,
        }
    }

    fn dependencies<'cfg>(&self, cx: &Context<'a, 'cfg>) -> CargoResult<Vec<Key<'a>>> {
        let unit = Unit {
            pkg: cx.get_package(self.pkg)?,
            target: self.target,
            profile: self.profile,
            kind: self.kind,
            mode: self.mode,
        };
        let targets = cx.dep_targets(&unit);
        Ok(targets
            .iter()
            .filter_map(|unit| {
                // Binaries aren't actually needed to *compile* tests, just to run
                // them, so we don't include this dependency edge in the job graph.
                if self.target.is_test() && unit.target.is_bin() {
                    None
                } else {
                    Some(Key::new(unit))
                }
            })
            .collect())
    }
}

impl<'a> fmt::Debug for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} => {}/{} => {:?}",
            self.pkg, self.target, self.profile, self.kind
        )
    }
}
