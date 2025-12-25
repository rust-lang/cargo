//! Render HTML report from timing tracking data.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;

use indexmap::IndexMap;
use itertools::Itertools as _;

use crate::CargoResult;
use crate::core::compiler::Unit;

use super::CompilationSection;
use super::UnitData;
use super::UnitTime;

/// Name of an individual compilation section.
#[derive(Clone, Hash, Eq, PartialEq)]
pub enum SectionName {
    Frontend,
    Codegen,
    Named(String),
    Other,
}

impl SectionName {
    /// Lower case name.
    fn name(&self) -> Cow<'static, str> {
        match self {
            SectionName::Frontend => "frontend".into(),
            SectionName::Codegen => "codegen".into(),
            SectionName::Named(n) => n.to_lowercase().into(),
            SectionName::Other => "other".into(),
        }
    }

    fn capitalized_name(&self) -> String {
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
        capitalize(&self.name())
    }
}

impl serde::ser::Serialize for SectionName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.name().serialize(serializer)
    }
}

/// Postprocessed section data that has both start and an end.
#[derive(Copy, Clone, serde::Serialize)]
pub struct SectionData {
    /// Start (relative to the start of the unit)
    pub start: f64,
    /// End (relative to the start of the unit)
    pub end: f64,
}

impl SectionData {
    fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// Concurrency tracking information.
#[derive(serde::Serialize)]
pub struct Concurrency {
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

pub struct RenderContext<'a> {
    /// A rendered string of when compilation started.
    pub start_str: String,
    /// A summary of the root units.
    ///
    /// Tuples of `(package_description, target_descriptions)`.
    pub root_units: Vec<(String, Vec<String>)>,
    /// The build profile.
    pub profile: String,
    /// Total number of fresh units.
    pub total_fresh: u32,
    /// Total number of dirty units.
    pub total_dirty: u32,
    /// Time tracking for each individual unit.
    pub unit_data: Vec<UnitData>,
    /// Concurrency-tracking information. This is periodically updated while
    /// compilation progresses.
    pub concurrency: Vec<Concurrency>,
    /// Recorded CPU states, stored as tuples. First element is when the
    /// recording was taken and second element is percentage usage of the
    /// system.
    pub cpu_usage: &'a [(f64, f64)],
    /// Compiler version info, i.e., `rustc 1.92.0-beta.2 (0a411606e 2025-10-31)`.
    pub rustc_version: String,
    /// The host triple (arch-platform-OS).
    pub host: String,
    /// The requested target platforms of compilation for this build.
    pub requested_targets: Vec<String>,
    /// The number of jobs specified for this build.
    pub jobs: u32,
    /// Available parallelism of the compilation environment.
    pub num_cpus: Option<u64>,
    /// Fatal error during the build.
    pub error: &'a Option<anyhow::Error>,
}

/// Writes an HTML report.
pub fn write_html(ctx: RenderContext<'_>, f: &mut impl Write) -> CargoResult<()> {
    // The last concurrency record should equal to the last unit finished time.
    let duration = ctx.concurrency.last().map(|c| c.t).unwrap_or(0.0);
    let roots: Vec<&str> = ctx
        .root_units
        .iter()
        .map(|(name, _targets)| name.as_str())
        .collect();
    f.write_all(HTML_TMPL.replace("{ROOTS}", &roots.join(", ")).as_bytes())?;
    write_summary_table(&ctx, f, duration)?;
    f.write_all(HTML_CANVAS.as_bytes())?;
    write_unit_table(&ctx, f)?;
    // It helps with pixel alignment to use whole numbers.
    writeln!(
        f,
        "<script>\n\
         DURATION = {};",
        f64::ceil(duration) as u32
    )?;
    write_js_data(&ctx, f)?;
    write!(
        f,
        "{}\n\
         </script>\n\
         </body>\n\
         </html>\n\
         ",
        include_str!("timings.js")
    )?;

    Ok(())
}

/// Render the summary table.
fn write_summary_table(
    ctx: &RenderContext<'_>,
    f: &mut impl Write,
    duration: f64,
) -> CargoResult<()> {
    let targets = ctx
        .root_units
        .iter()
        .map(|(name, targets)| format!("{} ({})", name, targets.join(", ")))
        .collect::<Vec<_>>()
        .join("<br>");

    let total_units = ctx.total_fresh + ctx.total_dirty;

    let time_human = if duration > 60.0 {
        format!(" ({}m {:.1}s)", duration as u32 / 60, duration % 60.0)
    } else {
        "".to_string()
    };
    let total_time = format!("{:.1}s{}", duration, time_human);

    let max_concurrency = ctx.concurrency.iter().map(|c| c.active).max().unwrap_or(0);
    let num_cpus = ctx
        .num_cpus
        .map(|x| x.to_string())
        .unwrap_or_else(|| "n/a".into());

    let requested_targets = ctx.requested_targets.join(", ");

    let error_msg = match ctx.error {
        Some(e) => format!(r#"<tr><td class="error-text">Error:</td><td>{e}</td></tr>"#),
        None => "".to_string(),
    };

    let RenderContext {
        start_str,
        profile,
        total_fresh,
        total_dirty,
        rustc_version,
        host,
        jobs,
        ..
    } = &ctx;

    write!(
        f,
        r#"
<table class="my-table summary-table">
<tr>
<td>Targets:</td><td>{targets}</td>
</tr>
<tr>
<td>Profile:</td><td>{profile}</td>
</tr>
<tr>
<td>Fresh units:</td><td>{total_fresh}</td>
</tr>
<tr>
<td>Dirty units:</td><td>{total_dirty}</td>
</tr>
<tr>
<td>Total units:</td><td>{total_units}</td>
</tr>
<tr>
<td>Max concurrency:</td><td>{max_concurrency} (jobs={jobs} ncpu={num_cpus})</td>
</tr>
<tr>
<td>Build start:</td><td>{start_str}</td>
</tr>
<tr>
<td>Total time:</td><td>{total_time}</td>
</tr>
<tr>
<td>rustc:</td><td>{rustc_version}<br>Host: {host}<br>Target: {requested_targets}</td>
</tr>
{error_msg}
</table>
"#,
    )?;
    Ok(())
}

/// Write timing data in JavaScript. Primarily for `timings.js` to put data
/// in a `<script>` HTML element to draw graphs.
fn write_js_data(ctx: &RenderContext<'_>, f: &mut impl Write) -> CargoResult<()> {
    writeln!(
        f,
        "const UNIT_DATA = {};",
        serde_json::to_string_pretty(&ctx.unit_data)?
    )?;
    writeln!(
        f,
        "const CONCURRENCY_DATA = {};",
        serde_json::to_string_pretty(&ctx.concurrency)?
    )?;
    writeln!(
        f,
        "const CPU_USAGE = {};",
        serde_json::to_string_pretty(&ctx.cpu_usage)?
    )?;
    Ok(())
}

/// Render the table of all units.
fn write_unit_table(ctx: &RenderContext<'_>, f: &mut impl Write) -> CargoResult<()> {
    let mut units: Vec<_> = ctx.unit_data.iter().collect();
    units.sort_unstable_by(|a, b| b.duration.partial_cmp(&a.duration).unwrap());

    let aggregated: Vec<Option<_>> = units.iter().map(|u| u.sections.as_ref()).collect();

    let headers: Vec<_> = aggregated
        .iter()
        .find_map(|s| s.as_ref())
        .map(|sections| {
            sections
                .iter()
                // We don't want to show the "Other" section in the table,
                // as it is usually a tiny portion out of the entire unit.
                .filter(|(name, _)| !matches!(name, SectionName::Other))
                .map(|s| s.0.clone())
                .collect()
        })
        .unwrap_or_default();

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
        headers = headers
            .iter()
            .map(|h| format!("<th>{}</th>", h.capitalized_name()))
            .join("\n")
    )?;

    for (i, (unit, aggregated_sections)) in units.iter().zip(aggregated).enumerate() {
        let format_duration = |section: Option<&SectionData>| match section {
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
        let mut cells: HashMap<_, _> = aggregated_sections
            .iter()
            .flat_map(|sections| sections.into_iter().map(|s| (&s.0, &s.1)))
            .collect();

        let cells = headers
            .iter()
            .map(|header| format!("<td>{}</td>", format_duration(cells.remove(header))))
            .join("\n");

        let features = unit.features.join(", ");
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
            format_args!("{} v{}", unit.name, unit.version),
            unit.target,
            unit.duration,
        )?;
    }
    write!(f, "</tbody>\n</table>\n")?;
    Ok(())
}

pub(super) fn to_unit_data(
    unit_times: &[UnitTime],
    unit_map: &HashMap<Unit, u64>,
) -> Vec<UnitData> {
    unit_times
        .iter()
        .map(|ut| (unit_map[&ut.unit], ut))
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
            let unblocked_units = ut
                .unblocked_units
                .iter()
                .filter_map(|unit| unit_map.get(unit).copied())
                .collect();
            let unblocked_rmeta_units = ut
                .unblocked_rmeta_units
                .iter()
                .filter_map(|unit| unit_map.get(unit).copied())
                .collect();
            let sections = aggregate_sections(ut.sections.clone(), ut.duration, ut.rmeta_time);

            UnitData {
                i,
                name: ut.unit.pkg.name().to_string(),
                version: ut.unit.pkg.version().to_string(),
                mode,
                target: ut.target.clone(),
                features: ut.unit.features.iter().map(|f| f.to_string()).collect(),
                start: round_to_centisecond(ut.start),
                duration: round_to_centisecond(ut.duration),
                unblocked_units,
                unblocked_rmeta_units,
                sections,
            }
        })
        .collect()
}

