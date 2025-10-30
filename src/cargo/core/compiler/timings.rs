//! Timing tracking.
//!
//! This module implements some simple tracking information for timing of how
//! long it takes for different units to compile.
use super::{CompileMode, Unit};
use crate::core::PackageId;
use crate::core::compiler::job_queue::JobId;
use crate::core::compiler::{BuildContext, BuildRunner, TimingOutput};
use crate::util::cpu::State;
use crate::util::log_message::LogMessage;
use crate::util::machine_message::{self, Message};
use crate::util::style;
use crate::util::{CargoResult, GlobalContext};
use anyhow::Context as _;
use cargo_util::paths;
use indexmap::IndexMap;
use itertools::Itertools;
use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::thread::available_parallelism;
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

const FRONTEND_SECTION_NAME: &str = "Frontend";
const CODEGEN_SECTION_NAME: &str = "Codegen";

impl UnitTime {
    fn aggregate_sections(&self) -> AggregatedSections {
        let end = self.duration;

        if !self.sections.is_empty() {
            // We have some detailed compilation section timings, so we postprocess them
            // Since it is possible that we do not have an end timestamp for a given compilation
            // section, we need to iterate them and if an end is missing, we assign the end of
            // the section to the start of the following section.

            let mut sections = vec![];

            // The frontend section is currently implicit in rustc, it is assumed to start at
            // compilation start and end when codegen starts. So we hard-code it here.
            let mut previous_section = (
                FRONTEND_SECTION_NAME.to_string(),
                CompilationSection {
                    start: 0.0,
                    end: None,
                },
            );
            for (name, section) in self.sections.clone() {
                // Store the previous section, potentially setting its end to the start of the
                // current section.
                sections.push((
                    previous_section.0.clone(),
                    SectionData {
                        start: previous_section.1.start,
                        end: previous_section.1.end.unwrap_or(section.start),
                    },
                ));
                previous_section = (name, section);
            }
            // Store the last section, potentially setting its end to the end of the whole
            // compilation.
            sections.push((
                previous_section.0.clone(),
                SectionData {
                    start: previous_section.1.start,
                    end: previous_section.1.end.unwrap_or(end),
                },
            ));

            AggregatedSections::Sections(sections)
        } else if let Some(rmeta) = self.rmeta_time {
            // We only know when the rmeta time was generated
            AggregatedSections::OnlyMetadataTime {
                frontend: SectionData {
                    start: 0.0,
                    end: rmeta,
                },
                codegen: SectionData { start: rmeta, end },
            }
        } else {
            // We only know the total duration
            AggregatedSections::OnlyTotalDuration
        }
    }
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

/// Postprocessed section data that has both start and an end.
#[derive(Copy, Clone, serde::Serialize)]
struct SectionData {
    /// Start (relative to the start of the unit)
    start: f64,
    /// End (relative to the start of the unit)
    end: f64,
}

impl SectionData {
    fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// Contains post-processed data of individual compilation sections.
enum AggregatedSections {
    /// We know the names and durations of individual compilation sections
    Sections(Vec<(String, SectionData)>),
    /// We only know when .rmeta was generated, so we can distill frontend and codegen time.
    OnlyMetadataTime {
        frontend: SectionData,
        codegen: SectionData,
    },
    /// We know only the total duration
    OnlyTotalDuration,
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
            self.report_html(build_runner, error)
                .context("failed to save timing report")?;
        }
        Ok(())
    }

