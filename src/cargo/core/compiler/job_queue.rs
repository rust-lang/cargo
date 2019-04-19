//! A job queue for executing Cargo's dependency graph
//!
//! This module primarily contains a `JobQueue` type which is the source of
//! executing Cargo's jobs in parallel. This is the module where we actually
//! execute rustc and the job graph. The internal `DependencyQueue` is
//! constructed elsewhere in Cargo, and the job of this module is to simply
//! execute what's described internally in the `DependencyQueue` as fast as it
//! can (using configured parallelism settings).
//!
//! Note that this is also the primary module where jobserver management comes
//! into play because that governs how many jobs we can execute in parallel.
//! Additionally of note is that everything here is using blocking/synchronous
//! I/O and such. We spawn a thread per rustc instance and it all coordinates
//! back to the main Cargo thread which coordinates everything. Perhaps one day
//! we can use async I/O, but it's not super pressing right now.
//!
//! ## Pipelining
//!
//! Another major feature of this module is the execution of a pipelined job
//! graph. The term "pipelining" here refers to executing rustc in an overlapped
//! fashion. We can build rlibs, for example, as soon as their dependencies have
//! metadata files available. No need to wait for codegen!
//!
//! Pipelined execution is modeled in Cargo with a few properties:
//!
//! * The `DependencyQueue` dependency graph has two nodes for each rlib. One
//!   node is a `BuildRmeta` node while the other is a `Build` node (which
//!   depends on `BuildRmeta`). This is prepared elsewhere.
//!
//! * Each node in a `DependencyQueue` is associated with a unit of work to
//!   complete that node. In the case of pipelined compilation the `BuildRmeta`
//!   node's work actually performs the full compile for both `BuildRmeta` and
//!   `Build`. The work associated with `Build` is just some bookkeeping that
//!   doesn't do any actual work.
//!
//! * The execution of a `BuildRmeta` unit will, before the end of the work,
//!   send a message through `JobState` that the metadata file is ready. Later,
//!   when it's fully finished, it will flag that the `Build` work is ready.
//!
//! All of this is somewhat wonky and doesn't fit perfectly into the
//! `JobQueue`'s management of all other sorts of jobs, but it looks like it
//! generally works ok below. The strategy is that we specially handle pipelined
//! units in the `run` and `finish` methods, but ideally not many other places.
//!
//! When a `BuildRmeta` unit is executed we'll arrange for the the correct
//! messages to be sent. A `Finish` for the `BuildRmeta` will be sent ASAP as
//! the work indicates, and then `Finish` for the `Build` will be sent
//! afterwards. This means that running a `BuildRmeta` unit allocates some
//! metadata for the `Build` unit as well.
//!
//! When a `BuildRmeta` unit finishes we try hard to make sure the `Build` unit
//! doesn't leak into the standard paths elsewhere because it's, well, not very
//! standard. To handle this we immediately remove the `Build` from the
//! dependency graph and specially handle it. It's internally flagged as an
//! active unit which allocates a jobserver token, and then execution proceeds
//! as usual.
//!
//! When the original work queued with the `BuildRmeta` completes it will send a
//! `Finish` notification for the `Build` unit. When this happens we catch this
//! happening and run the deferred `Job` we found during the completion of the
//! `BuildRmeta`. This should handle everything in theory and add up to a
//! working system!

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::marker;
use std::process::Output;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use crossbeam_utils::thread::Scope;
use jobserver::{Acquired, HelperThread};
use log::{debug, info, trace};

use super::context::OutputFile;
use super::job::{
    Freshness::{self, Dirty, Fresh},
    Job,
};
use super::{BuildContext, BuildPlan, CompileMode, Context, Unit};
use crate::core::{PackageId, TargetKind};
use crate::handle_error;
use crate::util;
use crate::util::diagnostic_server::{self, DiagnosticPrinter};
use crate::util::{internal, profile, CargoResult, CargoResultExt, ProcessBuilder};
use crate::util::{Config, DependencyQueue};
use crate::util::{Progress, ProgressStyle};

/// A management structure of the entire dependency graph to compile.
///
/// This structure is backed by the `DependencyQueue` type and manages the
/// actual compilation step of each package. Packages enqueue units of work and
/// then later on the entire graph is processed and compiled.
pub struct JobQueue<'a, 'cfg> {
    // An instance of a dependency queue which keeps track of all units and jobs
    // associated to execute with them. This is built up over time while we're
    // preparing a `JobQueue` and is then used to dynamically track what work is
    // ready to execute as work finishes.
    queue: DependencyQueue<Unit<'a>, Job>,

