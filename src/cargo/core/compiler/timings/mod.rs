//! Timing tracking.
//!
//! This module implements some simple tracking information for timing of how
//! long it takes for different units to compile.

pub mod report;

use super::CompileMode;
use super::Unit;
use super::UnitIndex;
use crate::core::compiler::BuildContext;
use crate::core::compiler::BuildRunner;
use crate::core::compiler::job_queue::JobId;
use crate::ops::cargo_report::timings::prepare_context;
use crate::util::cpu::State;
use crate::util::log_message::LogMessage;
use crate::util::style;
use crate::util::{CargoResult, GlobalContext};

use cargo_util::paths;
use std::collections::HashMap;
use std::io::BufWriter;
use std::time::{Duration, Instant};

/// Tracking information for the entire build.
///
/// Methods on this structure are generally called from the main thread of a
/// running [`JobQueue`] instance (`DrainState` in specific) when the queue
/// receives messages from spawned off threads.
///
/// [`JobQueue`]: super::JobQueue
pub struct Timings<'gctx> {
    gctx: &'gctx GlobalContext,
    /// Whether or not timings should be captured.
    enabled: bool,
    /// When Cargo started.
    start: Instant,
    /// A summary of the root units.
    ///
    /// A map from unit to index.
    unit_to_index: HashMap<Unit, UnitIndex>,
    /// Units that are in the process of being built.
    /// When they finished, they are moved to `unit_times`.
    active: HashMap<JobId, Unit>,
    /// Last recorded state of the system's CPUs and when it happened
    last_cpu_state: Option<State>,
    last_cpu_recording: Instant,
    /// Recorded CPU states, stored as tuples. First element is when the
    /// recording was taken and second element is percentage usage of the
    /// system.
    cpu_usage: Vec<(f64, f64)>,
}

/// Section of compilation (e.g. frontend, backend, linking).
#[derive(Copy, Clone, serde::Serialize)]
pub struct CompilationSection {
    /// Start of the section, as an offset in seconds from `UnitTime::start`.
    pub start: f64,
    /// End of the section, as an offset in seconds from `UnitTime::start`.
    pub end: Option<f64>,
}

/// Data for a single compilation unit, prepared for serialization to JSON.
///
/// This is used by the HTML report's JavaScript to render the pipeline graph.
#[derive(serde::Serialize)]
pub struct UnitData {
    pub i: UnitIndex,
    pub name: String,
    pub version: String,
    pub mode: String,
    pub target: String,
    pub features: Vec<String>,
    pub start: f64,
    pub duration: f64,
    pub unblocked_units: Vec<UnitIndex>,
    pub unblocked_rmeta_units: Vec<UnitIndex>,
    pub sections: Option<Vec<(report::SectionName, report::SectionData)>>,
}

impl<'gctx> Timings<'gctx> {
    pub fn new(bcx: &BuildContext<'_, 'gctx>) -> Timings<'gctx> {
        let start = bcx.gctx.creation_time();
        let enabled = bcx.logger.is_some();

        if !enabled {
            return Timings {
                gctx: bcx.gctx,
                enabled,
                start,
                unit_to_index: HashMap::new(),
                active: HashMap::new(),
                last_cpu_state: None,
                last_cpu_recording: Instant::now(),
                cpu_usage: Vec::new(),
            };
        }

        let last_cpu_state = match State::current() {
            Ok(state) => Some(state),
            Err(e) => {
                tracing::info!("failed to get CPU state, CPU tracking disabled: {:?}", e);
                None
            }
        };

