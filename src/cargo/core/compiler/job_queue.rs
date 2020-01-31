use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::marker;
use std::mem;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use anyhow::format_err;
use crossbeam_utils::thread::Scope;
use jobserver::{Acquired, HelperThread};
use log::{debug, info, trace};

use super::context::OutputFile;
use super::job::{
    Freshness::{self, Dirty, Fresh},
    Job,
};
use super::timings::Timings;
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
    queue: DependencyQueue<Unit<'a>, Artifact, Job>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    active: HashMap<u32, Unit<'a>>,
    compiled: HashSet<PackageId>,
    documented: HashSet<PackageId>,
    counts: HashMap<PackageId, usize>,
    progress: Progress<'cfg>,
    next_id: u32,
    timings: Timings<'a, 'cfg>,
}

pub struct JobState<'a> {
    /// Channel back to the main thread to coordinate messages and such.
    tx: Sender<Message>,

    /// The job id that this state is associated with, used when sending
    /// messages back to the main thread.
    id: u32,

    /// Whether or not we're expected to have a call to `rmeta_produced`. Once
    /// that method is called this is dynamically set to `false` to prevent
    /// sending a double message later on.
    rmeta_required: Cell<bool>,

    // Historical versions of Cargo made use of the `'a` argument here, so to
    // leave the door open to future refactorings keep it here.
    _marker: marker::PhantomData<&'a ()>,
}

/// Possible artifacts that can be produced by compilations, used as edge values
/// in the dependency graph.
///
/// As edge values we can have multiple kinds of edges depending on one node,
/// for example some units may only depend on the metadata for an rlib while
/// others depend on the full rlib. This `Artifact` enum is used to distinguish
/// this case and track the progress of compilations as they proceed.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
enum Artifact {
    /// A generic placeholder for "depends on everything run by a step" and
    /// means that we can't start the next compilation until the previous has
    /// finished entirely.
    All,

    /// A node indicating that we only depend on the metadata of a compilation,
    /// but the compilation is typically also producing an rlib. We can start
    /// our step, however, before the full rlib is available.
    Metadata,
}

enum Message {
    Run(u32, String),
    BuildPlanMsg(String, ProcessBuilder, Arc<Vec<OutputFile>>),
    Stdout(String),
    Stderr(String),
    FixDiagnostic(diagnostic_server::Message),
    Token(io::Result<Acquired>),
    Finish(u32, Artifact, CargoResult<()>),
}

impl<'a> JobState<'a> {
    pub fn running(&self, cmd: &ProcessBuilder) {
        let _ = self.tx.send(Message::Run(self.id, cmd.to_string()));
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

    pub fn stdout(&self, stdout: String) {
        drop(self.tx.send(Message::Stdout(stdout)));
    }

    pub fn stderr(&self, stderr: String) {
        drop(self.tx.send(Message::Stderr(stderr)));
    }

    /// A method used to signal to the coordinator thread that the rmeta file
    /// for an rlib has been produced. This is only called for some rmeta
    /// builds when required, and can be called at any time before a job ends.
    /// This should only be called once because a metadata file can only be
    /// produced once!
    pub fn rmeta_produced(&self) {
        self.rmeta_required.set(false);
        let _ = self
            .tx
            .send(Message::Finish(self.id, Artifact::Metadata, Ok(())));
    }
}

impl<'a, 'cfg> JobQueue<'a, 'cfg> {
    pub fn new(bcx: &BuildContext<'a, 'cfg>, root_units: &[Unit<'a>]) -> JobQueue<'a, 'cfg> {
        let (tx, rx) = channel();
        let progress = Progress::with_style("Building", ProgressStyle::Ratio, bcx.config);
        let timings = Timings::new(bcx, root_units);
        JobQueue {
            queue: DependencyQueue::new(),
            tx,
            rx,
            active: HashMap::new(),
            compiled: HashSet::new(),
            documented: HashSet::new(),
            counts: HashMap::new(),
            progress,
            next_id: 0,
            timings,
        }
    }

    pub fn enqueue(
        &mut self,
        cx: &Context<'a, 'cfg>,
        unit: &Unit<'a>,
        job: Job,
    ) -> CargoResult<()> {
        let dependencies = cx.unit_deps(unit);
        let mut queue_deps = dependencies
            .iter()
            .filter(|dep| {
                // Binaries aren't actually needed to *compile* tests, just to run
                // them, so we don't include this dependency edge in the job graph.
                !dep.unit.target.is_test() && !dep.unit.target.is_bin()
            })
            .map(|dep| {
                // Handle the case here where our `unit -> dep` dependency may
                // only require the metadata, not the full compilation to
                // finish. Use the tables in `cx` to figure out what kind
                // of artifact is associated with this dependency.
                let artifact = if cx.only_requires_rmeta(unit, &dep.unit) {
                    Artifact::Metadata
                } else {
                    Artifact::All
                };
                (dep.unit, artifact)
            })
            .collect::<HashMap<_, _>>();

        // This is somewhat tricky, but we may need to synthesize some
        // dependencies for this target if it requires full upstream
        // compilations to have completed. If we're in pipelining mode then some
        // dependency edges may be `Metadata` due to the above clause (as
        // opposed to everything being `All`). For example consider:
        //
        //    a (binary)
        //    └ b (lib)
        //        └ c (lib)
        //
        // Here the dependency edge from B to C will be `Metadata`, and the
        // dependency edge from A to B will be `All`. For A to be compiled,
        // however, it currently actually needs the full rlib of C. This means
        // that we need to synthesize a dependency edge for the dependency graph
        // from A to C. That's done here.
        //
        // This will walk all dependencies of the current target, and if any of
        // *their* dependencies are `Metadata` then we depend on the `All` of
        // the target as well. This should ensure that edges changed to
        // `Metadata` propagate upwards `All` dependencies to anything that
        // transitively contains the `Metadata` edge.
        if unit.requires_upstream_objects() {
            for dep in dependencies {
                depend_on_deps_of_deps(cx, &mut queue_deps, dep.unit);
            }

            fn depend_on_deps_of_deps<'a>(
                cx: &Context<'a, '_>,
                deps: &mut HashMap<Unit<'a>, Artifact>,
                unit: Unit<'a>,
            ) {
                for dep in cx.unit_deps(&unit) {
                    if deps.insert(dep.unit, Artifact::All).is_none() {
                        depend_on_deps_of_deps(cx, deps, dep.unit);
                    }
                }
            }
        }

        self.queue.queue(*unit, job, queue_deps);
        *self.counts.entry(unit.pkg.package_id()).or_insert(0) += 1;
        Ok(())
    }