/// Derives concurrency information from unit timing data.
pub fn compute_concurrency(unit_data: &[UnitData]) -> Vec<Concurrency> {
    if unit_data.is_empty() {
        return Vec::new();
    }

    let unit_by_index: HashMap<_, _> = unit_data.iter().map(|u| (u.i, u)).collect();

    enum UnblockedBy {
        Rmeta(u64),
        Full(u64),
    }

    // unit_id -> unit that unblocks it.
    let mut unblocked_by: HashMap<_, _> = HashMap::new();
    for unit in unit_data {
        for id in unit.unblocked_rmeta_units.iter() {
            assert!(
                unblocked_by
                    .insert(*id, UnblockedBy::Rmeta(unit.i))
                    .is_none()
            );
        }

        for id in unit.unblocked_units.iter() {
            assert!(
                unblocked_by
                    .insert(*id, UnblockedBy::Full(unit.i))
                    .is_none()
            );
        }
    }

    let ready_time = |unit: &UnitData| -> Option<f64> {
        let dep = unblocked_by.get(&unit.i)?;
        match dep {
            UnblockedBy::Rmeta(id) => {
                let dep = unit_by_index.get(id)?;
                let duration = dep.sections.iter().flatten().find_map(|(name, section)| {
                    matches!(name, SectionName::Frontend).then_some(section.end)
                });

                Some(dep.start + duration.unwrap_or(dep.duration))
            }
            UnblockedBy::Full(id) => {
                let dep = unit_by_index.get(id)?;
                Some(dep.start + dep.duration)
            }
        }
    };

    #[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
    enum State {
        Ready,
        Start,
        End,
    }

    let mut events: Vec<_> = unit_data
        .iter()
        .flat_map(|unit| {
            // Adding rounded numbers may cause ready > start,
            // so cap with unit.start here to be defensive.
            let ready = ready_time(unit).unwrap_or(unit.start).min(unit.start);

            [
                (ready, State::Ready, unit.i),
                (unit.start, State::Start, unit.i),
                (unit.start + unit.duration, State::End, unit.i),
            ]
        })
        .collect();

    events.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap()
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });

    let mut concurrency: Vec<Concurrency> = Vec::new();
    let mut inactive: HashSet<u64> = unit_data.iter().map(|unit| unit.i).collect();
    let mut waiting: HashSet<u64> = HashSet::new();
    let mut active: HashSet<u64> = HashSet::new();

    for (t, state, unit_id) in events {
        match state {
            State::Ready => {
                inactive.remove(&unit_id);
                waiting.insert(unit_id);
                active.remove(&unit_id);
            }
            State::Start => {
                inactive.remove(&unit_id);
                waiting.remove(&unit_id);
                active.insert(unit_id);
            }
            State::End => {
                inactive.remove(&unit_id);
                waiting.remove(&unit_id);
                active.remove(&unit_id);
            }
        }

        let record = Concurrency {
            t,
            active: active.len(),
            waiting: waiting.len(),
            inactive: inactive.len(),
        };

        if let Some(last) = concurrency.last_mut()
            && last.t == t
        {
            // We don't want to draw long vertical lines at the same timestamp,
            // so we keep only the latest state.
            *last = record;
        } else {
            concurrency.push(record);
        }
    }

    concurrency
}

