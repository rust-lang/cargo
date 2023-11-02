//! Management of the interaction between the main `cargo` and all spawned jobs.
//!
//! ## Overview
//!
//! This module implements a job queue. A job here represents a unit of work,
//! which is roughly a rusc invocation, a build script run, or just a no-op.
//! The job queue primarily handles the following things:
//!
//! * Spawns concurrent jobs. Depending on its [`Freshness`], a job could be
//!     either executed on a spawned thread or ran on the same thread to avoid
//!     the threading overhead.
//! * Controls the number of concurrency. It allocates and manages [`jobserver`]
//!     tokens to each spawned off rustc and build scripts.
//! * Manages the communication between the main `cargo` process and its
//!     spawned jobs. Those [`Message`]s are sent over a [`Queue`] shared
//!     across threads.
//! * Schedules the execution order of each [`Job`]. Priorities are determined
//!     when calling [`JobQueue::enqueue`] to enqueue a job. The scheduling is
//!     relatively rudimentary and could likely be improved.
//!
//! A rough outline of building a queue and executing jobs is:
//!
//! 1. [`JobQueue::new`] to simply create one queue.
//! 2. [`JobQueue::enqueue`] to add new jobs onto the queue.
//! 3. Consumes the queue and executes all jobs via [`JobQueue::execute`].
//!
//! The primary loop happens insides [`JobQueue::execute`], which is effectively
//! [`DrainState::drain_the_queue`]. [`DrainState`] is, as its name tells,
//! the running state of the job queue getting drained.
//!
//! ## Jobserver
//!
//! As of Feb. 2023, Cargo and rustc have a relatively simple jobserver
//! relationship with each other. They share a single jobserver amongst what
//! is potentially hundreds of threads of work on many-cored systems.
//! The jobserver could come from either the environment (e.g., from a `make`
//! invocation), or from Cargo creating its own jobserver server if there is no
//! jobserver to inherit from.
//!
//! Cargo wants to complete the build as quickly as possible, fully saturating
//! all cores (as constrained by the `-j=N`) parameter. Cargo also must not spawn
//! more than N threads of work: the total amount of tokens we have floating
//! around must always be limited to N.
//!
//! It is not really possible to optimally choose which crate should build
//! first or last; nor is it possible to decide whether to give an additional
//! token to rustc first or rather spawn a new crate of work. The algorithm in
//! Cargo prioritizes spawning as many crates (i.e., rustc processes) as
//! possible. In short, the jobserver relationship among Cargo and rustc
//! processes is **1 `cargo` to N `rustc`**. Cargo knows nothing beyond rustc
//! processes in terms of parallelism[^parallel-rustc].
//!
//! We integrate with the [jobserver] crate, originating from GNU make
//! [POSIX jobserver], to make sure that build scripts which use make to
//! build C code can cooperate with us on the number of used tokens and
//! avoid overfilling the system we're on.
//!
//! ## Scheduling
//!
//! The current scheduling algorithm is not really polished. It is simply based
//! on a dependency graph [`DependencyQueue`]. We continue adding nodes onto
//! the graph until we finalize it. When the graph gets finalized, it finds the
//! sum of the cost of each dependencies of each node, including transitively.
//! The sum of dependency cost turns out to be the cost of each given node.
//!
//! At the time being, the cost is just passed as a fixed placeholder in
//! [`JobQueue::enqueue`]. In the future, we could explore more possibilities
//! around it. For instance, we start persisting timing information for each
//! build somewhere. For a subsequent build, we can look into the historical
//! data and perform a PGO-like optimization to prioritize jobs, making a build
//! fully pipelined.
//!
//! ## Message queue
//!
//! Each spawned thread running a process uses the message queue [`Queue`] to
//! send messages back to the main thread (the one running `cargo`).
//! The main thread coordinates everything, and handles printing output.
//!
//! It is important to be careful which messages use [`push`] vs [`push_bounded`].
//! `push` is for priority messages (like tokens, or "finished") where the
//! sender shouldn't block. We want to handle those so real work can proceed
//! ASAP.
//!
//! `push_bounded` is only for messages being printed to stdout/stderr. Being
//! bounded prevents a flood of messages causing a large amount of memory
//! being used.
//!
//! `push` also avoids blocking which helps avoid deadlocks. For example, when
//! the diagnostic server thread is dropped, it waits for the thread to exit.
//! But if the thread is blocked on a full queue, and there is a critical
//! error, the drop will deadlock. This should be fixed at some point in the
//! future. The jobserver thread has a similar problem, though it will time
//! out after 1 second.
//!
//! To access the message queue, each running `Job` is given its own [`JobState`],
//! containing everything it needs to communicate with the main thread.
//!
//! See [`Message`] for all available message kinds.
//!
//! [^parallel-rustc]: In fact, `jobserver` that Cargo uses also manages the
//!     allocation of tokens to rustc beyond the implicit token each rustc owns
//!     (i.e., the ones used for parallel LLVM work and parallel rustc threads).
//!     See also ["Rust Compiler Development Guide: Parallel Compilation"]
//!     and [this comment][rustc-codegen] in rust-lang/rust.
//!
//! ["Rust Compiler Development Guide: Parallel Compilation"]: https://rustc-dev-guide.rust-lang.org/parallel-rustc.html
//! [rustc-codegen]: https://github.com/rust-lang/rust/blob/5423745db8b434fcde54888b35f518f00cce00e4/compiler/rustc_codegen_ssa/src/back/write.rs#L1204-L1217
//! [jobserver]: https://docs.rs/jobserver
//! [POSIX jobserver]: https://www.gnu.org/software/make/manual/html_node/POSIX-Jobserver.html
//! [`push`]: Queue::push
//! [`push_bounded`]: Queue::push_bounded

