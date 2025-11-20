//! Timing tracking.
//!
//! This module implements some simple tracking information for timing of how
//! long it takes for different units to compile.

mod report;

use super::{CompileMode, Unit};
use crate::core::PackageId;
use crate::core::compiler::job_queue::JobId;
use crate::core::compiler::{BuildContext, BuildRunner, TimingOutput};
use crate::util::cpu::State;
use crate::util::log_message::LogMessage;
use crate::util::machine_message::{self, Message};
use crate::util::style;
use crate::util::{CargoResult, GlobalContext};

use cargo_util::paths;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::io::BufWriter;
use std::time::{Duration, Instant};
use tracing::warn;

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
    /// If true, saves an HTML report to disk.
    report_html: bool,
    /// If true, emits JSON information with timing information.
    report_json: bool,
    /// When Cargo started.
    start: Instant,
    /// A rendered string of when compilation started.
    start_str: String,
    /// A summary of the root units.
    ///
    /// Tuples of `(package_description, target_descriptions)`.
    root_targets: Vec<(String, Vec<String>)>,
    /// The build profile.
    profile: String,
    /// Total number of fresh units.
    total_fresh: u32,
    /// Total number of dirty units.
    total_dirty: u32,
    /// Time tracking for each individual unit.
    unit_times: Vec<UnitTime>,
    /// Units that are in the process of being built.
    /// When they finished, they are moved to `unit_times`.
    active: HashMap<JobId, UnitTime>,
    /// Concurrency-tracking information. This is periodically updated while
    /// compilation progresses.
    concurrency: Vec<Concurrency>,
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
    start: f64,
    /// End of the section, as an offset in seconds from `UnitTime::start`.
    end: Option<f64>,
}

/// Tracking information for an individual unit.
struct UnitTime {
    unit: Unit,
    /// A string describing the cargo target.
    target: String,
    /// The time when this unit started as an offset in seconds from `Timings::start`.
    start: f64,
    /// Total time to build this unit in seconds.
    duration: f64,
    /// The time when the `.rmeta` file was generated, an offset in seconds
    /// from `start`.
    rmeta_time: Option<f64>,
    /// Reverse deps that are freed to run after this unit finished.
    unlocked_units: Vec<Unit>,
    /// Same as `unlocked_units`, but unlocked by rmeta.
    unlocked_rmeta_units: Vec<Unit>,
    /// Individual compilation section durations, gathered from `--json=timings`.
    ///
    /// IndexMap is used to keep original insertion order, we want to be able to tell which
    /// sections were started in which order.
    sections: IndexMap<String, CompilationSection>,
}

/// Periodic concurrency tracking information.
#[derive(serde::Serialize)]
struct Concurrency {
    /// Time as an offset in seconds from `Timings::start`.
    t: f64,
    /// Number of units currently running.
    active: usize,
    /// Number of units that could run, but are waiting for a jobserver token.
    waiting: usize,
    /// Number of units that are not yet ready, because they are waiting for
    /// dependencies to finish.
    inactive: usize,
}

/// Data for a single compilation unit, prepared for serialization to JSON.
///
/// This is used by the HTML report's JavaScript to render the pipeline graph.
#[derive(serde::Serialize)]
struct UnitData {
    i: usize,
    name: String,
    version: String,
    mode: String,
    target: String,
    start: f64,
    duration: f64,
    rmeta_time: Option<f64>,
    unlocked_units: Vec<usize>,
    unlocked_rmeta_units: Vec<usize>,
    sections: Option<Vec<(String, report::SectionData)>>,
}

impl<'gctx> Timings<'gctx> {
    pub fn new(bcx: &BuildContext<'_, 'gctx>, root_units: &[Unit]) -> Timings<'gctx> {
        let has_report = |what| bcx.build_config.timing_outputs.contains(&what);
        let report_html = has_report(TimingOutput::Html);
        let report_json = has_report(TimingOutput::Json);
        let enabled = report_html | report_json | bcx.logger.is_some();