/// Aggregates section timing information from individual compilation sections.
///
/// We can have a bunch of situations here.
///
/// - `-Zsection-timings` is enabled, and we received some custom sections,
///   in which case we use them to determine the headers.
/// - We have at least one rmeta time, so we hard-code Frontend and Codegen headers.
/// - We only have total durations, so we don't add any additional headers.
pub fn aggregate_sections(
    sections: IndexMap<String, CompilationSection>,
    end: f64,
    rmeta_time: Option<f64>,
) -> Option<Vec<(SectionName, SectionData)>> {
    if !sections.is_empty() {
        // We have some detailed compilation section timings, so we postprocess them
        // Since it is possible that we do not have an end timestamp for a given compilation
        // section, we need to iterate them and if an end is missing, we assign the end of
        // the section to the start of the following section.
        let mut sections = sections.into_iter().fold(
            // The frontend section is currently implicit in rustc.
            // It is assumed to start at compilation start and end when codegen starts,
            // So we hard-code it here.
            vec![(
                SectionName::Frontend,
                SectionData {
                    start: 0.0,
                    end: round_to_centisecond(end),
                },
            )],
            |mut sections, (name, section)| {
                let previous = sections.last_mut().unwrap();
                // Setting the end of previous to the start of the current.
                previous.1.end = section.start;

                sections.push((
                    SectionName::Named(name),
                    SectionData {
                        start: round_to_centisecond(section.start),
                        end: round_to_centisecond(section.end.unwrap_or(end)),
                    },
                ));

                sections
            },
        );

        // We draw the sections in the pipeline graph in a way where the frontend
        // section has the "default" build color, and then additional sections
        // (codegen, link) are overlaid on top with a different color.
        // However, there might be some time after the final (usually link) section,
        // which definitely shouldn't be classified as "Frontend". We thus try to
        // detect this situation and add a final "Other" section.
        if let Some((_, section)) = sections.last()
            && section.end < end
        {
            sections.push((
                SectionName::Other,
                SectionData {
                    start: round_to_centisecond(section.end),
                    end: round_to_centisecond(end),
                },
            ));
        }
        Some(sections)
    } else if let Some(rmeta) = rmeta_time {
        // We only know when the rmeta time was generated
        Some(vec![
            (
                SectionName::Frontend,
                SectionData {
                    start: 0.0,
                    end: round_to_centisecond(rmeta),
                },
            ),
            (
                SectionName::Codegen,
                SectionData {
                    start: round_to_centisecond(rmeta),
                    end: round_to_centisecond(end),
                },
            ),
        ])
    } else {
        // No section data provided. We only know the total duration.
        None
    }
}