mod job;
mod job_state;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{self, Scope};
use std::time::Duration;

use anyhow::{format_err, Context as _};
use cargo_util::ProcessBuilder;
use jobserver::{Acquired, HelperThread};
use semver::Version;
use tracing::{debug, trace};

pub use self::job::Freshness::{self, Dirty, Fresh};
pub use self::job::{Job, Work};
pub use self::job_state::JobState;
use super::context::OutputFile;
use super::timings::Timings;
use super::{BuildContext, BuildPlan, CompileMode, Context, Unit};
use crate::core::compiler::descriptive_pkg_name;
use crate::core::compiler::future_incompat::{
    self, FutureBreakageItem, FutureIncompatReportPackage,
};
use crate::core::resolver::ResolveBehavior;
use crate::core::{PackageId, Shell, TargetKind};
use crate::util::diagnostic_server::{self, DiagnosticPrinter};
use crate::util::errors::AlreadyPrintedError;
use crate::util::machine_message::{self, Message as _};
use crate::util::CargoResult;
use crate::util::{self, internal, profile};
use crate::util::{Config, DependencyQueue, Progress, ProgressStyle, Queue};

/// This structure is backed by the `DependencyQueue` type and manages the
/// queueing of compilation steps for each package. Packages enqueue units of
/// work and then later on the entire graph is converted to DrainState and
/// executed.
pub struct JobQueue<'cfg> {
    queue: DependencyQueue<Unit, Artifact, Job>,
    counts: HashMap<PackageId, usize>,
    timings: Timings<'cfg>,
}

/// This structure is backed by the `DependencyQueue` type and manages the
/// actual compilation step of each package. Packages enqueue units of work and
/// then later on the entire graph is processed and compiled.
///
/// It is created from JobQueue when we have fully assembled the crate graph
/// (i.e., all package dependencies are known).
struct DrainState<'cfg> {
    // This is the length of the DependencyQueue when starting out
    total_units: usize,

    queue: DependencyQueue<Unit, Artifact, Job>,
    messages: Arc<Queue<Message>>,
    /// Diagnostic deduplication support.
    diag_dedupe: DiagDedupe<'cfg>,
    /// Count of warnings, used to print a summary after the job succeeds
    warning_count: HashMap<JobId, WarningCount>,
    active: HashMap<JobId, Unit>,
    compiled: HashSet<PackageId>,
    documented: HashSet<PackageId>,
    scraped: HashSet<PackageId>,
    counts: HashMap<PackageId, usize>,
    progress: Progress<'cfg>,
    next_id: u32,
    timings: Timings<'cfg>,

    /// Tokens that are currently owned by this Cargo, and may be "associated"
    /// with a rustc process. They may also be unused, though if so will be
    /// dropped on the next loop iteration.
    ///
    /// Note that the length of this may be zero, but we will still spawn work,
    /// as we share the implicit token given to this Cargo process with a
    /// single rustc process.
    tokens: Vec<Acquired>,

    /// The list of jobs that we have not yet started executing, but have
    /// retrieved from the `queue`. We eagerly pull jobs off the main queue to
    /// allow us to request jobserver tokens pretty early.
    pending_queue: Vec<(Unit, Job, usize)>,
    print: DiagnosticPrinter<'cfg>,

    /// How many jobs we've finished
    finished: usize,
    per_package_future_incompat_reports: Vec<FutureIncompatReportPackage>,
}

/// Count of warnings, used to print a summary after the job succeeds
#[derive(Default)]
pub struct WarningCount {
    /// total number of warnings
    pub total: usize,
    /// number of warnings that were suppressed because they
    /// were duplicates of a previous warning
    pub duplicates: usize,
    /// number of fixable warnings set to `NotAllowed`
    /// if any errors have been seen ofr the current
    /// target
    pub fixable: FixableWarnings,
}