    // A cross-thread channel used to send messages to the main reactor thread
    // which spawns off work.
    tx: Sender<Message>,
    rx: Receiver<Message>,

    // All work items that are currently executing. Maps from a unique ID
    // assigned in a monotonically increasing fashion (allocated from `next_id`)
    // to the `Unit` that is being executed. `Finish` messages will contain
    // these ids and allow mapping back to the original `Unit`.
    active: HashMap<u32, Unit<'a>>,
    next_id: u32,

    // Metadata information used to keep track of what we're printing on the
    // console. This ensures we don't print messages more than once and we print
    // them at reasonable-ish times.
    compiled: HashSet<PackageId>,
    documented: HashSet<PackageId>,
    counts: HashMap<PackageId, usize>,
    is_release: bool,
    progress: Progress<'cfg>,

    // Auxiliary tables to track the status of pipelined units. A `Unit` with a
    // `BuildRmeta` mode will actually perform the work of a `Build` unit that
    // depends on it, so we need to track that information here to ensure that
    // we execute all the callbacks and finish units at the right time.
    //
    // The `pipelined` map keeps track of `BuildRmeta` units to their
    // corresponding `Build` unit along with a preallocated id for the `Build`
    // unit. This is created during `self.run`, and then consumed later during
    // `self.finish` for the `BuildRmeta` unit.
    //
    // The `deferred` map keeps track of a mapping of the id of the `Build` unit
    // to the `Job` that should execute when it's finished. The `Job` should
    // always be a quick noop.
    pipelined: HashMap<Unit<'a>, (Unit<'a>, u32)>,
    deferred: HashMap<u32, Job>,
}

pub struct JobState<'a> {
    // Communication channel back to the main coordination thread.
    tx: Sender<Message>,

    // If this job is executing as part of a pipelined compilation this will
    // list the id of the rmeta job. This is mutated to `None` once we send a
    // message saying that the rmeta if finished.
    rmeta_id: Cell<Option<u32>>,

    // Historical versions of Cargo made use of the `'a` argument here, so to
    // leave the door open to future refactorings keep it here.
    _marker: marker::PhantomData<&'a ()>,
}

enum Message {
    Run(String),
    BuildPlanMsg(String, ProcessBuilder, Arc<Vec<OutputFile>>),
    Stdout(String),
    Stderr(String),
    FixDiagnostic(diagnostic_server::Message),
    Token(io::Result<Acquired>),
    Finish(u32, CargoResult<()>),
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

    /// A method to only be used when rustc is being executed to compile an
    /// rlib. As soon as the job learns that a metadata file is available this
    /// method is called to send a notification back to the job queue that
    /// metadata is ready to go and work that only depends on the metadata
    /// should be executed ASAP.
    ///
    /// This will panic if executed twice or outside of a unit that's not for an
    /// rlib.
    pub fn finish_rmeta(&self) {
        let id = self.rmeta_id.replace(None).unwrap();
        self.tx.send(Message::Finish(id, Ok(()))).unwrap();
    }
}

impl<'a, 'cfg> JobQueue<'a, 'cfg> {
    pub fn new(bcx: &BuildContext<'a, 'cfg>) -> JobQueue<'a, 'cfg> {
        let (tx, rx) = channel();
        let progress = Progress::with_style("Building", ProgressStyle::Ratio, bcx.config);
        JobQueue {
            queue: DependencyQueue::new(),
            tx,
            rx,
            active: HashMap::new(),
            compiled: HashSet::new(),
            documented: HashSet::new(),
            counts: HashMap::new(),
            is_release: bcx.build_config.release,
            progress,
            next_id: 0,
            deferred: Default::default(),
            pipelined: Default::default(),
        }
    }

    pub fn enqueue(
        &mut self,
        cx: &Context<'a, 'cfg>,
        unit: &Unit<'a>,
        job: Job,
    ) -> CargoResult<()> {
        // Binaries aren't actually needed to *compile* tests, just to run
        // them, so we don't include this dependency edge in the job graph.
        let dependencies = cx.dep_targets(unit);
        let dependencies = dependencies
            .iter()
            .filter(|unit| !unit.target.is_test() || !unit.target.is_bin())
            .cloned()
            .collect::<Vec<_>>();
        self.queue.queue(unit, job, &dependencies);

        // Keep track of how many work items we have for each package. This'll
        // help us later print whether a package is "fresh" or not if all of its
        // component units are indeed fresh.
        //
        // Note that we skip pipelined units since they don't make their way
        // into `run` below.
        if !cx.pipelined_units.contains(unit) {
            *self.counts.entry(unit.pkg.package_id()).or_insert(0) += 1;
        }
        Ok(())
    }