    /// Save HTML report to disk.
    fn report_html(
        &self,
        build_runner: &BuildRunner<'_, '_>,
        error: &Option<anyhow::Error>,
    ) -> CargoResult<()> {
        let duration = self.start.elapsed().as_secs_f64();
        let timestamp = self.start_str.replace(&['-', ':'][..], "");
        let timings_path = build_runner.files().timings_dir();
        paths::create_dir_all(&timings_path)?;
        let filename = timings_path.join(format!("cargo-timing-{}.html", timestamp));
        let mut f = BufWriter::new(paths::create(&filename)?);
        let roots: Vec<&str> = self
            .root_targets
            .iter()
            .map(|(name, _targets)| name.as_str())
            .collect();
        f.write_all(HTML_TMPL.replace("{ROOTS}", &roots.join(", ")).as_bytes())?;
        self.write_summary_table(&mut f, duration, build_runner.bcx, error)?;
        f.write_all(HTML_CANVAS.as_bytes())?;
        self.write_unit_table(&mut f)?;
        // It helps with pixel alignment to use whole numbers.
        writeln!(
            f,
            "<script>\n\
             DURATION = {};",
            f64::ceil(duration) as u32
        )?;
        self.write_js_data(&mut f)?;
        write!(
            f,
            "{}\n\
             </script>\n\
             </body>\n\
             </html>\n\
             ",
            include_str!("timings.js")
        )?;
        drop(f);

        let unstamped_filename = timings_path.join("cargo-timing.html");
        paths::link_or_copy(&filename, &unstamped_filename)?;

        let mut shell = self.gctx.shell();
        let timing_path = std::env::current_dir().unwrap_or_default().join(&filename);
        let link = shell.err_file_hyperlink(&timing_path);
        let msg = format!("report saved to {link}{}{link:#}", timing_path.display(),);
        shell.status_with_color("Timing", msg, &style::NOTE)?;

        Ok(())
    }

    /// Render the summary table.
    fn write_summary_table(
        &self,
        f: &mut impl Write,
        duration: f64,
        bcx: &BuildContext<'_, '_>,
        error: &Option<anyhow::Error>,
    ) -> CargoResult<()> {
        let targets: Vec<String> = self
            .root_targets
            .iter()
            .map(|(name, targets)| format!("{} ({})", name, targets.join(", ")))
            .collect();
        let targets = targets.join("<br>");
        let time_human = if duration > 60.0 {
            format!(" ({}m {:.1}s)", duration as u32 / 60, duration % 60.0)
        } else {
            "".to_string()
        };
        let total_time = format!("{:.1}s{}", duration, time_human);
        let max_concurrency = self.concurrency.iter().map(|c| c.active).max().unwrap();
        let num_cpus = available_parallelism()
            .map(|x| x.get().to_string())
            .unwrap_or_else(|_| "n/a".into());
        let rustc_info = render_rustc_info(bcx);
        let error_msg = match error {
            Some(e) => format!(r#"<tr><td class="error-text">Error:</td><td>{e}</td></tr>"#),
            None => "".to_string(),
        };
        write!(
            f,
            r#"
<table class="my-table summary-table">
  <tr>
    <td>Targets:</td><td>{}</td>
  </tr>
  <tr>
    <td>Profile:</td><td>{}</td>
  </tr>
  <tr>
    <td>Fresh units:</td><td>{}</td>
  </tr>
  <tr>
    <td>Dirty units:</td><td>{}</td>
  </tr>
  <tr>
    <td>Total units:</td><td>{}</td>
  </tr>
  <tr>
    <td>Max concurrency:</td><td>{} (jobs={} ncpu={})</td>
  </tr>
  <tr>
    <td>Build start:</td><td>{}</td>
  </tr>
  <tr>
    <td>Total time:</td><td>{}</td>
  </tr>
  <tr>
    <td>rustc:</td><td>{}</td>
  </tr>
{}
</table>
"#,
            targets,
            self.profile,
            self.total_fresh,
            self.total_dirty,
            self.total_fresh + self.total_dirty,
            max_concurrency,
            bcx.jobs(),
            num_cpus,
            self.start_str,
            total_time,
            rustc_info,
            error_msg,
        )?;
        Ok(())
    }

    /// Write timing data in JavaScript. Primarily for `timings.js` to put data
    /// in a `<script>` HTML element to draw graphs.
    fn write_js_data(&self, f: &mut impl Write) -> CargoResult<()> {
        // Create a map to link indices of unlocked units.
        let unit_map: HashMap<Unit, usize> = self
            .unit_times
            .iter()
            .enumerate()
            .map(|(i, ut)| (ut.unit.clone(), i))
            .collect();
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
            sections: Option<Vec<(String, SectionData)>>,
        }
        let round = |x: f64| (x * 100.0).round() / 100.0;
        let unit_data: Vec<UnitData> = self
            .unit_times
            .iter()
            .enumerate()
            .map(|(i, ut)| {
                let mode = if ut.unit.mode.is_run_custom_build() {
                    "run-custom-build"
                } else {
                    "todo"
                }
                .to_string();
                // These filter on the unlocked units because not all unlocked
                // units are actually "built". For example, Doctest mode units
                // don't actually generate artifacts.
                let unlocked_units: Vec<usize> = ut
                    .unlocked_units
                    .iter()
                    .filter_map(|unit| unit_map.get(unit).copied())
                    .collect();
                let unlocked_rmeta_units: Vec<usize> = ut
                    .unlocked_rmeta_units
                    .iter()
                    .filter_map(|unit| unit_map.get(unit).copied())
                    .collect();
                let aggregated = ut.aggregate_sections();
                let sections = match aggregated {
                    AggregatedSections::Sections(mut sections) => {
                        // We draw the sections in the pipeline graph in a way where the frontend
                        // section has the "default" build color, and then additional sections
                        // (codegen, link) are overlayed on top with a different color.
                        // However, there might be some time after the final (usually link) section,
                        // which definitely shouldn't be classified as "Frontend". We thus try to
                        // detect this situation and add a final "Other" section.
                        if let Some((_, section)) = sections.last()
                            && section.end < ut.duration
                        {
                            sections.push((
                                "other".to_string(),
                                SectionData {
                                    start: section.end,
                                    end: ut.duration,
                                },
                            ));
                        }

                        Some(sections)
                    }
                    AggregatedSections::OnlyMetadataTime { .. }
                    | AggregatedSections::OnlyTotalDuration => None,
                };

                UnitData {
                    i,
                    name: ut.unit.pkg.name().to_string(),
                    version: ut.unit.pkg.version().to_string(),
                    mode,
                    target: ut.target.clone(),
                    start: round(ut.start),
                    duration: round(ut.duration),
                    rmeta_time: ut.rmeta_time.map(round),
                    unlocked_units,
                    unlocked_rmeta_units,
                    sections,
                }
            })
            .collect();
        writeln!(
            f,
            "const UNIT_DATA = {};",
            serde_json::to_string_pretty(&unit_data)?
        )?;
        writeln!(
            f,
            "const CONCURRENCY_DATA = {};",
            serde_json::to_string_pretty(&self.concurrency)?
        )?;
        writeln!(
            f,
            "const CPU_USAGE = {};",
            serde_json::to_string_pretty(&self.cpu_usage)?
        )?;
        Ok(())
    }