impl WarningCount {
    /// If an error is seen this should be called
    /// to set `fixable` to `NotAllowed`
    fn disallow_fixable(&mut self) {
        self.fixable = FixableWarnings::NotAllowed;
    }

    /// Checks fixable if warnings are allowed
    /// fixable warnings are allowed if no
    /// errors have been seen for the current
    /// target. If an error was seen `fixable`
    /// will be `NotAllowed`.
    fn fixable_allowed(&self) -> bool {
        match &self.fixable {
            FixableWarnings::NotAllowed => false,
            _ => true,
        }
    }
}

/// Used to keep track of how many fixable warnings there are
/// and if fixable warnings are allowed
#[derive(Default)]
pub enum FixableWarnings {
    NotAllowed,
    #[default]
    Zero,
    Positive(usize),
}

pub struct ErrorsDuringDrain {
    pub count: usize,
}

struct ErrorToHandle {
    error: anyhow::Error,

    /// This field is true for "interesting" errors and false for "mundane"
    /// errors. If false, we print the above error only if it's the first one
    /// encountered so far while draining the job queue.
    ///
    /// At most places that an error is propagated, we set this to false to
    /// avoid scenarios where Cargo might end up spewing tons of redundant error
    /// messages. For example if an i/o stream got closed somewhere, we don't
    /// care about individually reporting every thread that it broke; just the
    /// first is enough.
    ///
    /// The exception where print_always is true is that we do report every
    /// instance of a rustc invocation that failed with diagnostics. This
    /// corresponds to errors from Message::Finish.
    print_always: bool,
}

