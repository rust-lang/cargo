//! Support for future-incompatible warning reporting.

use crate::core::{PackageId, Workspace};
use crate::util::{iter_join, CargoResult, Config};
use anyhow::{bail, format_err, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// Maps package names to the corresponding report
    /// We use a `BTreeMap` so that the iteration order
    /// is stable across multiple runs of `cargo`
    per_package: BTreeMap<String, String>,
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
            per_package: render_report(per_package_reports),
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

    pub fn get_report(
        &self,
        id: u32,
        config: &Config,
        package: Option<&str>,
    ) -> CargoResult<String> {
        let report = self.reports.iter().find(|r| r.id == id).ok_or_else(|| {
            let available = iter_join(self.reports.iter().map(|r| r.id.to_string()), ", ");
            format_err!(
                "could not find report with ID {}\n\
                 Available IDs are: {}",
                id,
                available
            )
        })?;
        let to_display = if let Some(package) = package {
            report
                .per_package
                .get(package)
                .ok_or_else(|| {
                    format_err!(
                        "could not find package with ID `{}`\n
                Available packages are: {}\n
                Omit the `--crate` flag to display a report for all crates",
                        package,
                        iter_join(report.per_package.keys(), ", ")
                    )
                })?
                .clone()
        } else {
            report
                .per_package
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        };
        let to_display = if config.shell().err_supports_color() {
            to_display
        } else {
            strip_ansi_escapes::strip(&to_display)
                .map(|v| String::from_utf8(v).expect("utf8"))
                .expect("strip should never fail")
        };
        Ok(to_display)
    }
}

fn render_report(per_package_reports: &[FutureIncompatReportPackage]) -> BTreeMap<String, String> {
    let mut report: BTreeMap<String, String> = BTreeMap::new();
    for per_package in per_package_reports {
        let package_spec = format!(
            "{}:{}",
            per_package.package_id.name(),
            per_package.package_id.version()
        );
        let rendered = report.entry(package_spec).or_default();
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
    }
    report
}
