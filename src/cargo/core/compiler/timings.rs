//! Timing tracking.
//!
//! This module implements some simple tracking information for timing of how
//! long it takes for different units to compile.
use super::{CompileMode, Unit};
use crate::core::compiler::BuildContext;
use crate::core::PackageId;
use crate::util::machine_message::{self, Message};
use crate::util::{CargoResult, Config};
use std::cmp::max;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::time::{Duration, Instant, SystemTime};

pub struct Timings<'a, 'cfg> {
    config: &'cfg Config,
    /// If true, saves an HTML report to disk.
    report_html: bool,
    /// If true, reports unit completion to stderr.
    report_info: bool,
    /// If true, emits JSON information with timing information.
    report_json: bool,
    /// When Cargo started.
    start: Instant,
    /// A rendered string of when compilation started.
    start_str: String,
    /// Some information to display about rustc.
    rustc_info: String,
    /// A summary of the root units.
    ///
    /// Tuples of `(package_description, target_descrptions)`.
    root_targets: Vec<(String, Vec<String>)>,
    /// The build profile.
    profile: String,
    /// Total number of fresh units.
    total_fresh: u32,
    /// Total number of dirty units.
    total_dirty: u32,
    /// Time tracking for each individual unit.
    unit_times: Vec<UnitTime<'a>>,
    /// Units that are in the process of being built.
    /// When they finished, they are moved to `unit_times`.
    active: HashMap<u32, UnitTime<'a>>,
    /// Concurrency-tracking information. This is periodically updated while
    /// compilation progresses.
    concurrency: Vec<Concurrency>,
}

/// Tracking information for an individual unit.
struct UnitTime<'a> {
    unit: Unit<'a>,
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
    unlocked_units: Vec<Unit<'a>>,
    /// Same as `unlocked_units`, but unlocked by rmeta.
    unlocked_rmeta_units: Vec<Unit<'a>>,
}

/// Periodic concurrency tracking information.
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

impl<'a, 'cfg> Timings<'a, 'cfg> {
    pub fn new(bcx: &BuildContext<'a, 'cfg>, root_units: &[Unit<'_>]) -> Timings<'a, 'cfg> {
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
        let start_str = format!("{}", humantime::format_rfc3339_seconds(SystemTime::now()));
        let has_report = |what| {
            bcx.config
                .cli_unstable()
                .timings
                .as_ref()
                .map_or(false, |t| t.iter().any(|opt| opt == what))
        };
        let rustc_info = render_rustc_info(bcx);
        let profile = if bcx.build_config.release {
            "release"
        } else {
            "dev"
        }
        .to_string();

        Timings {
            config: bcx.config,
            report_html: has_report("html"),
            report_info: has_report("info"),
            report_json: has_report("json"),
            start: bcx.config.creation_time(),
            start_str,
            rustc_info,
            root_targets,
            profile,
            total_fresh: 0,
            total_dirty: 0,
            unit_times: Vec::new(),
            active: HashMap::new(),
            concurrency: Vec::new(),
        }
    }

    /// Mark that a unit has started running.
    pub fn unit_start(&mut self, id: u32, unit: Unit<'a>) {
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
            CompileMode::Bench => target.push_str(" (bench)"),
            CompileMode::Doc { .. } => target.push_str(" (doc)"),
            CompileMode::Doctest => target.push_str(" (doc test)"),
            CompileMode::RunCustomBuild => target.push_str(" (run)"),
        }
        let unit_time = UnitTime {
            unit,
            target,
            start: d_as_f64(self.start.elapsed()),
            duration: 0.0,
            rmeta_time: None,
            unlocked_units: Vec::new(),
            unlocked_rmeta_units: Vec::new(),
        };
        assert!(self.active.insert(id, unit_time).is_none());
    }

    /// Mark that the `.rmeta` file as generated.
    pub fn unit_rmeta_finished(&mut self, id: u32, unlocked: Vec<&Unit<'a>>) {
        if let Some(unit_time) = self.active.get_mut(&id) {
            let t = d_as_f64(self.start.elapsed());
            unit_time.rmeta_time = Some(t - unit_time.start);
            unit_time.unlocked_rmeta_units.extend(unlocked);
        }
    }