impl<E> From<E> for ErrorToHandle
where
    anyhow::Error: From<E>,
{
    fn from(error: E) -> Self {
        ErrorToHandle {
            error: anyhow::Error::from(error),
            print_always: false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub u32);

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Handler for deduplicating diagnostics.
struct DiagDedupe<'cfg> {
    seen: RefCell<HashSet<u64>>,
    config: &'cfg Config,
}

impl<'cfg> DiagDedupe<'cfg> {
    fn new(config: &'cfg Config) -> Self {
        DiagDedupe {
            seen: RefCell::new(HashSet::new()),
            config,
        }
    }

    /// Emits a diagnostic message.
    ///
    /// Returns `true` if the message was emitted, or `false` if it was
    /// suppressed for being a duplicate.
    fn emit_diag(&self, diag: &str) -> CargoResult<bool> {
        let h = util::hash_u64(diag);
        if !self.seen.borrow_mut().insert(h) {
            return Ok(false);
        }
        let mut shell = self.config.shell();
        shell.print_ansi_stderr(diag.as_bytes())?;
        shell.err().write_all(b"\n")?;
        Ok(true)
    }
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
    Run(JobId, String),
    BuildPlanMsg(String, ProcessBuilder, Arc<Vec<OutputFile>>),
    Stdout(String),
    Stderr(String),

    // This is for general stderr output from subprocesses
    Diagnostic {
        id: JobId,
        level: String,
        diag: String,
        fixable: bool,
    },
    // This handles duplicate output that is suppressed, for showing
    // only a count of duplicate messages instead
    WarningCount {
        id: JobId,
        emitted: bool,
        fixable: bool,
    },
    // This is for warnings generated by Cargo's interpretation of the
    // subprocess output, e.g. scrape-examples prints a warning if a
    // unit fails to be scraped
    Warning {
        id: JobId,
        warning: String,
    },

    FixDiagnostic(diagnostic_server::Message),
    Token(io::Result<Acquired>),
    Finish(JobId, Artifact, CargoResult<()>),
    FutureIncompatReport(JobId, Vec<FutureBreakageItem>),
}

impl<'cfg> JobQueue<'cfg> {
    pub fn new(bcx: &BuildContext<'_, 'cfg>) -> JobQueue<'cfg> {
        JobQueue {
            queue: DependencyQueue::new(),
            counts: HashMap::new(),
            timings: Timings::new(bcx, &bcx.roots),
        }
    }

    pub fn enqueue(&mut self, cx: &Context<'_, 'cfg>, unit: &Unit, job: Job) -> CargoResult<()> {
        let dependencies = cx.unit_deps(unit);
        let mut queue_deps = dependencies
            .iter()
            .filter(|dep| {
                // Binaries aren't actually needed to *compile* tests, just to run
                // them, so we don't include this dependency edge in the job graph.
                // But we shouldn't filter out dependencies being scraped for Rustdoc.
                (!dep.unit.target.is_test() && !dep.unit.target.is_bin())
                    || dep.unit.artifact.is_true()
                    || dep.unit.mode.is_doc_scrape()
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
                (dep.unit.clone(), artifact)
            })
            .collect::<HashMap<_, _>>();

        // This is somewhat tricky, but we may need to synthesize some
        // dependencies for this target if it requires full upstream
        // compilations to have completed. Because of pipelining, some
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
                depend_on_deps_of_deps(cx, &mut queue_deps, dep.unit.clone());
            }

            fn depend_on_deps_of_deps(
                cx: &Context<'_, '_>,
                deps: &mut HashMap<Unit, Artifact>,
                unit: Unit,
            ) {
                for dep in cx.unit_deps(&unit) {
                    if deps.insert(dep.unit.clone(), Artifact::All).is_none() {
                        depend_on_deps_of_deps(cx, deps, dep.unit.clone());
                    }
                }
            }
        }

        // For now we use a fixed placeholder value for the cost of each unit, but
        // in the future this could be used to allow users to provide hints about
        // relative expected costs of units, or this could be automatically set in
        // a smarter way using timing data from a previous compilation.
        self.queue.queue(unit.clone(), job, queue_deps, 100);
        *self.counts.entry(unit.pkg.package_id()).or_insert(0) += 1;
        Ok(())
    }

    /// Executes all jobs necessary to build the dependency graph.
    ///
    /// This function will spawn off `config.jobs()` workers to build all of the
    /// necessary dependencies, in order. Freshness is propagated as far as
    /// possible along each dependency chain.
    pub fn execute(mut self, cx: &mut Context<'_, '_>, plan: &mut BuildPlan) -> CargoResult<()> {
        let _p = profile::start("executing the job graph");
        self.queue.queue_finished();

        let progress = Progress::with_style("Building", ProgressStyle::Ratio, cx.bcx.config);
        let state = DrainState {
            total_units: self.queue.len(),
            queue: self.queue,
            // 100 here is somewhat arbitrary. It is a few screenfulls of
            // output, and hopefully at most a few megabytes of memory for
            // typical messages. If you change this, please update the test
            // caching_large_output, too.
            messages: Arc::new(Queue::new(100)),
            diag_dedupe: DiagDedupe::new(cx.bcx.config),
            warning_count: HashMap::new(),
            active: HashMap::new(),
            compiled: HashSet::new(),
            documented: HashSet::new(),
            scraped: HashSet::new(),
            counts: self.counts,
            progress,
            next_id: 0,
            timings: self.timings,
            tokens: Vec::new(),
            pending_queue: Vec::new(),
            print: DiagnosticPrinter::new(cx.bcx.config, &cx.bcx.rustc().workspace_wrapper),
            finished: 0,
            per_package_future_incompat_reports: Vec::new(),
        };

        // Create a helper thread for acquiring jobserver tokens
        let messages = state.messages.clone();
        let helper = cx
            .jobserver
            .clone()
            .into_helper_thread(move |token| {
                messages.push(Message::Token(token));
            })
            .with_context(|| "failed to create helper thread for jobserver management")?;

        // Create a helper thread to manage the diagnostics for rustfix if
        // necessary.
        let messages = state.messages.clone();
        // It is important that this uses `push` instead of `push_bounded` for
        // now. If someone wants to fix this to be bounded, the `drop`
        // implementation needs to be changed to avoid possible deadlocks.
        let _diagnostic_server = cx
            .bcx
            .build_config
            .rustfix_diagnostic_server
            .borrow_mut()
            .take()
            .map(move |srv| srv.start(move |msg| messages.push(Message::FixDiagnostic(msg))));

        thread::scope(
            move |scope| match state.drain_the_queue(cx, plan, scope, &helper) {
                Some(err) => Err(err),
                None => Ok(()),
            },
        )
    }
}