        let mut root_map: HashMap<PackageId, Vec<String>> = HashMap::new();
        for unit in root_units {
            let target_desc = unit.target.description_named();
            root_map
                .entry(unit.pkg.package_id())
                .or_default()
                .push(target_desc);
        }
        let root_targets = root_map
            .into_iter()
            .map(|(pkg_id, targets)| {
                let pkg_desc = format!("{} {}", pkg_id.name(), pkg_id.version());
                (pkg_desc, targets)
            })
            .collect();
        let start_str = jiff::Timestamp::now().to_string();
        let profile = bcx.build_config.requested_profile.to_string();
        let last_cpu_state = if enabled {
            match State::current() {
                Ok(state) => Some(state),
                Err(e) => {
                    tracing::info!("failed to get CPU state, CPU tracking disabled: {:?}", e);
                    None
                }
            }
        } else {
            None
        };

        Timings {
            gctx: bcx.gctx,
            enabled,
            report_html,
            report_json,
            start: bcx.gctx.creation_time(),
            start_str,
            root_targets,
            profile,
            total_fresh: 0,
            total_dirty: 0,
            unit_times: Vec::new(),
            active: HashMap::new(),
            concurrency: Vec::new(),
            last_cpu_state,
            last_cpu_recording: Instant::now(),
            cpu_usage: Vec::new(),
        }
    }

    /// Mark that a unit has started running.
    pub fn unit_start(&mut self, id: JobId, unit: Unit) {
        if !self.enabled {
            return;
        }
        let mut target = if unit.target.is_lib() && unit.mode == CompileMode::Build {
            // Special case for brevity, since most dependencies hit
            // this path.
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
        let unit_time = UnitTime {
            unit,
            target,
            start: self.start.elapsed().as_secs_f64(),
            duration: 0.0,
            rmeta_time: None,
            unlocked_units: Vec::new(),
            unlocked_rmeta_units: Vec::new(),
            sections: Default::default(),
        };
        assert!(self.active.insert(id, unit_time).is_none());
    }

    /// Mark that the `.rmeta` file as generated.
    pub fn unit_rmeta_finished(&mut self, id: JobId, unlocked: Vec<&Unit>) {
        if !self.enabled {
            return;
        }
        // `id` may not always be active. "fresh" units unconditionally
        // generate `Message::Finish`, but this active map only tracks dirty
        // units.
        let Some(unit_time) = self.active.get_mut(&id) else {
            return;
        };
        let t = self.start.elapsed().as_secs_f64();
        unit_time.rmeta_time = Some(t - unit_time.start);
        assert!(unit_time.unlocked_rmeta_units.is_empty());
        unit_time
            .unlocked_rmeta_units
            .extend(unlocked.iter().cloned().cloned());
    }

    /// Mark that a unit has finished running.
    pub fn unit_finished(
        &mut self,
        build_runner: &BuildRunner<'_, '_>,
        id: JobId,
        unlocked: Vec<&Unit>,
    ) {
        if !self.enabled {
            return;
        }
        // See note above in `unit_rmeta_finished`, this may not always be active.
        let Some(mut unit_time) = self.active.remove(&id) else {
            return;
        };
        let t = self.start.elapsed().as_secs_f64();
        unit_time.duration = t - unit_time.start;
        assert!(unit_time.unlocked_units.is_empty());
        unit_time
            .unlocked_units
            .extend(unlocked.iter().cloned().cloned());
        if self.report_json {
            let msg = machine_message::TimingInfo {
                package_id: unit_time.unit.pkg.package_id().to_spec(),
                target: &unit_time.unit.target,
                mode: unit_time.unit.mode,
                duration: unit_time.duration,
                rmeta_time: unit_time.rmeta_time,
                sections: unit_time.sections.clone().into_iter().collect(),
            }
            .to_json_string();
            crate::drop_println!(self.gctx, "{}", msg);
        }
        if let Some(logger) = build_runner.bcx.logger {
            logger.log(LogMessage::TimingInfo {
                package_id: unit_time.unit.pkg.package_id().to_spec(),
                target: unit_time.unit.target.clone(),
                mode: unit_time.unit.mode,
                duration: unit_time.duration,
                rmeta_time: unit_time.rmeta_time,
                sections: unit_time.sections.clone().into_iter().collect(),
            });
        }
        self.unit_times.push(unit_time);
    }

    /// Handle the start/end of a compilation section.
    pub fn unit_section_timing(&mut self, id: JobId, section_timing: &SectionTiming) {
        if !self.enabled {
            return;
        }
        let Some(unit_time) = self.active.get_mut(&id) else {
            return;
        };
        let now = self.start.elapsed().as_secs_f64();

        match section_timing.event {
            SectionTimingEvent::Start => {
                unit_time.start_section(&section_timing.name, now);
            }
            SectionTimingEvent::End => {
                unit_time.end_section(&section_timing.name, now);
            }
        }
    }

    /// This is called periodically to mark the concurrency of internal structures.
    pub fn mark_concurrency(&mut self, active: usize, waiting: usize, inactive: usize) {
        if !self.enabled {
            return;
        }
        let c = Concurrency {
            t: self.start.elapsed().as_secs_f64(),
            active,
            waiting,
            inactive,
        };
        self.concurrency.push(c);
    }

    /// Mark that a fresh unit was encountered. (No re-compile needed)
    pub fn add_fresh(&mut self) {
        self.total_fresh += 1;
    }

    /// Mark that a dirty unit was encountered. (Re-compile needed)
    pub fn add_dirty(&mut self) {
        self.total_dirty += 1;
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
        if !self.enabled {
            return Ok(());
        }
        self.mark_concurrency(0, 0, 0);
        self.unit_times
            .sort_unstable_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        if self.report_html {
            let timestamp = self.start_str.replace(&['-', ':'][..], "");
            let timings_path = build_runner
                .files()
                .timings_dir()
                .expect("artifact-dir was not locked");
            paths::create_dir_all(&timings_path)?;
            let filename = timings_path.join(format!("cargo-timing-{}.html", timestamp));
            let mut f = BufWriter::new(paths::create(&filename)?);

            let ctx = report::RenderContext {
                start: self.start,
                start_str: &self.start_str,
                root_units: &self.root_targets,
                profile: &self.profile,
                total_fresh: self.total_fresh,
                total_dirty: self.total_dirty,
                unit_times: &self.unit_times,
                concurrency: &self.concurrency,
                cpu_usage: &self.cpu_usage,
            };
            report::write_html(ctx, &mut f, build_runner, error)?;

            let unstamped_filename = timings_path.join("cargo-timing.html");
            paths::link_or_copy(&filename, &unstamped_filename)?;

            let mut shell = self.gctx.shell();
            let timing_path = std::env::current_dir().unwrap_or_default().join(&filename);
            let link = shell.err_file_hyperlink(&timing_path);
            let msg = format!("report saved to {link}{}{link:#}", timing_path.display(),);
            shell.status_with_color("Timing", msg, &style::NOTE)?;
        }
        Ok(())
    }
}

impl UnitTime {
    fn name_ver(&self) -> String {
        format!("{} v{}", self.unit.pkg.name(), self.unit.pkg.version())
    }

    fn start_section(&mut self, name: &str, now: f64) {
        if self
            .sections
            .insert(
                name.to_string(),
                CompilationSection {
                    start: now - self.start,
                    end: None,
                },
            )
            .is_some()
        {
            warn!("compilation section {name} started more than once");
        }
    }

    fn end_section(&mut self, name: &str, now: f64) {
        let Some(section) = self.sections.get_mut(name) else {
            warn!("compilation section {name} ended, but it has no start recorded");
            return;
        };
        section.end = Some(now - self.start);
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