    /// Mark that a unit has finished running.
    pub fn unit_finished(&mut self, id: u32, unlocked: Vec<&Unit<'a>>) {
        if let Some(mut unit_time) = self.active.remove(&id) {
            let t = d_as_f64(self.start.elapsed());
            unit_time.duration = t - unit_time.start;
            unit_time.unlocked_units.extend(unlocked);
            if self.report_info {
                let msg = format!(
                    "{}{} in {:.1}s",
                    unit_time.name_ver(),
                    unit_time.target,
                    unit_time.duration
                );
                let _ =
                    self.config
                        .shell()
                        .status_with_color("Completed", msg, termcolor::Color::Cyan);
            }
            if self.report_json {
                let msg = machine_message::TimingInfo {
                    package_id: unit_time.unit.pkg.package_id(),
                    target: unit_time.unit.target,
                    mode: unit_time.unit.mode,
                    duration: unit_time.duration,
                    rmeta_time: unit_time.rmeta_time,
                }
                .to_json_string();
                self.config.shell().stdout_println(msg);
            }
            self.unit_times.push(unit_time);
        }
    }

    /// This is called periodically to mark the concurrency of internal structures.
    pub fn mark_concurrency(&mut self, active: usize, waiting: usize, inactive: usize) {
        let c = Concurrency {
            t: d_as_f64(self.start.elapsed()),
            active,
            waiting,
            inactive,
        };
        self.concurrency.push(c);
    }

    /// Mark that a fresh unit was encountered.
    pub fn add_fresh(&mut self) {
        self.total_fresh += 1;
    }

    /// Mark that a dirty unit was encountered.
    pub fn add_dirty(&mut self) {
        self.total_dirty += 1;
    }

    /// Call this when all units are finished.
    pub fn finished(&mut self) -> CargoResult<()> {
        self.mark_concurrency(0, 0, 0);
        self.unit_times
            .sort_unstable_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        if self.report_html {
            self.report_html()?;
        }
        Ok(())
    }

    /// Save HTML report to disk.
    pub fn report_html(&self) -> CargoResult<()> {
        let duration = self.start.elapsed().as_secs() as u32 + 1;
        let mut f = File::create("cargo-timing.html")?;
        let roots: Vec<&str> = self
            .root_targets
            .iter()
            .map(|(name, _targets)| name.as_str())
            .collect();
        f.write_all(HTML_TMPL.replace("{ROOTS}", &roots.join(", ")).as_bytes())?;
        self.fmt_summary_table(&mut f, duration)?;
        let graph_width = self.fmt_pipeline_graph(&mut f, duration)?;
        self.fmt_timing_graph(&mut f, graph_width, duration)?;
        self.fmt_unit_table(&mut f)?;
        f.write_all(HTML_TMPL_FOOT.as_bytes())?;
        Ok(())
    }