/// Rounds seconds to 0.01s precision.
pub fn round_to_centisecond(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
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

.canvas-container.hidden {
  display: none;
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
    <td>Renderer:</td>
  </tr>
  <tr>
    <td><input type="range" min="0" max="30" step="0.1" value="0" id="min-unit-time"></td>
    <!--
        The scale corresponds to some number of "pixels per second".
        Its min, max, and initial values are automatically set by JavaScript on page load,
        based on the client viewport.
    -->
    <td><input type="range" min="1" max="100" value="50" id="scale"></td>
    <td>
        <label>
            <input type="radio" name="renderer" value="canvas" checked />
            Canvas
        </label>
        <label>
            <input type="radio" name="renderer" value="svg" />
            SVG
        </label>
    </td>
  </tr>
  <tr>
    <td><output for="min-unit-time" id="min-unit-time-output"></output></td>
    <td><output for="scale" id="scale-output"></output></td>
    <td></td>
  </tr>
</table>

<div id="pipeline-container" class="canvas-container" part="canvas">
 <canvas id="pipeline-graph" class="graph" style="position: absolute; left: 0; top: 0; z-index: 0;"></canvas>
 <canvas id="pipeline-graph-lines" style="position: absolute; left: 0; top: 0; z-index: 1; pointer-events:none;"></canvas>
</div>
<div class="canvas-container" part="canvas">
  <canvas id="timing-graph" class="graph"></canvas>
</div>
<div id="pipeline-container-svg" class="canvas-container" part="svg"></div>
<div id="timing-container-svg" class="canvas-container" part="svg"></div>
"#;
