//! See [`JobState`].

use std::{cell::Cell, marker, sync::Arc};

use cargo_util::ProcessBuilder;

use crate::core::compiler::context::OutputFile;
use crate::core::compiler::future_incompat::FutureBreakageItem;
use crate::util::Queue;
use crate::CargoResult;

use super::{Artifact, DiagDedupe, Job, JobId, Message};

/// A `JobState` is constructed by `JobQueue::run` and passed to `Job::run`. It includes everything
/// necessary to communicate between the main thread and the execution of the job.
///
/// The job may execute on either a dedicated thread or the main thread. If the job executes on the
/// main thread, the `output` field must be set to prevent a deadlock.
pub struct JobState<'a, 'cfg> {
    /// Channel back to the main thread to coordinate messages and such.
    ///
    /// When the `output` field is `Some`, care must be taken to avoid calling `push_bounded` on
    /// the message queue to prevent a deadlock.
    messages: Arc<Queue<Message>>,

    /// Normally output is sent to the job queue with backpressure. When the job is fresh
    /// however we need to immediately display the output to prevent a deadlock as the
    /// output messages are processed on the same thread as they are sent from. `output`
    /// defines where to output in this case.
    ///
    /// Currently the `Shell` inside `Config` is wrapped in a `RefCell` and thus can't be passed
    /// between threads. This means that it isn't possible for multiple output messages to be
    /// interleaved. In the future, it may be wrapped in a `Mutex` instead. In this case
    /// interleaving is still prevented as the lock would be held for the whole printing of an
    /// output message.
    output: Option<&'a DiagDedupe<'cfg>>,

    /// The job id that this state is associated with, used when sending
    /// messages back to the main thread.
    id: JobId,

    /// Whether or not we're expected to have a call to `rmeta_produced`. Once
    /// that method is called this is dynamically set to `false` to prevent
    /// sending a double message later on.
    rmeta_required: Cell<bool>,

    // Historical versions of Cargo made use of the `'a` argument here, so to
    // leave the door open to future refactorings keep it here.
    _marker: marker::PhantomData<&'a ()>,
}

impl<'a, 'cfg> JobState<'a, 'cfg> {
    pub(super) fn new(
        id: JobId,
        messages: Arc<Queue<Message>>,
        output: Option<&'a DiagDedupe<'cfg>>,
        rmeta_required: bool,
    ) -> Self {
        Self {
            id,
            messages,
            output,
            rmeta_required: Cell::new(rmeta_required),
            _marker: marker::PhantomData,
        }
    }

    pub fn running(&self, cmd: &ProcessBuilder) {
        self.messages.push(Message::Run(self.id, cmd.to_string()));
    }

    pub fn build_plan(
        &self,
        module_name: String,
        cmd: ProcessBuilder,
        filenames: Arc<Vec<OutputFile>>,
    ) {
        self.messages
            .push(Message::BuildPlanMsg(module_name, cmd, filenames));
    }

    pub fn stdout(&self, stdout: String) -> CargoResult<()> {
        if let Some(dedupe) = self.output {
            writeln!(dedupe.config.shell().out(), "{}", stdout)?;
        } else {
            self.messages.push_bounded(Message::Stdout(stdout));
        }
        Ok(())
    }

    pub fn stderr(&self, stderr: String) -> CargoResult<()> {
        if let Some(dedupe) = self.output {
            let mut shell = dedupe.config.shell();
            shell.print_ansi_stderr(stderr.as_bytes())?;
            shell.err().write_all(b"\n")?;
        } else {
            self.messages.push_bounded(Message::Stderr(stderr));
        }
        Ok(())
    }

    /// See [`Message::Diagnostic`] and [`Message::WarningCount`].
    pub fn emit_diag(&self, level: String, diag: String, fixable: bool) -> CargoResult<()> {
        if let Some(dedupe) = self.output {
            let emitted = dedupe.emit_diag(&diag)?;
            if level == "warning" {
                self.messages.push(Message::WarningCount {
                    id: self.id,
                    emitted,
                    fixable,
                });
            }
        } else {
            self.messages.push_bounded(Message::Diagnostic {
                id: self.id,
                level,
                diag,
                fixable,
            });
        }
        Ok(())
    }

    /// See [`Message::Warning`].
    pub fn warning(&self, warning: String) -> CargoResult<()> {
        self.messages.push_bounded(Message::Warning {
            id: self.id,
            warning,
        });
        Ok(())
    }

    /// A method used to signal to the coordinator thread that the rmeta file
    /// for an rlib has been produced. This is only called for some rmeta
    /// builds when required, and can be called at any time before a job ends.
    /// This should only be called once because a metadata file can only be
    /// produced once!
    pub fn rmeta_produced(&self) {
        self.rmeta_required.set(false);
        self.messages
            .push(Message::Finish(self.id, Artifact::Metadata, Ok(())));
    }

    /// Drives a [`Job`] to finish. This ensures that a [`Message::Finish`] is
    /// sent even if our job panics.
    pub(super) fn run_to_finish(self, job: Job) {
        let mut sender = FinishOnDrop {
            messages: &self.messages,
            id: self.id,
            result: None,
        };
        sender.result = Some(job.run(&self));

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
        if self.rmeta_required.get() && sender.result.as_ref().unwrap().is_ok() {
            self.messages
                .push(Message::Finish(self.id, Artifact::Metadata, Ok(())));
        }

        // Use a helper struct with a `Drop` implementation to guarantee
        // that a `Finish` message is sent even if our job panics. We
        // shouldn't panic unless there's a bug in Cargo, so we just need
        // to make sure nothing hangs by accident.
        struct FinishOnDrop<'a> {
            messages: &'a Queue<Message>,
            id: JobId,
            result: Option<CargoResult<()>>,
        }

        impl Drop for FinishOnDrop<'_> {
            fn drop(&mut self) {
                let result = self
                    .result
                    .take()
                    .unwrap_or_else(|| Err(anyhow::format_err!("worker panicked")));
                self.messages
                    .push(Message::Finish(self.id, Artifact::All, result));
            }
        }
    }

    pub fn future_incompat_report(&self, report: Vec<FutureBreakageItem>) {
        self.messages
            .push(Message::FutureIncompatReport(self.id, report));
    }
}