    /// Render the summary table.
    fn fmt_summary_table(&self, f: &mut File, duration: u32) -> CargoResult<()> {
        let targets: Vec<String> = self
            .root_targets
            .iter()
            .map(|(name, targets)| format!("{} ({})", name, targets.join(", ")))
            .collect();
        let targets = targets.join("<br>");
        let time_human = if duration > 60 {
            format!(" ({}m {:02}s)", duration / 60, duration % 60)
        } else {
            "".to_string()
        };
        let total_time = format!("{}s{}", duration, time_human);
        let max_concurrency = self.concurrency.iter().map(|c| c.active).max().unwrap();
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
    <td>Max concurrency:</td><td>{}</td>
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

</table>
"#,
            targets,
            self.profile,
            self.total_fresh,
            self.total_dirty,
            self.total_fresh + self.total_dirty,
            max_concurrency,
            self.start_str,
            total_time,
            self.rustc_info,
        )?;
        Ok(())
    }

    /// Render the box graph of the units over time.
    fn fmt_pipeline_graph(&self, f: &mut File, duration: u32) -> CargoResult<u32> {
        if self.unit_times.is_empty() {
            return Ok(0);
        }
        const BOX_HEIGHT: u32 = 25;
        const Y_TICK_DIST: u32 = BOX_HEIGHT + 2;

        let graph_height = Y_TICK_DIST * self.unit_times.len() as u32;

        let graph_width = draw_graph_axes(f, graph_height, duration)?;

        // Draw Y tick marks.
        write!(f, "<path class=\"graph-axes\" d=\"")?;
        for n in 1..self.unit_times.len() as u32 {
            let y = graph_height - (n * Y_TICK_DIST);
            write!(f, "M {} {} l -5 0 ", X_LINE, y)?;
        }
        writeln!(f, "\" />")?;

        // Draw Y labels.
        for n in 0..self.unit_times.len() as u32 {
            let y = MARGIN + (Y_TICK_DIST * (n + 1)) - 13;
            writeln!(
                f,
                r#"<text x="{}" y="{}" class="graph-label-v">{}</text>"#,
                X_LINE - 4,
                y,
                n + 1
            )?;
        }

        // Draw the graph.
        writeln!(
            f,
            r#"<svg x="{}" y="{}" width="{}" height="{}">"#,
            X_LINE, MARGIN, graph_width, graph_height
        )?;

        // Create a map that will be used for drawing dependency lines.
        let unit_map: HashMap<&Unit<'_>, (f64, u32)> = self
            .unit_times
            .iter()
            .enumerate()
            .map(|(i, unit)| {
                let y = i as u32 * Y_TICK_DIST + 1;
                let x = PX_PER_SEC * unit.start;
                (&unit.unit, (x, y))
            })
            .collect();

        for (i, unit) in self.unit_times.iter().enumerate() {
            let (x, y) = unit_map[&unit.unit];
            let width = (PX_PER_SEC * unit.duration).max(1.0);

            let dep_class = format!("dep-{}", i);
            writeln!(
                f,
                "  <rect x=\"{:.1}\" y=\"{}\" width=\"{:.1}\" height=\"{}\" \
                 rx=\"3\" class=\"unit-block\" data-dep-class=\"{}\" />",
                x, y, width, BOX_HEIGHT, dep_class,
            )?;
            let draw_dep_lines = |f: &mut File, x, units| -> CargoResult<()> {
                for unlocked in units {
                    let (u_x, u_y) = unit_map[&unlocked];
                    writeln!(
                        f,
                        "  <path class=\"{} dep-line\" d=\"M {:.1} {} l -5 0 l 0 {} l {:.1} 0\" />",
                        dep_class,
                        x,
                        y + BOX_HEIGHT / 2,
                        u_y - y,
                        u_x - x + 5.0,
                    )?;
                }
                Ok(())
            };
            if let Some((rmeta_time, ctime, _cent)) = unit.codegen_time() {
                let rmeta_x = x + PX_PER_SEC * rmeta_time;
                writeln!(
                    f,
                    "  <rect x=\"{:.1}\" y=\"{}\" width=\"{:.1}\" \
                     height=\"{}\" rx=\"3\" class=\"unit-block-codegen\"/>",
                    rmeta_x,
                    y,
                    PX_PER_SEC * ctime,
                    BOX_HEIGHT,
                )?;
                draw_dep_lines(f, rmeta_x, &unit.unlocked_rmeta_units)?;
            }
            writeln!(
                f,
                "  <text x=\"{:.1}\" y=\"{}\" class=\"unit-label\">{}{} {:.1}s</text>",
                x + 5.0,
                y + BOX_HEIGHT / 2 - 5,
                unit.unit.pkg.name(),
                unit.target,
                unit.duration
            )?;
            draw_dep_lines(f, x + width, &unit.unlocked_units)?;
        }
        writeln!(f, r#"</svg>"#)?;
        writeln!(f, r#"</svg>"#)?;
        Ok(graph_width)
    }

    /// Render the line chart of concurrency information.
    fn fmt_timing_graph(&self, f: &mut File, graph_width: u32, duration: u32) -> CargoResult<()> {
        if graph_width == 0 || self.concurrency.is_empty() {
            return Ok(());
        }
        const HEIGHT: u32 = 400;
        const AXIS_HEIGHT: u32 = HEIGHT - MARGIN - Y_LINE;
        const TOP_MARGIN: u32 = 10;
        const GRAPH_HEIGHT: u32 = AXIS_HEIGHT - TOP_MARGIN;

        draw_graph_axes(f, AXIS_HEIGHT, duration)?;

        // Draw Y tick marks and labels.
        write!(f, "<path class=\"graph-axes\" d=\"")?;
        let max_v = self
            .concurrency
            .iter()
            .map(|c| max(max(c.active, c.waiting), c.inactive))
            .max()
            .unwrap();
        let (step, top) = split_ticks(max_v as u32, GRAPH_HEIGHT / MIN_TICK_DIST);
        let num_ticks = top / step;
        let tick_dist = GRAPH_HEIGHT / num_ticks;
        let mut labels = String::new();

        for n in 0..num_ticks {
            let y = HEIGHT - Y_LINE - ((n + 1) * tick_dist);
            write!(f, "M {} {} l -5 0 ", X_LINE, y)?;
            labels.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" class=\"graph-label-v\">{}</text>\n",
                X_LINE - 10,
                y + 5,
                (n + 1) * step
            ));
        }
        writeln!(f, "\"/>")?;
        f.write_all(labels.as_bytes())?;

        // Label the Y axis.
        let label_y = (HEIGHT - Y_LINE) / 2;
        writeln!(
            f,
            "<text x=\"15\", y=\"{}\" \
             class=\"graph-label-v\" transform=\"rotate(-90, 15, {})\"># Units</text>",
            label_y, label_y
        )?;

        // Draw the graph.
        writeln!(
            f,
            r#"<svg x="{}" y="{}" width="{}" height="{}">"#,
            X_LINE,
            MARGIN,
            graph_width,
            GRAPH_HEIGHT + TOP_MARGIN
        )?;

        let coord = |t, v| {
            (
                f64::from(graph_width) * (t / f64::from(duration)),
                f64::from(TOP_MARGIN) + f64::from(GRAPH_HEIGHT) * (1.0 - (v as f64 / max_v as f64)),
            )
        };
        let mut draw_line = |class, key: fn(&Concurrency) -> usize| {
            write!(f, "<polyline points=\"")?;
            let first = &self.concurrency[0];
            let mut last = coord(first.t, key(first));
            for c in &self.concurrency {
                let (x, y) = coord(c.t, key(c));
                write!(f, "{:.1},{:.1} {:.1},{:.1} ", x, last.1, x, y)?;
                last = (x, y);
            }
            writeln!(f, "\" class=\"{}\" />", class)
        };

        draw_line("line-inactive", |c| c.inactive)?;
        draw_line("line-waiting", |c| c.waiting)?;
        draw_line("line-active", |c| c.active)?;

        // Draw a legend.
        write!(
            f,
            r#"
<svg x="{}" y="20" width="100" height="62">
  <rect width="100%" height="100%" fill="white" stroke="black" stroke-width="1" />
  <line x1="5" y1="10" x2="50" y2="10" stroke="red" stroke-width="2"/>
  <text x="54" y="11" dominant-baseline="middle" font-size="12px">Waiting</text>

  <line x1="5" y1="50" x2="50" y2="50" stroke="green" stroke-width="2"/>
  <text x="54" y="51" dominant-baseline="middle" font-size="12px">Active</text>

  <line x1="5" y1="30" x2="50" y2="30" stroke="blue" stroke-width="2"/>
  <text x="54" y="31" dominant-baseline="middle" font-size="12px">Inactive</text>
</svg>
"#,
            graph_width - 120
        )?;

        writeln!(f, "</svg>")?;
        writeln!(f, "</svg>")?;
        Ok(())
    }

    /// Render the table of all units.
    fn fmt_unit_table(&self, f: &mut File) -> CargoResult<()> {
        write!(
            f,
            r#"
<table class="my-table">
  <thead>
    <tr>
      <th></th>
      <th>Unit</th>
      <th>Total</th>
      <th>Codegen</th>
    </tr>
  </thead>
  <tbody>
"#
        )?;
        let mut units: Vec<&UnitTime<'_>> = self.unit_times.iter().collect();
        units.sort_unstable_by(|a, b| b.duration.partial_cmp(&a.duration).unwrap());
        for (i, unit) in units.iter().enumerate() {
            let codegen = match unit.codegen_time() {
                None => "".to_string(),
                Some((_rt, ctime, cent)) => format!("{:.1}s ({:.0}%)", ctime, cent),
            };
            write!(
                f,
                r#"
<tr>
  <td>{}.</td>
  <td>{}{}</td>
  <td>{:.1}s</td>
  <td>{}</td>
</tr>
"#,
                i + 1,
                unit.name_ver(),
                unit.target,
                unit.duration,
                codegen
            )?;
        }
        write!(f, "</tbody>\n</table>\n")?;
        Ok(())
    }
}

