//! Timing tracking.
//!
//! This module implements some simple tracking information for timing of how
//! long it takes for different units to compile.

pub mod report;

use super::{CompileMode, Unit};
use crate::core::PackageId;
use crate::core::compiler::BuildContext;
use crate::core::compiler::BuildRunner;
use crate::core::compiler::job_queue::JobId;
use crate::util::cpu::State;
use crate::util::log_message::LogMessage;
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
    /// A map from unit to index.
    unit_to_index: HashMap<Unit, u64>,
    /// Time tracking for each individual unit.
    unit_times: Vec<UnitTime>,
    /// Units that are in the process of being built.
    /// When they finished, they are moved to `unit_times`.
    active: HashMap<JobId, UnitTime>,
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
    /// Reverse deps that are unblocked and ready to run after this unit finishes.
    unblocked_units: Vec<Unit>,
    /// Same as `unblocked_units`, but unblocked by rmeta.
    unblocked_rmeta_units: Vec<Unit>,
    /// Individual compilation section durations, gathered from `--json=timings`.
    ///
    /// IndexMap is used to keep original insertion order, we want to be able to tell which
    /// sections were started in which order.
    sections: IndexMap<String, CompilationSection>,
}

/// Data for a single compilation unit, prepared for serialization to JSON.
///
/// This is used by the HTML report's JavaScript to render the pipeline graph.
#[derive(serde::Serialize)]
pub struct UnitData {
    pub i: u64,
    pub name: String,
    pub version: String,
    pub mode: String,
    pub target: String,
    pub features: Vec<String>,
    pub start: f64,
    pub duration: f64,
    pub unblocked_units: Vec<u64>,
    pub unblocked_rmeta_units: Vec<u64>,
    pub sections: Option<Vec<(report::SectionName, report::SectionData)>>,
}

impl<'gctx> Timings<'gctx> {
    pub fn new(bcx: &BuildContext<'_, 'gctx>, root_units: &[Unit]) -> Timings<'gctx> {
        let start = bcx.gctx.creation_time();
        let report_html = bcx.build_config.timing_report;
        let enabled = report_html | bcx.logger.is_some();

