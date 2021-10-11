//! Support for future-incompatible warning reporting.

use crate::core::{Dependency, PackageId, Workspace};
use crate::sources::SourceConfigMap;
use crate::util::{iter_join, CargoResult, Config};
use anyhow::{bail, format_err, Context};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::{Read, Write};

pub const REPORT_PREAMBLE: &str = "\
The following warnings were discovered during the build. These warnings are an
indication that the packages contain code that will become an error in a
future release of Rust. These warnings typically cover changes to close
soundness problems, unintended or undocumented behavior, or critical problems
that cannot be fixed in a backwards-compatible fashion, and are not expected
to be in wide use.

Each warning should contain a link for more information on what the warning
means and how to resolve it.
";

/// Current version of the on-disk format.
const ON_DISK_VERSION: u32 = 0;

/// The future incompatibility report, emitted by the compiler as a JSON message.
#[derive(serde::Deserialize)]
pub struct FutureIncompatReport {
    pub future_incompat_report: Vec<FutureBreakageItem>,
}

/// Structure used for collecting reports in-memory.
pub struct FutureIncompatReportPackage {
    pub package_id: PackageId,
    pub items: Vec<FutureBreakageItem>,
}

/// A single future-incompatible warning emitted by rustc.
#[derive(Serialize, Deserialize)]
pub struct FutureBreakageItem {
    /// The date at which this lint will become an error.
    /// Currently unused
    pub future_breakage_date: Option<String>,
    /// The original diagnostic emitted by the compiler
    pub diagnostic: Diagnostic,
}

/// A diagnostic emitted by the compiler as a JSON message.
/// We only care about the 'rendered' field
#[derive(Serialize, Deserialize)]
pub struct Diagnostic {
    pub rendered: String,
    pub level: String,
}

/// The filename in the top-level `target` directory where we store
/// the report
const FUTURE_INCOMPAT_FILE: &str = ".future-incompat-report.json";
/// Max number of reports to save on disk.
const MAX_REPORTS: usize = 5;

/// The structure saved to disk containing the reports.
#[derive(Serialize, Deserialize)]
pub struct OnDiskReports {
    /// A schema version number, to handle older cargo's from trying to read
    /// something that they don't understand.
    version: u32,
    /// The report ID to use for the next report to save.
    next_id: u32,
    /// Available reports.
    reports: Vec<OnDiskReport>,
}

/// A single report for a given compilation session.
#[derive(Serialize, Deserialize)]
struct OnDiskReport {
    /// Unique reference to the report for the `--id` CLI flag.
    id: u32,
    /// Report, suitable for printing to the console.
    report: String,
}

impl Default for OnDiskReports {
    fn default() -> OnDiskReports {
        OnDiskReports {
            version: ON_DISK_VERSION,
            next_id: 1,
            reports: Vec::new(),
        }
    }
}

impl OnDiskReports {
    /// Saves a new report.
    pub fn save_report(
        ws: &Workspace<'_>,
        per_package_reports: &[FutureIncompatReportPackage],
    ) -> OnDiskReports {
        let mut current_reports = match Self::load(ws) {
            Ok(r) => r,
            Err(e) => {
                log::debug!(
                    "saving future-incompatible reports failed to load current reports: {:?}",
                    e
                );
                OnDiskReports::default()
            }
        };
        let report = OnDiskReport {
            id: current_reports.next_id,
            report: render_report(ws, per_package_reports),
        };
        current_reports.next_id += 1;
        current_reports.reports.push(report);
        if current_reports.reports.len() > MAX_REPORTS {
            current_reports.reports.remove(0);
        }
        let on_disk = serde_json::to_vec(&current_reports).unwrap();
        if let Err(e) = ws
            .target_dir()
            .open_rw(
                FUTURE_INCOMPAT_FILE,
                ws.config(),
                "Future incompatibility report",
            )
            .and_then(|file| {
                let mut file = file.file();
                file.set_len(0)?;
                file.write_all(&on_disk)?;
                Ok(())
            })
        {
            crate::display_warning_with_error(
                "failed to write on-disk future incompatible report",
                &e,
                &mut ws.config().shell(),
            );
        }
        current_reports
    }