    /// Render the table of all units.
    fn write_unit_table(&self, f: &mut impl Write) -> CargoResult<()> {
        let mut units: Vec<&UnitTime> = self.unit_times.iter().collect();
        units.sort_unstable_by(|a, b| b.duration.partial_cmp(&a.duration).unwrap());

        // Make the first "letter" uppercase. We could probably just assume ASCII here, but this
        // should be Unicode compatible.
        fn capitalize(s: &str) -> String {
            let first_char = s
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default();
            format!("{first_char}{}", s.chars().skip(1).collect::<String>())
        }

        // We can have a bunch of situations here.
        // - -Zsection-timings is enabled, and we received some custom sections, in which
        // case we use them to determine the headers.
        // - We have at least one rmeta time, so we hard-code Frontend and Codegen headers.
        // - We only have total durations, so we don't add any additional headers.
        let aggregated: Vec<AggregatedSections> = units
            .iter()
            .map(|u|
                // Normalize the section names so that they are capitalized, so that we can later
                // refer to them with the capitalized name both when computing headers and when
                // looking up cells.
                match u.aggregate_sections() {
                    AggregatedSections::Sections(sections) => AggregatedSections::Sections(
                        sections.into_iter()
                            .map(|(name, data)| (capitalize(&name), data))
                            .collect()
                    ),
                    s => s
                })
            .collect();

        let headers: Vec<String> = if let Some(sections) = aggregated.iter().find_map(|s| match s {
            AggregatedSections::Sections(sections) => Some(sections),
            _ => None,
        }) {
            sections.into_iter().map(|s| s.0.clone()).collect()
        } else if aggregated
            .iter()
            .any(|s| matches!(s, AggregatedSections::OnlyMetadataTime { .. }))
        {
            vec![
                FRONTEND_SECTION_NAME.to_string(),
                CODEGEN_SECTION_NAME.to_string(),
            ]
        } else {
            vec![]
        };

        write!(
            f,
            r#"
<table class="my-table">
  <thead>
    <tr>
      <th></th>
      <th>Unit</th>
      <th>Total</th>
      {headers}
      <th>Features</th>
    </tr>
  </thead>
  <tbody>
"#,
            headers = headers.iter().map(|h| format!("<th>{h}</th>")).join("\n")
        )?;

        for (i, (unit, aggregated_sections)) in units.iter().zip(aggregated).enumerate() {
            let format_duration = |section: Option<SectionData>| match section {
                Some(section) => {
                    let duration = section.duration();
                    let pct = (duration / unit.duration) * 100.0;
                    format!("{duration:.1}s ({:.0}%)", pct)
                }
                None => "".to_string(),
            };

            // This is a bit complex, as we assume the most general option - we can have an
            // arbitrary set of headers, and an arbitrary set of sections per unit, so we always
            // initiate the cells to be empty, and then try to find a corresponding column for which
            // we might have data.
            let mut cells: HashMap<&str, SectionData> = Default::default();

            match &aggregated_sections {
                AggregatedSections::Sections(sections) => {
                    for (name, data) in sections {
                        cells.insert(&name, *data);
                    }
                }
                AggregatedSections::OnlyMetadataTime { frontend, codegen } => {
                    cells.insert(FRONTEND_SECTION_NAME, *frontend);
                    cells.insert(CODEGEN_SECTION_NAME, *codegen);
                }
                AggregatedSections::OnlyTotalDuration => {}
            };
            let cells = headers
                .iter()
                .map(|header| {
                    format!(
                        "<td>{}</td>",
                        format_duration(cells.remove(header.as_str()))
                    )
                })
                .join("\n");

            let features = unit.unit.features.join(", ");
            write!(
                f,
                r#"
<tr>
  <td>{}.</td>
  <td>{}{}</td>
  <td>{:.1}s</td>
  {cells}
  <td>{features}</td>
</tr>
"#,
                i + 1,
                unit.name_ver(),
                unit.target,
                unit.duration,
            )?;
        }
        write!(f, "</tbody>\n</table>\n")?;
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

fn render_rustc_info(bcx: &BuildContext<'_, '_>) -> String {
    let version = bcx
        .rustc()
        .verbose_version
        .lines()
        .next()
        .expect("rustc version");
    let requested_target = bcx
        .build_config
        .requested_kinds
        .iter()
        .map(|kind| bcx.target_data.short_name(kind))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}<br>Host: {}<br>Target: {}",
        version,
        bcx.rustc().host,
        requested_target
    )
}