impl<'cfg> DrainState<'cfg> {
    fn spawn_work_if_possible<'s>(
        &mut self,
        cx: &mut Context<'_, '_>,
        jobserver_helper: &HelperThread,
        scope: &'s Scope<'s, '_>,
    ) -> CargoResult<()> {
        // Dequeue as much work as we can, learning about everything
        // possible that can run. Note that this is also the point where we
        // start requesting job tokens. Each job after the first needs to
        // request a token.
        while let Some((unit, job, priority)) = self.queue.dequeue() {
            // We want to keep the pieces of work in the `pending_queue` sorted
            // by their priorities, and insert the current job at its correctly
            // sorted position: following the lower priority jobs, and the ones
            // with the same priority (since they were dequeued before the
            // current one, we also keep that relation).
            let idx = self
                .pending_queue
                .partition_point(|&(_, _, p)| p <= priority);
            self.pending_queue.insert(idx, (unit, job, priority));
            if self.active.len() + self.pending_queue.len() > 1 {
                jobserver_helper.request_token();
            }
        }

        // Now that we've learned of all possible work that we can execute
        // try to spawn it so long as we've got a jobserver token which says
        // we're able to perform some parallel work.
        // The `pending_queue` is sorted in ascending priority order, and we
        // remove items from its end to schedule the highest priority items
        // sooner.
        while self.has_extra_tokens() && !self.pending_queue.is_empty() {
            let (unit, job, _) = self.pending_queue.pop().unwrap();
            *self.counts.get_mut(&unit.pkg.package_id()).unwrap() -= 1;
            if !cx.bcx.build_config.build_plan {
                // Print out some nice progress information.
                // NOTE: An error here will drop the job without starting it.
                // That should be OK, since we want to exit as soon as
                // possible during an error.
                self.note_working_on(cx.bcx.config, cx.bcx.ws.root(), &unit, job.freshness())?;
            }
            self.run(&unit, job, cx, scope);
        }

        Ok(())
    }

    fn has_extra_tokens(&self) -> bool {
        self.active.len() < self.tokens.len() + 1
    }

    fn handle_event(
        &mut self,
        cx: &mut Context<'_, '_>,
        plan: &mut BuildPlan,
        event: Message,
    ) -> Result<(), ErrorToHandle> {
        match event {
            Message::Run(id, cmd) => {
                cx.bcx
                    .config
                    .shell()
                    .verbose(|c| c.status("Running", &cmd))?;
                self.timings.unit_start(id, self.active[&id].clone());
            }
            Message::BuildPlanMsg(module_name, cmd, filenames) => {
                plan.update(&module_name, &cmd, &filenames)?;
            }
            Message::Stdout(out) => {
                writeln!(cx.bcx.config.shell().out(), "{}", out)?;
            }
            Message::Stderr(err) => {
                let mut shell = cx.bcx.config.shell();
                shell.print_ansi_stderr(err.as_bytes())?;
                shell.err().write_all(b"\n")?;
            }
            Message::Diagnostic {
                id,
                level,
                diag,
                fixable,
            } => {
                let emitted = self.diag_dedupe.emit_diag(&diag)?;
                if level == "warning" {
                    self.bump_warning_count(id, emitted, fixable);
                }
                if level == "error" {
                    let cnts = self.warning_count.entry(id).or_default();
                    // If there is an error, the `cargo fix` message should not show
                    cnts.disallow_fixable();
                }
            }
            Message::Warning { id, warning } => {
                cx.bcx.config.shell().warn(warning)?;
                self.bump_warning_count(id, true, false);
            }
            Message::WarningCount {
                id,
                emitted,
                fixable,
            } => {
                self.bump_warning_count(id, emitted, fixable);
            }
            Message::FixDiagnostic(msg) => {
                self.print.print(&msg)?;
            }
            Message::Finish(id, artifact, result) => {
                let unit = match artifact {
                    // If `id` has completely finished we remove it
                    // from the `active` map ...
                    Artifact::All => {
                        trace!("end: {:?}", id);
                        self.finished += 1;
                        self.report_warning_count(
                            cx.bcx.config,
                            id,
                            &cx.bcx.rustc().workspace_wrapper,
                        );
                        self.active.remove(&id).unwrap()
                    }
                    // ... otherwise if it hasn't finished we leave it
                    // in there as we'll get another `Finish` later on.
                    Artifact::Metadata => {
                        trace!("end (meta): {:?}", id);
                        self.active[&id].clone()
                    }
                };
                debug!("end ({:?}): {:?}", unit, result);
                match result {
                    Ok(()) => self.finish(id, &unit, artifact, cx)?,
                    Err(_) if cx.bcx.unit_can_fail_for_docscraping(&unit) => {
                        cx.failed_scrape_units
                            .lock()
                            .unwrap()
                            .insert(cx.files().metadata(&unit));
                        self.queue.finish(&unit, &artifact);
                    }
                    Err(error) => {
                        let msg = "The following warnings were emitted during compilation:";
                        self.emit_warnings(Some(msg), &unit, cx)?;
                        self.back_compat_notice(cx, &unit)?;
                        return Err(ErrorToHandle {
                            error,
                            print_always: true,
                        });
                    }
                }
            }
            Message::FutureIncompatReport(id, items) => {
                let package_id = self.active[&id].pkg.package_id();
                self.per_package_future_incompat_reports
                    .push(FutureIncompatReportPackage { package_id, items });
            }
            Message::Token(acquired_token) => {
                let token = acquired_token.with_context(|| "failed to acquire jobserver token")?;
                self.tokens.push(token);
            }
        }

        Ok(())
    }

    // This will also tick the progress bar as appropriate
    fn wait_for_events(&mut self) -> Vec<Message> {
        // Drain all events at once to avoid displaying the progress bar
        // unnecessarily. If there's no events we actually block waiting for
        // an event, but we keep a "heartbeat" going to allow `record_cpu`
        // to run above to calculate CPU usage over time. To do this we
        // listen for a message with a timeout, and on timeout we run the
        // previous parts of the loop again.
        let mut events = self.messages.try_pop_all();
        if events.is_empty() {
            loop {
                self.tick_progress();
                self.tokens.truncate(self.active.len() - 1);
                match self.messages.pop(Duration::from_millis(500)) {
                    Some(message) => {
                        events.push(message);
                        break;
                    }
                    None => continue,
                }
            }
        }
        events
    }

    /// This is the "main" loop, where Cargo does all work to run the
    /// compiler.
    ///
    /// This returns an Option to prevent the use of `?` on `Result` types
    /// because it is important for the loop to carefully handle errors.
    fn drain_the_queue<'s>(
        mut self,
        cx: &mut Context<'_, '_>,
        plan: &mut BuildPlan,
        scope: &'s Scope<'s, '_>,
        jobserver_helper: &HelperThread,
    ) -> Option<anyhow::Error> {
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
        // and then immediately return (or keep going, if requested by the build
        // config).
        let mut errors = ErrorsDuringDrain { count: 0 };
        // CAUTION! Do not use `?` or break out of the loop early. Every error
        // must be handled in such a way that the loop is still allowed to
        // drain event messages.
        loop {
            if errors.count == 0 || cx.bcx.build_config.keep_going {
                if let Err(e) = self.spawn_work_if_possible(cx, jobserver_helper, scope) {
                    self.handle_error(&mut cx.bcx.config.shell(), &mut errors, e);
                }
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
            for event in self.wait_for_events() {
                if let Err(event_err) = self.handle_event(cx, plan, event) {
                    self.handle_error(&mut cx.bcx.config.shell(), &mut errors, event_err);
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
        if profile.debuginfo.is_turned_on() {
            opt_type += " + debuginfo";
        }

        let time_elapsed = util::elapsed(cx.bcx.config.creation_time().elapsed());
        if let Err(e) = self.timings.finished(cx, &errors.to_error()) {
            self.handle_error(&mut cx.bcx.config.shell(), &mut errors, e);
        }
        if cx.bcx.build_config.emit_json() {
            let mut shell = cx.bcx.config.shell();
            let msg = machine_message::BuildFinished {
                success: errors.count == 0,
            }
            .to_json_string();
            if let Err(e) = writeln!(shell.out(), "{}", msg) {
                self.handle_error(&mut shell, &mut errors, e);
            }
        }

        if let Some(error) = errors.to_error() {
            // Any errors up to this point have already been printed via the
            // `display_error` inside `handle_error`.
            Some(anyhow::Error::new(AlreadyPrintedError::new(error)))
        } else if self.queue.is_empty() && self.pending_queue.is_empty() {
            let message = format!(
                "{} [{}] target(s) in {}",
                profile_name, opt_type, time_elapsed
            );
            if !cx.bcx.build_config.build_plan {
                // It doesn't really matter if this fails.
                let _ = cx.bcx.config.shell().status("Finished", message);
                future_incompat::save_and_display_report(
                    cx.bcx,
                    &self.per_package_future_incompat_reports,
                );
            }

            None
        } else {
            debug!("queue: {:#?}", self.queue);
            Some(internal("finished with jobs still left in the queue"))
        }
    }

    fn handle_error(
        &self,
        shell: &mut Shell,
        err_state: &mut ErrorsDuringDrain,
        new_err: impl Into<ErrorToHandle>,
    ) {
        let new_err = new_err.into();
        if new_err.print_always || err_state.count == 0 {
            crate::display_error(&new_err.error, shell);
            if err_state.count == 0 && !self.active.is_empty() {
                let _ = shell.warn("build failed, waiting for other jobs to finish...");
            }
            err_state.count += 1;
        } else {
            tracing::warn!("{:?}", new_err.error);
        }
    }

    // This also records CPU usage and marks concurrency; we roughly want to do
    // this as often as we spin on the events receiver (at least every 500ms or
    // so).
    fn tick_progress(&mut self) {
        // Record some timing information if `--timings` is enabled, and
        // this'll end up being a noop if we're not recording this
        // information.
        self.timings.mark_concurrency(
            self.active.len(),
            self.pending_queue.len(),
            self.queue.len(),
        );
        self.timings.record_cpu();

        let active_names = self
            .active
            .values()
            .map(|u| self.name_for_progress(u))
            .collect::<Vec<_>>();
        let _ = self.progress.tick_now(
            self.finished,
            self.total_units,
            &format!(": {}", active_names.join(", ")),
        );
    }

    fn name_for_progress(&self, unit: &Unit) -> String {
        let pkg_name = unit.pkg.name();
        let target_name = unit.target.name();
        match unit.mode {
            CompileMode::Doc { .. } => format!("{}(doc)", pkg_name),
            CompileMode::RunCustomBuild => format!("{}(build)", pkg_name),
            CompileMode::Test | CompileMode::Check { test: true } => match unit.target.kind() {
                TargetKind::Lib(_) => format!("{}(test)", target_name),
                TargetKind::CustomBuild => panic!("cannot test build script"),
                TargetKind::Bin => format!("{}(bin test)", target_name),
                TargetKind::Test => format!("{}(test)", target_name),
                TargetKind::Bench => format!("{}(bench)", target_name),
                TargetKind::ExampleBin | TargetKind::ExampleLib(_) => {
                    format!("{}(example test)", target_name)
                }
            },
            _ => match unit.target.kind() {
                TargetKind::Lib(_) => pkg_name.to_string(),
                TargetKind::CustomBuild => format!("{}(build.rs)", pkg_name),
                TargetKind::Bin => format!("{}(bin)", target_name),
                TargetKind::Test => format!("{}(test)", target_name),
                TargetKind::Bench => format!("{}(bench)", target_name),
                TargetKind::ExampleBin | TargetKind::ExampleLib(_) => {
                    format!("{}(example)", target_name)
                }
            },
        }
    }

    /// Executes a job.
    ///
    /// Fresh jobs block until finished (which should be very fast!), Dirty
    /// jobs will spawn a thread in the background and return immediately.
    fn run<'s>(&mut self, unit: &Unit, job: Job, cx: &Context<'_, '_>, scope: &'s Scope<'s, '_>) {
        let id = JobId(self.next_id);
        self.next_id = self.next_id.checked_add(1).unwrap();

        debug!("start {}: {:?}", id, unit);

        assert!(self.active.insert(id, unit.clone()).is_none());

        let messages = self.messages.clone();
        let is_fresh = job.freshness().is_fresh();
        let rmeta_required = cx.rmeta_required(unit);

        let doit = move |diag_dedupe| {
            let state = JobState::new(id, messages, diag_dedupe, rmeta_required);
            state.run_to_finish(job);
        };

        match is_fresh {
            true => {
                self.timings.add_fresh();
                // Running a fresh job on the same thread is often much faster than spawning a new
                // thread to run the job.
                doit(Some(&self.diag_dedupe));
            }
            false => {
                self.timings.add_dirty();
                scope.spawn(move || doit(None));
            }
        }
    }

    fn emit_warnings(
        &mut self,
        msg: Option<&str>,
        unit: &Unit,
        cx: &mut Context<'_, '_>,
    ) -> CargoResult<()> {
        let outputs = cx.build_script_outputs.lock().unwrap();
        let Some(metadata) = cx.find_build_script_metadata(unit) else {
            return Ok(());
        };
        let bcx = &mut cx.bcx;
        if let Some(output) = outputs.get(metadata) {
            if !output.warnings.is_empty() {
                if let Some(msg) = msg {
                    writeln!(bcx.config.shell().err(), "{}\n", msg)?;
                }

                for warning in output.warnings.iter() {
                    let warning_with_package =
                        format!("{}@{}: {}", unit.pkg.name(), unit.pkg.version(), warning);

                    bcx.config.shell().warn(warning_with_package)?;
                }

                if msg.is_some() {
                    // Output an empty line.
                    writeln!(bcx.config.shell().err())?;
                }
            }
        }

        Ok(())
    }

    fn bump_warning_count(&mut self, id: JobId, emitted: bool, fixable: bool) {
        let cnts = self.warning_count.entry(id).or_default();
        cnts.total += 1;
        if !emitted {
            cnts.duplicates += 1;
        // Don't add to fixable if it's already been emitted
        } else if fixable {
            // Do not add anything to the fixable warning count if
            // is `NotAllowed` since that indicates there was an
            // error while building this `Unit`
            if cnts.fixable_allowed() {
                cnts.fixable = match cnts.fixable {
                    FixableWarnings::NotAllowed => FixableWarnings::NotAllowed,
                    FixableWarnings::Zero => FixableWarnings::Positive(1),
                    FixableWarnings::Positive(fixable) => FixableWarnings::Positive(fixable + 1),
                };
            }
        }
    }

    /// Displays a final report of the warnings emitted by a particular job.
    fn report_warning_count(
        &mut self,
        config: &Config,
        id: JobId,
        rustc_workspace_wrapper: &Option<PathBuf>,
    ) {
        let count = match self.warning_count.remove(&id) {
            // An error could add an entry for a `Unit`
            // with 0 warnings but having fixable
            // warnings be disallowed
            Some(count) if count.total > 0 => count,
            None | Some(_) => return,
        };
        let unit = &self.active[&id];
        let mut message = descriptive_pkg_name(&unit.pkg.name(), &unit.target, &unit.mode);
        message.push_str(" generated ");
        match count.total {
            1 => message.push_str("1 warning"),
            n => {
                let _ = write!(message, "{} warnings", n);
            }
        };
        match count.duplicates {
            0 => {}
            1 => message.push_str(" (1 duplicate)"),
            n => {
                let _ = write!(message, " ({} duplicates)", n);
            }
        }
        // Only show the `cargo fix` message if its a local `Unit`
        if unit.is_local() {
            // Do not show this if there are any errors or no fixable warnings
            if let FixableWarnings::Positive(fixable) = count.fixable {
                // `cargo fix` doesnt have an option for custom builds
                if !unit.target.is_custom_build() {
                    // To make sure the correct command is shown for `clippy` we
                    // check if `RUSTC_WORKSPACE_WRAPPER` is set and pointing towards
                    // `clippy-driver`.
                    let clippy = std::ffi::OsStr::new("clippy-driver");
                    let command = match rustc_workspace_wrapper.as_ref().and_then(|x| x.file_stem())
                    {
                        Some(wrapper) if wrapper == clippy => "cargo clippy --fix",
                        _ => "cargo fix",
                    };
                    let mut args = {
                        let named = unit.target.description_named();
                        // if its a lib we need to add the package to fix
                        if unit.target.is_lib() {
                            format!("{} -p {}", named, unit.pkg.name())
                        } else {
                            named
                        }
                    };
                    if unit.mode.is_rustc_test()
                        && !(unit.target.is_test() || unit.target.is_bench())
                    {
                        args.push_str(" --tests");
                    }
                    let mut suggestions = format!("{} suggestion", fixable);
                    if fixable > 1 {
                        suggestions.push_str("s")
                    }
                    let _ = write!(
                        message,
                        " (run `{command} --{args}` to apply {suggestions})"
                    );
                }
            }
        }
        // Errors are ignored here because it is tricky to handle them
        // correctly, and they aren't important.
        let _ = config.shell().warn(message);
    }

    fn finish(
        &mut self,
        id: JobId,
        unit: &Unit,
        artifact: Artifact,
        cx: &mut Context<'_, '_>,
    ) -> CargoResult<()> {
        if unit.mode.is_run_custom_build() && unit.show_warnings(cx.bcx.config) {
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
        ws_root: &Path,
        unit: &Unit,
        fresh: &Freshness,
    ) -> CargoResult<()> {
        if (self.compiled.contains(&unit.pkg.package_id())
            && !unit.mode.is_doc()
            && !unit.mode.is_doc_scrape())
            || (self.documented.contains(&unit.pkg.package_id()) && unit.mode.is_doc())
            || (self.scraped.contains(&unit.pkg.package_id()) && unit.mode.is_doc_scrape())
        {
            return Ok(());
        }

        match fresh {
            // Any dirty stage which runs at least one command gets printed as
            // being a compiled package.
            Dirty(dirty_reason) => {
                if let Some(reason) = dirty_reason {
                    config
                        .shell()
                        .verbose(|shell| reason.present_to(shell, unit, ws_root))?;
                }

                if unit.mode.is_doc() {
                    self.documented.insert(unit.pkg.package_id());
                    config.shell().status("Documenting", &unit.pkg)?;
                } else if unit.mode.is_doc_test() {
                    // Skip doc test.
                } else if unit.mode.is_doc_scrape() {
                    self.scraped.insert(unit.pkg.package_id());
                    config.shell().status("Scraping", &unit.pkg)?;
                } else {
                    self.compiled.insert(unit.pkg.package_id());
                    if unit.mode.is_check() {
                        config.shell().status("Checking", &unit.pkg)?;
                    } else {
                        config.shell().status("Compiling", &unit.pkg)?;
                    }
                }
            }
            Fresh => {
                // If doc test are last, only print "Fresh" if nothing has been printed.
                if self.counts[&unit.pkg.package_id()] == 0
                    && !(unit.mode.is_doc_test() && self.compiled.contains(&unit.pkg.package_id()))
                {
                    self.compiled.insert(unit.pkg.package_id());
                    config.shell().verbose(|c| c.status("Fresh", &unit.pkg))?;
                }
            }
        }
        Ok(())
    }

    fn back_compat_notice(&self, cx: &Context<'_, '_>, unit: &Unit) -> CargoResult<()> {
        if unit.pkg.name() != "diesel"
            || unit.pkg.version() >= &Version::new(1, 4, 8)
            || cx.bcx.ws.resolve_behavior() == ResolveBehavior::V1
            || !unit.pkg.package_id().source_id().is_registry()
            || !unit.features.is_empty()
        {
            return Ok(());
        }
        if !cx
            .bcx
            .unit_graph
            .keys()
            .any(|unit| unit.pkg.name() == "diesel" && !unit.features.is_empty())
        {
            return Ok(());
        }
        cx.bcx.config.shell().note(
            "\
This error may be due to an interaction between diesel and Cargo's new
feature resolver. Try updating to diesel 1.4.8 to fix this error.
",
        )?;
        Ok(())
    }
}

impl ErrorsDuringDrain {
    fn to_error(&self) -> Option<anyhow::Error> {
        match self.count {
            0 => None,
            1 => Some(format_err!("1 job failed")),
            n => Some(format_err!("{} jobs failed", n)),
        }
    }
}