    /// Executes all jobs necessary to build the dependency graph.
    ///
    /// This function will spawn off `config.jobs()` workers to build all of the
    /// necessary dependencies, in order. Freshness is propagated as far as
    /// possible along each dependency chain.
    pub fn execute(&mut self, cx: &mut Context<'a, 'cfg>, plan: &mut BuildPlan) -> CargoResult<()> {
        let _p = profile::start("executing the job graph");
        self.queue.queue_finished();

        // Create a helper thread for acquiring jobserver tokens
        let tx = self.tx.clone();
        let helper = cx
            .jobserver
            .clone()
            .into_helper_thread(move |token| {
                drop(tx.send(Message::Token(token)));
            })
            .chain_err(|| "failed to create helper thread for jobserver management")?;

        // Create a helper thread to manage the diagnostics for rustfix if
        // necessary.
        let tx = self.tx.clone();
        let _diagnostic_server = cx
            .bcx
            .build_config
            .rustfix_diagnostic_server
            .borrow_mut()
            .take()
            .map(move |srv| srv.start(move |msg| drop(tx.send(Message::FixDiagnostic(msg)))));

        // Use `crossbeam` to create a scope in which we can execute scoped
        // threads. Note that this isn't currently required by Cargo but it was
        // historically required. This is left in for now in case we need the
        // `'a` ability for child threads in the near future, but if this
        // comment has been sitting here for a long time feel free to refactor
        // away crossbeam.
        crossbeam_utils::thread::scope(|scope| self.drain_the_queue(cx, plan, scope, &helper))
            .expect("child threads should't panic")
    }