impl<'a> UnitTime<'a> {
    /// Returns the codegen time as (rmeta_time, codegen_time, percent of total)
    fn codegen_time(&self) -> Option<(f64, f64, f64)> {
        self.rmeta_time.map(|rmeta_time| {
            let ctime = self.duration - rmeta_time;
            let cent = (ctime / self.duration) * 100.0;
            (rmeta_time, ctime, cent)
        })
    }

    fn name_ver(&self) -> String {
        format!("{} v{}", self.unit.pkg.name(), self.unit.pkg.version())
    }
}

// Replace with as_secs_f64 when 1.38 hits stable.
fn d_as_f64(d: Duration) -> f64 {
    (d.as_secs() as f64) + f64::from(d.subsec_nanos()) / 1_000_000_000.0
}

fn round_up(n: u32, step: u32) -> u32 {
    if n % step == 0 {
        n
    } else {
        (step - n % step) + n
    }
}

/// Determine the `(step, max_value)` of the number of ticks along an axis.
fn split_ticks(n: u32, max_ticks: u32) -> (u32, u32) {
    if n <= max_ticks {
        (1, n)
    } else if n <= max_ticks * 2 {
        (2, round_up(n, 2))
    } else if n <= max_ticks * 4 {
        (4, round_up(n, 4))
    } else if n <= max_ticks * 5 {
        (5, round_up(n, 5))
    } else {
        let mut step = 10;
        loop {
            let top = round_up(n, step);
            if top <= max_ticks * step {
                break (step, top);
            }
            step += 10;
        }
    }
}