        Timings {
            gctx: bcx.gctx,
            enabled,
            start,
            unit_to_index: bcx.unit_to_index.clone(),
            active: HashMap::new(),
            last_cpu_state,
            last_cpu_recording: Instant::now(),
            cpu_usage: Vec::new(),
        }
    }

    /// Mark that a unit has started running.
    pub fn unit_start(&mut self, build_runner: &BuildRunner<'_, '_>, id: JobId, unit: Unit) {
        let Some(logger) = build_runner.bcx.logger else {
            return;
        };
        let mut target = if unit.target.is_lib()
            && matches!(unit.mode, CompileMode::Build | CompileMode::Check { .. })
        {
            // Special case for brevity, since most dependencies hit this path.
            "".to_string()
        } else {
            format!(" {}", unit.target.description_named())
        };
        match unit.mode {
            CompileMode::Test => target.push_str(" (test)"),
            CompileMode::Build => {}
            CompileMode::Check { test: true } => target.push_str(" (check-test)"),
            CompileMode::Check { test: false } => target.push_str(" (check)"),
            CompileMode::Doc { .. } => target.push_str(" (doc)"),
            CompileMode::Doctest => target.push_str(" (doc test)"),
            CompileMode::Docscrape => target.push_str(" (doc scrape)"),
            CompileMode::RunCustomBuild => target.push_str(" (run)"),
        }
        let start = self.start.elapsed().as_secs_f64();
        logger.log(LogMessage::UnitStarted {
            index: self.unit_to_index[&unit],
            elapsed: start,
        });

        assert!(self.active.insert(id, unit).is_none());
    }

    /// Mark that the `.rmeta` file as generated.
    pub fn unit_rmeta_finished(
        &mut self,
        build_runner: &BuildRunner<'_, '_>,
        id: JobId,
        unblocked: Vec<&Unit>,
    ) {
        let Some(logger) = build_runner.bcx.logger else {
            return;
        };
        // `id` may not always be active. "fresh" units unconditionally
        // generate `Message::Finish`, but this active map only tracks dirty
        // units.
        let Some(unit) = self.active.get(&id) else {
            return;
        };
        let elapsed = self.start.elapsed().as_secs_f64();

        let unblocked = unblocked.iter().map(|u| self.unit_to_index[u]).collect();
        logger.log(LogMessage::UnitRmetaFinished {
            index: self.unit_to_index[unit],
            elapsed,
            unblocked,
        });
    }

    /// Mark that a unit has finished running.
    pub fn unit_finished(
        &mut self,
        build_runner: &BuildRunner<'_, '_>,
        id: JobId,
        unblocked: Vec<&Unit>,
    ) {
        let Some(logger) = build_runner.bcx.logger else {
            return;
        };
        // See note above in `unit_rmeta_finished`, this may not always be active.
        let Some(unit) = self.active.remove(&id) else {
            return;
        };
        let elapsed = self.start.elapsed().as_secs_f64();

        let unblocked = unblocked.iter().map(|u| self.unit_to_index[u]).collect();
        logger.log(LogMessage::UnitFinished {
            index: self.unit_to_index[&unit],
            elapsed,
            unblocked,
        });
    }

    /// Handle the start/end of a compilation section.
    pub fn unit_section_timing(
        &mut self,
        build_runner: &BuildRunner<'_, '_>,
        id: JobId,
        section_timing: &SectionTiming,
    ) {
        let Some(logger) = build_runner.bcx.logger else {
            return;
        };
        let Some(unit) = self.active.get(&id) else {
            return;
        };
        let elapsed = self.start.elapsed().as_secs_f64();

        let index = self.unit_to_index[&unit];
        let section = section_timing.name.clone();
        logger.log(match section_timing.event {
            SectionTimingEvent::Start => LogMessage::UnitSectionStarted {
                index,
                elapsed,
                section,
            },
            SectionTimingEvent::End => LogMessage::UnitSectionFinished {
                index,
                elapsed,
                section,
            },
        })
    }

    /// Take a sample of CPU usage
    pub fn record_cpu(&mut self) {
        if !self.enabled {
            return;
        }
        let Some(prev) = &mut self.last_cpu_state else {
            return;
        };
        // Don't take samples too frequently, even if requested.
        let now = Instant::now();
        if self.last_cpu_recording.elapsed() < Duration::from_millis(100) {
            return;
        }
        let current = match State::current() {
            Ok(s) => s,
            Err(e) => {
                tracing::info!("failed to get CPU state: {:?}", e);
                return;
            }
        };
        let pct_idle = current.idle_since(prev);
        *prev = current;
        self.last_cpu_recording = now;
        let dur = now.duration_since(self.start).as_secs_f64();
        self.cpu_usage.push((dur, 100.0 - pct_idle));
    }

    /// Call this when all units are finished.
    pub fn finished(
        &mut self,
        build_runner: &BuildRunner<'_, '_>,
        error: &Option<anyhow::Error>,
    ) -> CargoResult<()> {
        if let Some(logger) = build_runner.bcx.logger {
            // Log CPU usage data so it can be reconstructed by `cargo report timings`.
            for &(elapsed, usage) in &self.cpu_usage {
                logger.log(LogMessage::CpuUsage { elapsed, usage });
            }

            if let Some(logs) = logger.get_logs() {
                let timings_path = build_runner
                    .files()
                    .timings_dir()
                    .expect("artifact-dir was not locked");
                paths::create_dir_all(&timings_path)?;
                let run_id = logger.run_id();
                let filename = timings_path.join(format!("cargo-timing-{run_id}.html"));
                let mut f = BufWriter::new(paths::create(&filename)?);

                let mut ctx = prepare_context(logs.into_iter(), run_id)?;
                ctx.error = error;
                ctx.cpu_usage = std::borrow::Cow::Borrowed(&self.cpu_usage);
                report::write_html(ctx, &mut f)?;

                let unstamped_filename = timings_path.join("cargo-timing.html");
                paths::link_or_copy(&filename, &unstamped_filename)?;

                let mut shell = self.gctx.shell();
                let timing_path = std::env::current_dir().unwrap_or_default().join(&filename);
                let link = shell.err_file_hyperlink(&timing_path);
                let msg = format!("report saved to {link}{}{link:#}", timing_path.display(),);
                shell.status_with_color("Timing", msg, &style::NOTE)?;
            }
        }
        Ok(())
    }
}

/// Start or end of a section timing.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum SectionTimingEvent {
    Start,
    End,
}

/// Represents a certain section (phase) of rustc compilation.
/// It is emitted by rustc when the `--json=timings` flag is used.
#[derive(serde::Deserialize, Debug)]
pub struct SectionTiming {
    pub name: String,
    pub event: SectionTimingEvent,
}