    /// Executes all jobs necessary to build the dependency graph.
    ///
    /// This function will spawn off `config.jobs()` workers to build all of the
    /// necessary dependencies, in order. Freshness is propagated as far as
    /// possible along each dependency chain.
    pub fn execute(&mut self, cx: &mut Context<'a, '_>, plan: &mut BuildPlan) -> CargoResult<()> {
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
            .expect("child threads shouldn't panic")
    }

    fn drain_the_queue(
        &mut self,
        cx: &mut Context<'a, '_>,
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
        let mut finished = 0;
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

            // Record some timing information if `-Ztimings` is enabled, and
            // this'll end up being a noop if we're not recording this
            // information.
            self.timings
                .mark_concurrency(self.active.len(), queue.len(), self.queue.len());
            self.timings.record_cpu();

            // Drain all events at once to avoid displaying the progress bar
            // unnecessarily. If there's no events we actually block waiting for
            // an event, but we keep a "heartbeat" going to allow `record_cpu`
            // to run above to calculate CPU usage over time. To do this we
            // listen for a message with a timeout, and on timeout we run the
            // previous parts of the loop again.
            let events: Vec<_> = self.rx.try_iter().collect();
            let events = if events.is_empty() {
                self.show_progress(finished, total);
                match self.rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(message) => vec![message],
                    Err(_) => continue,
                }
            } else {
                events
            };