const X_LINE: u32 = 50;
const MARGIN: u32 = 5;
const Y_LINE: u32 = 35; // relative to bottom
const PX_PER_SEC: f64 = 20.0;
const MIN_TICK_DIST: u32 = 50;

fn draw_graph_axes(f: &mut File, graph_height: u32, duration: u32) -> CargoResult<u32> {
    let graph_width = PX_PER_SEC as u32 * duration;
    let width = graph_width + X_LINE + 30;
    let height = graph_height + MARGIN + Y_LINE;
    writeln!(
        f,
        r#"<svg width="{}" height="{}" class="graph">"#,
        width, height
    )?;

    // Draw axes.
    write!(
        f,
        "<path class=\"graph-axes\" d=\"\
         M {} {} \
         l 0 {} \
         l {} 0 ",
        X_LINE,
        MARGIN,
        graph_height,
        graph_width + 20
    )?;

    // Draw X tick marks.
    let tick_width = graph_width - 10;
    let (step, top) = split_ticks(duration, tick_width / MIN_TICK_DIST);
    let num_ticks = top / step;
    let tick_dist = tick_width / num_ticks;
    let mut labels = String::new();
    for n in 0..num_ticks {
        let x = X_LINE + ((n + 1) * tick_dist);
        write!(f, "M {} {} l 0 5 ", x, height - Y_LINE)?;
        labels.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" class=\"graph-label-h\">{}s</text>\n",
            x,
            height - Y_LINE + 20,
            (n + 1) * step
        ));
    }

    writeln!(f, "\" />")?;
    f.write_all(labels.as_bytes())?;

    // Draw vertical lines.
    write!(f, "<path class=\"vert-line\" d=\"")?;
    for n in 0..num_ticks {
        let x = X_LINE + ((n + 1) * tick_dist);
        write!(f, "M {} {} l 0 {} ", x, MARGIN, graph_height)?;
    }
    writeln!(f, "\" />")?;

    Ok(graph_width)
}