static HTML_TMPL: &str = r#"
<html>
<head>
  <title>Cargo Build Timings — {ROOTS}</title>
  <meta charset="utf-8">
<style type="text/css">
:root {
  --error-text: #e80000;
  --text: #000;
  --background: #fff;
  --h1-border-bottom: #c0c0c0;
  --table-box-shadow: rgba(0, 0, 0, 0.1);
  --table-th: #d5dde5;
  --table-th-background: #1b1e24;
  --table-th-border-bottom: #9ea7af;
  --table-th-border-right: #343a45;
  --table-tr-border-top: #c1c3d1;
  --table-tr-border-bottom: #c1c3d1;
  --table-tr-odd-background: #ebebeb;
  --table-td-background: #ffffff;
  --table-td-border-right: #C1C3D1;
  --canvas-background: #f7f7f7;
  --canvas-axes: #303030;
  --canvas-grid: #e6e6e6;
  --canvas-codegen: #aa95e8;
  --canvas-link: #95e8aa;
  --canvas-other: #e895aa;
  --canvas-custom-build: #f0b165;
  --canvas-not-custom-build: #95cce8;
  --canvas-dep-line: #ddd;
  --canvas-dep-line-highlighted: #000;
  --canvas-cpu: rgba(250, 119, 0, 0.2);
}