    /// Loads the on-disk reports.
    pub fn load(ws: &Workspace<'_>) -> CargoResult<OnDiskReports> {
        let report_file = match ws.target_dir().open_ro(
            FUTURE_INCOMPAT_FILE,
            ws.config(),
            "Future incompatible report",
        ) {
            Ok(r) => r,
            Err(e) => {
                if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                    if io_err.kind() == std::io::ErrorKind::NotFound {
                        bail!("no reports are currently available");
                    }
                }
                return Err(e);
            }
        };

        let mut file_contents = String::new();
        report_file
            .file()
            .read_to_string(&mut file_contents)
            .with_context(|| "failed to read report")?;
        let on_disk_reports: OnDiskReports =
            serde_json::from_str(&file_contents).with_context(|| "failed to load report")?;
        if on_disk_reports.version != ON_DISK_VERSION {
            bail!("unable to read reports; reports were saved from a future version of Cargo");
        }
        Ok(on_disk_reports)
    }

    /// Returns the most recent report ID.
    pub fn last_id(&self) -> u32 {
        self.reports.last().map(|r| r.id).unwrap()
    }

    pub fn get_report(&self, id: u32, config: &Config) -> CargoResult<String> {
        let report = self.reports.iter().find(|r| r.id == id).ok_or_else(|| {
            let available = iter_join(self.reports.iter().map(|r| r.id.to_string()), ", ");
            format_err!(
                "could not find report with ID {}\n\
                 Available IDs are: {}",
                id,
                available
            )
        })?;
        let report = if config.shell().err_supports_color() {
            report.report.clone()
        } else {
            strip_ansi_escapes::strip(&report.report)
                .map(|v| String::from_utf8(v).expect("utf8"))
                .expect("strip should never fail")
        };
        Ok(report)
    }
}

fn render_report(
    ws: &Workspace<'_>,
    per_package_reports: &[FutureIncompatReportPackage],
) -> String {
    let mut per_package_reports: Vec<_> = per_package_reports.iter().collect();
    per_package_reports.sort_by_key(|r| r.package_id);
    let mut rendered = String::new();
    for per_package in &per_package_reports {
        rendered.push_str(&format!(
            "The package `{}` currently triggers the following future \
             incompatibility lints:\n",
            per_package.package_id
        ));
        for item in &per_package.items {
            rendered.extend(
                item.diagnostic
                    .rendered
                    .lines()
                    .map(|l| format!("> {}\n", l)),
            );
        }
        rendered.push('\n');
    }
    if let Some(s) = render_suggestions(ws, &per_package_reports) {
        rendered.push_str(&s);
    }
    rendered
}

fn render_suggestions(
    ws: &Workspace<'_>,
    per_package_reports: &[&FutureIncompatReportPackage],
) -> Option<String> {
    // This in general ignores all errors since this is opportunistic.
    let _lock = ws.config().acquire_package_cache_lock().ok()?;
    // Create a set of updated registry sources.
    let map = SourceConfigMap::new(ws.config()).ok()?;
    let package_ids: BTreeSet<_> = per_package_reports
        .iter()
        .map(|r| r.package_id)
        .filter(|pkg_id| pkg_id.source_id().is_registry())
        .collect();
    let source_ids: HashSet<_> = package_ids
        .iter()
        .map(|pkg_id| pkg_id.source_id())
        .collect();
    let mut sources: HashMap<_, _> = source_ids
        .into_iter()
        .filter_map(|sid| {
            let source = map.load(sid, &HashSet::new()).ok()?;
            Some((sid, source))
        })
        .collect();
    // Query the sources for new versions.
    let mut suggestions = String::new();
    for pkg_id in package_ids {
        let source = match sources.get_mut(&pkg_id.source_id()) {
            Some(s) => s,
            None => continue,
        };
        let dep = Dependency::parse(pkg_id.name(), None, pkg_id.source_id()).ok()?;
        let summaries = source.query_vec(&dep).ok()?;
        let versions = itertools::sorted(
            summaries
                .iter()
                .map(|summary| summary.version())
                .filter(|version| *version > pkg_id.version()),
        );
        let versions = versions.map(|version| version.to_string());
        let versions = iter_join(versions, ", ");
        if !versions.is_empty() {
            writeln!(
                suggestions,
                "{} has the following newer versions available: {}",
                pkg_id, versions
            )
            .unwrap();
        }
    }
    if suggestions.is_empty() {
        None
    } else {
        Some(format!(
            "The following packages appear to have newer versions available.\n\
             You may want to consider updating them to a newer version to see if the \
             issue has been fixed.\n\n{}",
            suggestions
        ))
    }
}