fn render_rustc_info(bcx: &BuildContext<'_, '_>) -> String {
    let version = bcx
        .rustc
        .verbose_version
        .lines()
        .next()
        .expect("rustc version");
    let requested_target = bcx
        .build_config
        .requested_target
        .as_ref()
        .map_or("Host", String::as_str);
    format!(
        "{}<br>Host: {}<br>Target: {}",
        version, bcx.rustc.host, requested_target
    )
}

static HTML_TMPL: &str = r#"
<html>
<head>
  <title>Cargo Build Timings — {ROOTS}</title>
  <meta charset="utf-8">
<style type="text/css">
html {
  font-family: sans-serif;
}
svg {
  margin-top: 5px;
  margin-bottom: 5px;
  background: #f7f7f7;
}
h1 {
  border-bottom: 1px solid #c0c0c0;
}
.unit-label {
  font-size: 12px;
  dominant-baseline: hanging;
}

.unit-block {
  fill: #95cce8;
}

.unit-block-codegen {
  fill: #aa95e8;
  pointer-events: none;
}

.graph {
  display: block;
}

.graph-label-v {
  text-anchor: end;
  fill: #303030;
}

.graph-label-h {
  text-anchor: middle;
  fill: #303030;
}

.graph-axes {
  stroke-width: 2;
  fill: none;
  stroke: black;
}

.vert-line {
  fill: none;
  stroke: #c0c0c0;
  stroke-dasharray: 2,4
}

.my-table {
  margin-top: 20px;
  margin-bottom: 20px;
  border-collapse: collapse;
  box-shadow: 0 5px 10px rgba(0, 0, 0, 0.1);
}

.my-table th {
  color: #d5dde5;
  background: #1b1e24;
  border-bottom: 4px solid #9ea7af;
  border-right: 1px solid #343a45;
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
  border-top: 1px solid #c1c3d1;
  border-bottom: 1px solid #c1c3d1;
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
  background: #ebebeb;
}

.my-table tr:last-child td:first-child {
  border-bottom-left-radius:3px;
}

.my-table tr:last-child td:last-child {
  border-bottom-right-radius:3px;
}

.my-table td {
  background: #ffffff;
  padding: 10px;
  text-align: left;
  vertical-align: middle;
  font-weight: 300;
  font-size: 14px;
  border-right: 1px solid #C1C3D1;
}

.my-table td:last-child {
  border-right: 0px;
}

.summary-table td:first-child {
  vertical-align: top;
  text-align: right;
}

.line-waiting {
  fill: none;
  stroke: red;
  stroke-width: 2px;
}

.line-active {
  fill: none;
  stroke: green;
  stroke-width: 2px;
}

.line-inactive {
  fill: none;
  stroke: blue;
  stroke-width: 2px;
}

.dep-line {
  fill: none;
  stroke: #ddd;
  stroke-dasharray: 2;
}

.dep-line-highlight {
  stroke: #3e3e3e;
  stroke-width: 2;
  stroke-dasharray: 4;
}

</style>
</head>
<body>

<h1>Cargo Build Timings</h1>
"#;

static HTML_TMPL_FOOT: &str = r#"
<script>
function show_deps(event) {
  for (const el of document.getElementsByClassName('dep-line')) {
    el.classList.remove('dep-line-highlight');
  }
  for (const el of document.getElementsByClassName(event.currentTarget.dataset.depClass)) {
    el.classList.add('dep-line-highlight');
  }
}

for (const el of document.getElementsByClassName('unit-block')) {
  el.onmouseover = show_deps;
}
</script>
</body>
</html>
"#;