    fn drain_the_queue(
        &mut self,
        cx: &mut Context<'a, 'cfg>,
        plan: &mut BuildPlan,
        scope: &Scope<'a>,
        jobserver_helper: &HelperThread,
    ) -> CargoResult<()> {
        let mut tokens = Vec::new();
        let mut queue = Vec::new();
        let mut print = DiagnosticPrinter::new(cx.bcx.config);
        trace!("queue: {:#?}", self.queue);

        // Iteratively execute the entire dependency graph. Each turn of the
        // loop starts out by scheduling as much work as possible (up to the
        // maximum number of parallel jobs we have tokens for). A local queue
        // is maintained separately from the main dependency queue as one
        // dequeue may actually dequeue quite a bit of work (e.g., 10 binaries
        // in one package).
        //
        // After a job has finished we update our internal state if it was
        // successful and otherwise wait for pending work to finish if it failed
        // and then immediately return.
        let mut error = None;
        let total = self.queue.len();
        loop {
            // Dequeue as much work as we can, learning about everything
            // possible that can run. Note that this is also the point where we
            // start requesting job tokens. Each job after the first needs to
            // request a token.
            while let Some((unit, job)) = self.queue.dequeue() {
                queue.push((unit, job));
                if self.active.len() + queue.len() > 1 {
                    jobserver_helper.request_token();
                }
            }

            // Now that we've learned of all possible work that we can execute
            // try to spawn it so long as we've got a jobserver token which says
            // we're able to perform some parallel work.
            while error.is_none() && self.active.len() < tokens.len() + 1 && !queue.is_empty() {
                let (unit, job) = queue.remove(0);
                self.run(&unit, job, cx, scope)?;
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

            // Drain all events at once to avoid displaying the progress bar
            // unnecessarily.
            let events: Vec<_> = self.rx.try_iter().collect();
            let events = if events.is_empty() {
                self.show_progress(total);
                vec![self.rx.recv().unwrap()]
            } else {
                events
            };

            for event in events {
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
                        self.progress.clear();
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
                    Message::Finish(id, result) => {
                        trace!("end id {}", id);
                        let unit = self.active.remove(&id).unwrap();
                        info!("end: {:?}", unit);
                        match result {
                            Ok(()) => self.finish(&unit, id, cx)?,
                            Err(e) => {
                                let msg = "The following warnings were emitted during compilation:";
                                self.emit_warnings(Some(msg), &unit, cx)?;

                                if !self.active.is_empty() {
                                    error = Some(failure::format_err!("build failed"));
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
                        tokens.push(
                            acquired_token.chain_err(|| "failed to acquire jobserver token")?,
                        );
                    }
                }
            }
        }
        self.progress.clear();

        let build_type = if self.is_release { "release" } else { "dev" };
        // NOTE: this may be a bit inaccurate, since this may not display the
        // profile for what was actually built. Profile overrides can change
        // these settings, and in some cases different targets are built with
        // different profiles. To be accurate, it would need to collect a
        // list of Units built, and maybe display a list of the different
        // profiles used. However, to keep it simple and compatible with old
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
            if !cx.bcx.build_config.build_plan {
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

    fn show_progress(&mut self, total: usize) {
        let count = total - self.queue.len();
        let active_names = self
            .active
            .values()
            .map(|u| self.name_for_progress(u))
            .collect::<Vec<_>>();
        drop(
            self.progress
                .tick_now(count, total, &format!(": {}", active_names.join(", "))),
        );
    }

    fn name_for_progress(&self, unit: &Unit<'_>) -> String {
        let pkg_name = unit.pkg.name();
        match unit.mode {
            CompileMode::Doc { .. } => format!("{}(doc)", pkg_name),
            CompileMode::RunCustomBuild => format!("{}(build)", pkg_name),
            _ => {
                let annotation = match unit.target.kind() {
                    TargetKind::Lib(_) => return pkg_name.to_string(),
                    TargetKind::CustomBuild => return format!("{}(build.rs)", pkg_name),
                    TargetKind::Bin => "bin",
                    TargetKind::Test => "test",
                    TargetKind::Bench => "bench",
                    TargetKind::ExampleBin | TargetKind::ExampleLib(_) => "example",
                };
                format!("{}({})", unit.target.name(), annotation)
            }
        }
    }

    /// Executes a job in the `scope` given, pushing the spawned thread's
    /// handled onto `threads`.
    fn run(
        &mut self,
        unit: &Unit<'a>,
        job: Job,
        cx: &Context<'a, 'cfg>,
        scope: &Scope<'a>,
    ) -> CargoResult<()> {
        // This unit should never run with a unit which has been pipelined.
        // These units are handled specially in `finish` below so just assert we
        // don't handle those here.
        assert!(cx.rmeta_unit_if_pipelined(unit).is_none());

        let id = self.next_id();
        info!("start: {} {:?}", id, unit);
        assert!(self.active.insert(id, *unit).is_none());
        *self.counts.get_mut(&unit.pkg.package_id()).unwrap() -= 1;

        // If we're the rmeta component of a build then our work will actually
        // produce the `Build` unit as well. Allocate a `build_id` for that unit
        // here and prepare some internal state to know about how this unit is
        // pipelined with another `Build` unit. This will also configure what
        // messages are sent back to the coordination thread to ensure that we
        // finish the `BuildRmeta` unit first and then the `Build` unit.
        let fresh = job.freshness();
        let (rmeta_id, last_id) = if unit.mode == CompileMode::BuildRmeta {
            log::debug!("preparing state to pipeline next unit");
            let build_id = self.next_id();
            let build_unit = unit.with_mode(CompileMode::Build, cx.bcx.units);
            assert!(self
                .pipelined
                .insert(*unit, (build_unit, build_id))
                .is_none());
            (Some(id), build_id)
        } else {
            (None, id)
        };

        let my_tx = self.tx.clone();
        let doit = move || {
            let state = JobState {
                tx: my_tx.clone(),
                _marker: marker::PhantomData,
                rmeta_id: Cell::new(rmeta_id),
            };
            let res = job.run(&state);

            // If the `rmeta_id` wasn't consumed but it was set previously, then
            // we either have:
            //
            // 1. The `job` didn't do anything because it was "fresh".
            // 2. The `job` returned an error and didn't reach the point where
            //    it called `finish_rmeta`.
            // 3. We forgot to call `finish_rmeta` and there's a bug in Cargo.
            //
            // Ruling out the third, the other two are pretty common and we just need
            // to say that both the `BuildRmeta` and `Build` (which we're
            // responsible for) are finished. If an error happens we don't
            // finish `Build` because it didn't actually complete.
            if let Some(rmeta_id) = state.rmeta_id.replace(None) {
                let ok = res.is_ok();
                my_tx.send(Message::Finish(rmeta_id, res)).unwrap();
                if ok {
                    my_tx.send(Message::Finish(last_id, Ok(()))).unwrap();
                }
            } else {
                // ... otherwise if we don't have anything to do with pipelining
                // this is business as usual...
                my_tx.send(Message::Finish(last_id, res)).unwrap();
            }
        };

        if !cx.bcx.build_config.build_plan {
            // Print out some nice progress information.
            self.note_working_on(cx.bcx.config, unit, fresh)?;
        }

        match fresh {
            Freshness::Fresh => doit(),
            Freshness::Dirty => {
                scope.spawn(move |_| doit());
            }
        }

        Ok(())
    }

    fn next_id(&mut self) -> u32 {
        let ret = self.next_id;
        self.next_id = ret.checked_add(1).unwrap();
        return ret;
    }

    fn emit_warnings(
        &mut self,
        msg: Option<&str>,
        unit: &Unit<'a>,
        cx: &mut Context<'_, '_>,
    ) -> CargoResult<()> {
        let output = cx.build_state.outputs.lock().unwrap();
        let bcx = &mut cx.bcx;
        if let Some(output) = output.get(&(unit.pkg.package_id(), unit.kind)) {
            if !output.warnings.is_empty() {
                if let Some(msg) = msg {
                    writeln!(bcx.config.shell().err(), "{}\n", msg)?;
                }

                for warning in output.warnings.iter() {
                    bcx.config.shell().warn(warning)?;
                }

                if msg.is_some() {
                    // Output an empty line.
                    writeln!(bcx.config.shell().err())?;
                }
            }
        }

        Ok(())
    }

    fn finish(&mut self, unit: &Unit<'a>, id: u32, cx: &mut Context<'_, '_>) -> CargoResult<()> {
        if unit.mode.is_run_custom_build() && cx.bcx.show_warnings(unit.pkg.package_id()) {
            self.emit_warnings(None, unit, cx)?;
        }
        self.queue.finish(unit);

        // Specially handled pipelined units here. If we're finishing the
        // metadata component of a pipelined unit then we have an invariant that
        // the `Build` unit is always ready to go. Forcibly dequeue that from
        // the dependency queue and save off the work it would otherwise do.
        // Along the way we also update the `active` map which should transition
        // the jobserver token we were previously using to this unit.
        //
        // If we're finishing a `Build` unit then the whole pipeline is now
        // finished, so we need to actually run the work we deferred earlier
        // which should complete quickly.
        if unit.mode == CompileMode::BuildRmeta {
            log::debug!("first of a pipelined unit finished");
            let (build_unit, build_id) = self.pipelined[unit];
            assert!(self.active.insert(build_id, build_unit).is_none());
            let job = self.queue.dequeue_key(&build_unit);
            assert!(self.deferred.insert(build_id, job).is_none());
        } else if cx.pipelined_units.contains(unit) {
            log::debug!("completing the work of a pipelined unit");
            let job = self.deferred.remove(&id).unwrap();
            job.run(&JobState {
                tx: self.tx.clone(),
                _marker: marker::PhantomData,
                rmeta_id: Cell::new(None),
            })?;
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
        unit: &Unit<'a>,
        fresh: Freshness,
    ) -> CargoResult<()> {
        if (self.compiled.contains(&unit.pkg.package_id()) && !unit.mode.is_doc())
            || (self.documented.contains(&unit.pkg.package_id()) && unit.mode.is_doc())
        {
            return Ok(());
        }

        match fresh {
            // Any dirty stage which runs at least one command gets printed as
            // being a compiled package.
            Dirty => {
                if unit.mode.is_doc() {
                    // Skip doc test.
                    if !unit.mode.is_any_test() {
                        self.documented.insert(unit.pkg.package_id());
                        config.shell().status("Documenting", unit.pkg)?;
                    }
                } else {
                    self.compiled.insert(unit.pkg.package_id());
                    if unit.mode.is_check() {
                        config.shell().status("Checking", unit.pkg)?;
                    } else {
                        config.shell().status("Compiling", unit.pkg)?;
                    }
                }
            }
            Fresh => {
                // If doc test are last, only print "Fresh" if nothing has been printed.
                if self.counts[&unit.pkg.package_id()] == 0
                    && !(unit.mode == CompileMode::Doctest
                        && self.compiled.contains(&unit.pkg.package_id()))
                {
                    self.compiled.insert(unit.pkg.package_id());
                    config.shell().verbose(|c| c.status("Fresh", unit.pkg))?;
                }
            }
        }
        Ok(())
    }
}