            for event in events {
                match event {
                    Message::Run(id, cmd) => {
                        cx.bcx
                            .config
                            .shell()
                            .verbose(|c| c.status("Running", &cmd))?;
                        self.timings.unit_start(id, self.active[&id]);
                    }
                    Message::BuildPlanMsg(module_name, cmd, filenames) => {
                        plan.update(&module_name, &cmd, &filenames)?;
                    }
                    Message::Stdout(out) => {
                        cx.bcx.config.shell().stdout_println(out);
                    }
                    Message::Stderr(err) => {
                        let mut shell = cx.bcx.config.shell();
                        shell.print_ansi(err.as_bytes())?;
                        shell.err().write_all(b"\n")?;
                    }
                    Message::FixDiagnostic(msg) => {
                        print.print(&msg)?;
                    }
                    Message::Finish(id, artifact, result) => {
                        let unit = match artifact {
                            // If `id` has completely finished we remove it
                            // from the `active` map ...
                            Artifact::All => {
                                info!("end: {:?}", id);
                                finished += 1;
                                self.active.remove(&id).unwrap()
                            }
                            // ... otherwise if it hasn't finished we leave it
                            // in there as we'll get another `Finish` later on.
                            Artifact::Metadata => {
                                info!("end (meta): {:?}", id);
                                self.active[&id]
                            }
                        };
                        info!("end ({:?}): {:?}", unit, result);
                        match result {
                            Ok(()) => self.finish(id, &unit, artifact, cx)?,
                            Err(e) => {
                                let msg = "The following warnings were emitted during compilation:";
                                self.emit_warnings(Some(msg), &unit, cx)?;

                                if !self.active.is_empty() {
                                    error = Some(anyhow::format_err!("build failed"));
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

        let profile_name = cx.bcx.build_config.requested_profile;
        // NOTE: this may be a bit inaccurate, since this may not display the
        // profile for what was actually built. Profile overrides can change
        // these settings, and in some cases different targets are built with
        // different profiles. To be accurate, it would need to collect a
        // list of Units built, and maybe display a list of the different
        // profiles used. However, to keep it simple and compatible with old
        // behavior, we just display what the base profile is.
        let profile = cx.bcx.profiles.base_profile();
        let mut opt_type = String::from(if profile.opt_level.as_str() == "0" {
            "unoptimized"
        } else {
            "optimized"
        });
        if profile.debuginfo.unwrap_or(0) != 0 {
            opt_type += " + debuginfo";
        }

        let time_elapsed = util::elapsed(cx.bcx.config.creation_time().elapsed());

        if let Some(e) = error {
            Err(e)
        } else if self.queue.is_empty() && queue.is_empty() {
            let message = format!(
                "{} [{}] target(s) in {}",
                profile_name, opt_type, time_elapsed
            );
            if !cx.bcx.build_config.build_plan {
                cx.bcx.config.shell().status("Finished", message)?;
            }
            self.timings.finished(cx.bcx)?;
            Ok(())
        } else {
            debug!("queue: {:#?}", self.queue);
            Err(internal("finished with jobs still left in the queue"))
        }
    }

    fn show_progress(&mut self, count: usize, total: usize) {
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
        cx: &Context<'a, '_>,
        scope: &Scope<'a>,
    ) -> CargoResult<()> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).unwrap();

        info!("start {}: {:?}", id, unit);

        assert!(self.active.insert(id, *unit).is_none());
        *self.counts.get_mut(&unit.pkg.package_id()).unwrap() -= 1;

        let my_tx = self.tx.clone();
        let fresh = job.freshness();
        let rmeta_required = cx.rmeta_required(unit);
        let doit = move || {
            let state = JobState {
                id,
                tx: my_tx.clone(),
                rmeta_required: Cell::new(rmeta_required),
                _marker: marker::PhantomData,
            };

            let mut sender = FinishOnDrop {
                tx: &my_tx,
                id,
                result: Err(format_err!("worker panicked")),
            };
            sender.result = job.run(&state);

            // If the `rmeta_required` wasn't consumed but it was set
            // previously, then we either have:
            //
            // 1. The `job` didn't do anything because it was "fresh".
            // 2. The `job` returned an error and didn't reach the point where
            //    it called `rmeta_produced`.
            // 3. We forgot to call `rmeta_produced` and there's a bug in Cargo.
            //
            // Ruling out the third, the other two are pretty common for 2
            // we'll just naturally abort the compilation operation but for 1
            // we need to make sure that the metadata is flagged as produced so
            // send a synthetic message here.
            if state.rmeta_required.get() && sender.result.is_ok() {
                my_tx
                    .send(Message::Finish(id, Artifact::Metadata, Ok(())))
                    .unwrap();
            }

            // Use a helper struct with a `Drop` implementation to guarantee
            // that a `Finish` message is sent even if our job panics. We
            // shouldn't panic unless there's a bug in Cargo, so we just need
            // to make sure nothing hangs by accident.
            struct FinishOnDrop<'a> {
                tx: &'a Sender<Message>,
                id: u32,
                result: CargoResult<()>,
            }

            impl Drop for FinishOnDrop<'_> {
                fn drop(&mut self) {
                    let msg = mem::replace(&mut self.result, Ok(()));
                    drop(self.tx.send(Message::Finish(self.id, Artifact::All, msg)));
                }
            }
        };

        if !cx.bcx.build_config.build_plan {
            // Print out some nice progress information.
            self.note_working_on(cx.bcx.config, unit, fresh)?;
        }

        match fresh {
            Freshness::Fresh => {
                self.timings.add_fresh();
                doit()
            }
            Freshness::Dirty => {
                self.timings.add_dirty();
                scope.spawn(move |_| doit());
            }
        }

        Ok(())
    }

    fn emit_warnings(
        &mut self,
        msg: Option<&str>,
        unit: &Unit<'a>,
        cx: &mut Context<'_, '_>,
    ) -> CargoResult<()> {
        let outputs = cx.build_script_outputs.lock().unwrap();
        let bcx = &mut cx.bcx;
        if let Some(output) = outputs.get(&(unit.pkg.package_id(), unit.kind)) {
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

    fn finish(
        &mut self,
        id: u32,
        unit: &Unit<'a>,
        artifact: Artifact,
        cx: &mut Context<'a, '_>,
    ) -> CargoResult<()> {
        if unit.mode.is_run_custom_build() && cx.bcx.show_warnings(unit.pkg.package_id()) {
            self.emit_warnings(None, unit, cx)?;
        }
        let unlocked = self.queue.finish(unit, &artifact);
        match artifact {
            Artifact::All => self.timings.unit_finished(id, unlocked),
            Artifact::Metadata => self.timings.unit_rmeta_finished(id, unlocked),
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
                    self.documented.insert(unit.pkg.package_id());
                    config.shell().status("Documenting", unit.pkg)?;
                } else if unit.mode.is_doc_test() {
                    // Skip doc test.
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
                    && !(unit.mode.is_doc_test() && self.compiled.contains(&unit.pkg.package_id()))
                {
                    self.compiled.insert(unit.pkg.package_id());
                    config.shell().verbose(|c| c.status("Fresh", unit.pkg))?;
                }
            }
        }
        Ok(())
    }
}