@media (prefers-color-scheme: dark) {
  :root {
    --error-text: #e80000;
    --text: #fff;
    --background: #121212;
    --h1-border-bottom: #444;
    --table-box-shadow: rgba(255, 255, 255, 0.1);
    --table-th: #a0a0a0;
    --table-th-background: #2c2c2c;
    --table-th-border-bottom: #555;
    --table-th-border-right: #444;
    --table-tr-border-top: #333;
    --table-tr-border-bottom: #333;
    --table-tr-odd-background: #1e1e1e;
    --table-td-background: #262626;
    --table-td-border-right: #333;
    --canvas-background: #1a1a1a;
    --canvas-axes: #b0b0b0;
    --canvas-grid: #333;
    --canvas-block: #aa95e8;
    --canvas-custom-build: #f0b165;
    --canvas-not-custom-build: #95cce8;
    --canvas-dep-line: #444;
    --canvas-dep-line-highlighted: #fff;
    --canvas-cpu: rgba(250, 119, 0, 0.2);
  }
}

html {
  font-family: sans-serif;
  color: var(--text);
  background: var(--background);
}

.canvas-container {
  position: relative;
  margin-top: 5px;
  margin-bottom: 5px;
}

h1 {
  border-bottom: 1px solid var(--h1-border-bottom);
}

.graph {
  display: block;
}

.my-table {
  margin-top: 20px;
  margin-bottom: 20px;
  border-collapse: collapse;
  box-shadow: 0 5px 10px var(--table-box-shadow);
}

.my-table th {
  color: var(--table-th);
  background: var(--table-th-background);
  border-bottom: 4px solid var(--table-th-border-bottom);
  border-right: 1px solid var(--table-th-border-right);
  font-size: 18px;
  font-weight: 100;
  padding: 12px;
  text-align: left;
  vertical-align: middle;
}

.my-table th:first-child {
  border-top-left-radius: 3px;
}

.my-table th:last-child {
  border-top-right-radius: 3px;
  border-right:none;
}

.my-table tr {
  border-top: 1px solid var(--table-tr-border-top);
  border-bottom: 1px solid var(--table-tr-border-bottom);
  font-size: 16px;
  font-weight: normal;
}

.my-table tr:first-child {
  border-top:none;
}

.my-table tr:last-child {
  border-bottom:none;
}

.my-table tr:nth-child(odd) td {
  background: var(--table-tr-odd-background);
}

.my-table tr:last-child td:first-child {
  border-bottom-left-radius:3px;
}

.my-table tr:last-child td:last-child {
  border-bottom-right-radius:3px;
}

.my-table td {
  background: var(--table-td-background);
  padding: 10px;
  text-align: left;
  vertical-align: middle;
  font-weight: 300;
  font-size: 14px;
  border-right: 1px solid var(--table-td-border-right);
}

.my-table td:last-child {
  border-right: 0px;
}

.summary-table td:first-child {
  vertical-align: top;
  text-align: right;
}

.input-table td {
  text-align: center;
}

.error-text {
  color: var(--error-text);
}

</style>
</head>
<body>

<h1>Cargo Build Timings</h1>
See <a href="https://doc.rust-lang.org/nightly/cargo/reference/timings.html">Documentation</a>
"#;

static HTML_CANVAS: &str = r#"
<table class="input-table">
  <tr>
    <td><label for="min-unit-time">Min unit time:</label></td>
    <td title="Scale corresponds to a number of pixels per second. It is automatically initialized based on your viewport width.">
      <label for="scale">Scale:</label>
    </td>
  </tr>
  <tr>
    <td><input type="range" min="0" max="30" step="0.1" value="0" id="min-unit-time"></td>
    <!--
        The scale corresponds to some number of "pixels per second".
        Its min, max, and initial values are automatically set by JavaScript on page load,
        based on the client viewport.
    -->
    <td><input type="range" min="1" max="100" value="50" id="scale"></td>
  </tr>
  <tr>
    <td><output for="min-unit-time" id="min-unit-time-output"></output></td>
    <td><output for="scale" id="scale-output"></output></td>
  </tr>
</table>

<div id="pipeline-container" class="canvas-container">
 <canvas id="pipeline-graph" class="graph" style="position: absolute; left: 0; top: 0; z-index: 0;"></canvas>
 <canvas id="pipeline-graph-lines" style="position: absolute; left: 0; top: 0; z-index: 1; pointer-events:none;"></canvas>
</div>
<div class="canvas-container">
  <canvas id="timing-graph" class="graph"></canvas>
</div>
"#;
