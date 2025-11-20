//! Render HTML report from timing tracking data.

use std::collections::HashMap;
use std::io::Write;

use itertools::Itertools as _;

use crate::CargoResult;
use crate::core::compiler::BuildContext;
use crate::core::compiler::BuildRunner;
use crate::core::compiler::CompilationSection;
use crate::core::compiler::Unit;
use crate::core::compiler::timings::Timings;

use super::UnitData;
use super::UnitTime;

const FRONTEND_SECTION_NAME: &str = "Frontend";
const CODEGEN_SECTION_NAME: &str = "Codegen";

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

/// Postprocessed section data that has both start and an end.
#[derive(Copy, Clone, serde::Serialize)]
pub(super) struct SectionData {
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

/// Writes an HTML report.
pub(super) fn write_html(
    ctx: &Timings<'_>,
    f: &mut impl Write,
    build_runner: &BuildRunner<'_, '_>,
    error: &Option<anyhow::Error>,
) -> CargoResult<()> {
    let duration = ctx.start.elapsed().as_secs_f64();
    let roots: Vec<&str> = ctx
        .root_targets
        .iter()
        .map(|(name, _targets)| name.as_str())
        .collect();
    f.write_all(HTML_TMPL.replace("{ROOTS}", &roots.join(", ")).as_bytes())?;
    write_summary_table(ctx, f, duration, build_runner.bcx, error)?;
    f.write_all(HTML_CANVAS.as_bytes())?;
    write_unit_table(ctx, f)?;
    // It helps with pixel alignment to use whole numbers.
    writeln!(
        f,
        "<script>\n\
         DURATION = {};",
        f64::ceil(duration) as u32
    )?;
    write_js_data(ctx, f)?;
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
    ctx: &Timings<'_>,
    f: &mut impl Write,
    duration: f64,
    bcx: &BuildContext<'_, '_>,
    error: &Option<anyhow::Error>,
) -> CargoResult<()> {
    let targets: Vec<String> = ctx
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
    let max_concurrency = ctx.concurrency.iter().map(|c| c.active).max().unwrap();
    let num_cpus = std::thread::available_parallelism()
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
        ctx.profile,
        ctx.total_fresh,
        ctx.total_dirty,
        ctx.total_fresh + ctx.total_dirty,
        max_concurrency,
        bcx.jobs(),
        num_cpus,
        ctx.start_str,
        total_time,
        rustc_info,
        error_msg,
    )?;
    Ok(())
}

/// Write timing data in JavaScript. Primarily for `timings.js` to put data
/// in a `<script>` HTML element to draw graphs.
fn write_js_data(ctx: &Timings<'_>, f: &mut impl Write) -> CargoResult<()> {
    let unit_data = to_unit_data(&ctx.unit_times);

    writeln!(
        f,
        "const UNIT_DATA = {};",
        serde_json::to_string_pretty(&unit_data)?
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
fn write_unit_table(ctx: &Timings<'_>, f: &mut impl Write) -> CargoResult<()> {
    let mut units: Vec<&UnitTime> = ctx.unit_times.iter().collect();
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
            match aggregate_sections(u) {
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

fn to_unit_data(unit_times: &[UnitTime]) -> Vec<UnitData> {
    // Create a map to link indices of unlocked units.
    let unit_map: HashMap<Unit, usize> = unit_times
        .iter()
        .enumerate()
        .map(|(i, ut)| (ut.unit.clone(), i))
        .collect();
    let round = |x: f64| (x * 100.0).round() / 100.0;
    unit_times
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
            let aggregated = aggregate_sections(ut);
            let sections = match aggregated {
                AggregatedSections::Sections(mut sections) => {
                    // We draw the sections in the pipeline graph in a way where the frontend
                    // section has the "default" build color, and then additional sections
                    // (codegen, link) are overlaid on top with a different color.
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
        .collect()
}

/// Aggregates section timing information from individual compilation sections.
fn aggregate_sections(unit_time: &UnitTime) -> AggregatedSections {
    let end = unit_time.duration;

    if !unit_time.sections.is_empty() {
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
        for (name, section) in unit_time.sections.clone() {
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
    } else if let Some(rmeta) = unit_time.rmeta_time {
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