        if !enabled {
            return Timings {
                gctx: bcx.gctx,
                enabled,
                report_html,
                start,
                start_str: String::new(),
                root_targets: Vec::new(),
                profile: String::new(),
                total_fresh: 0,
                total_dirty: 0,
                unit_to_index: HashMap::new(),
                unit_times: Vec::new(),
                active: HashMap::new(),
                last_cpu_state: None,
                last_cpu_recording: Instant::now(),
                cpu_usage: Vec::new(),
            };
        }

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
            report_html,
            start,
            start_str,
            root_targets,
            profile,
            total_fresh: 0,
            total_dirty: 0,
            unit_to_index: bcx.unit_to_index.clone(),
            unit_times: Vec::new(),
            active: HashMap::new(),
            last_cpu_state,
            last_cpu_recording: Instant::now(),
            cpu_usage: Vec::new(),
        }
    }

    /// Mark that a unit has started running.
    pub fn unit_start(&mut self, build_runner: &BuildRunner<'_, '_>, id: JobId, unit: Unit) {
        if !self.enabled {
            return;
        }
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
        let unit_time = UnitTime {
            unit,
            target,
            start,
            duration: 0.0,
            rmeta_time: None,
            unblocked_units: Vec::new(),
            unblocked_rmeta_units: Vec::new(),
            sections: Default::default(),
        };
        if let Some(logger) = build_runner.bcx.logger {
            logger.log(LogMessage::UnitStarted {
                index: self.unit_to_index[&unit_time.unit],
                elapsed: start,
            });
        }
        assert!(self.active.insert(id, unit_time).is_none());
    }

    /// Mark that the `.rmeta` file as generated.
    pub fn unit_rmeta_finished(
        &mut self,
        build_runner: &BuildRunner<'_, '_>,
        id: JobId,
        unblocked: Vec<&Unit>,
    ) {
        if !self.enabled {
            return;
        }
        // `id` may not always be active. "fresh" units unconditionally
        // generate `Message::Finish`, but this active map only tracks dirty
        // units.
        let Some(unit_time) = self.active.get_mut(&id) else {
            return;
        };
        let elapsed = self.start.elapsed().as_secs_f64();
        unit_time.rmeta_time = Some(elapsed - unit_time.start);
        assert!(unit_time.unblocked_rmeta_units.is_empty());
        unit_time
            .unblocked_rmeta_units
            .extend(unblocked.iter().cloned().cloned());

        if let Some(logger) = build_runner.bcx.logger {
            let unblocked = unblocked.iter().map(|u| self.unit_to_index[u]).collect();
            logger.log(LogMessage::UnitRmetaFinished {
                index: self.unit_to_index[&unit_time.unit],
                elapsed,
                unblocked,
            });
        }
    }

    /// Mark that a unit has finished running.
    pub fn unit_finished(
        &mut self,
        build_runner: &BuildRunner<'_, '_>,
        id: JobId,
        unblocked: Vec<&Unit>,
    ) {
        if !self.enabled {
            return;
        }
        // See note above in `unit_rmeta_finished`, this may not always be active.
        let Some(mut unit_time) = self.active.remove(&id) else {
            return;
        };
        let elapsed = self.start.elapsed().as_secs_f64();
        unit_time.duration = elapsed - unit_time.start;
        assert!(unit_time.unblocked_units.is_empty());
        unit_time
            .unblocked_units
            .extend(unblocked.iter().cloned().cloned());

        if let Some(logger) = build_runner.bcx.logger {
            let unblocked = unblocked.iter().map(|u| self.unit_to_index[u]).collect();
            logger.log(LogMessage::UnitFinished {
                index: self.unit_to_index[&unit_time.unit],
                elapsed,
                unblocked,
            });
        }
        self.unit_times.push(unit_time);
    }

    /// Handle the start/end of a compilation section.
    pub fn unit_section_timing(
        &mut self,
        build_runner: &BuildRunner<'_, '_>,
        id: JobId,
        section_timing: &SectionTiming,
    ) {
        if !self.enabled {
            return;
        }
        let Some(unit_time) = self.active.get_mut(&id) else {
            return;
        };
        let elapsed = self.start.elapsed().as_secs_f64();

        match section_timing.event {
            SectionTimingEvent::Start => {
                unit_time.start_section(&section_timing.name, elapsed);
            }
            SectionTimingEvent::End => {
                unit_time.end_section(&section_timing.name, elapsed);
            }
        }

        if let Some(logger) = build_runner.bcx.logger {
            let index = self.unit_to_index[&unit_time.unit];
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

            let rustc_version = build_runner
                .bcx
                .rustc()
                .verbose_version
                .lines()
                .next()
                .expect("rustc version");
            let requested_targets = build_runner
                .bcx
                .build_config
                .requested_kinds
                .iter()
                .map(|kind| build_runner.bcx.target_data.short_name(kind).to_owned())
                .collect::<Vec<_>>();
            let num_cpus = std::thread::available_parallelism()
                .ok()
                .map(|x| x.get() as u64);

            let unit_data = report::to_unit_data(&self.unit_times, &self.unit_to_index);
            let concurrency = report::compute_concurrency(&unit_data);

            let ctx = report::RenderContext {
                start_str: self.start_str.clone(),
                root_units: self.root_targets.clone(),
                profile: self.profile.clone(),
                total_fresh: self.total_fresh,
                total_dirty: self.total_dirty,
                unit_data,
                concurrency,
                cpu_usage: &self.cpu_usage,
                rustc_version: rustc_version.into(),
                host: build_runner.bcx.rustc().host.to_string(),
                requested_targets,
                jobs: build_runner.bcx.jobs(),
                num_cpus,
                error,
            };
            report::write_html(ctx, &mut f)?;

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
