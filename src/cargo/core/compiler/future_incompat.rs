//! Support for [future-incompatible warning reporting][1].
//!
//! Here is an overview of how Cargo handles future-incompatible reports.
//!
//! ## Receive reports from the compiler
//!
//! When receiving a compiler message during a build, if it is effectively
//! a [`FutureIncompatReport`], Cargo gathers and forwards it as a
//! `Message::FutureIncompatReport` to the main thread.
//!
//! To have the correct layout of strucutures for deserializing a report
//! emitted by the compiler, most of structure definitions, for example
//! [`FutureIncompatReport`], are copied either partially or entirely from
//! [compiler/rustc_errors/src/json.rs][2] in rust-lang/rust repository.
//!
//! ## Persist reports on disk
//!
//! When a build comes to an end, by calling [`save_and_display_report`]
//! Cargo saves the report on disk, and displays it directly if requested
//! via command line or configuration. The information of the on-disk file can
//! be found in [`FUTURE_INCOMPAT_FILE`].
//!
//! During the persistent process, Cargo will attempt to query the source of
//! each package emitting the report, for the sake of providing an upgrade
//! information as a solution to fix the incompatibility.
//!
//! ## Display reports to users
//!
//! Users can run `cargo report future-incompat` to retrieve a report. This is
//! done by [`OnDiskReports::load`]. Cargo simply prints reports to the
//! standard output.
//!
//! [1]: https://doc.rust-lang.org/nightly/cargo/reference/future-incompat-report.html
//! [2]: https://github.com/rust-lang/rust/blob/9bb6e60d1f1360234aae90c97964c0fa5524f141/compiler/rustc_errors/src/json.rs#L312-L315

use crate::core::compiler::BuildContext;
use crate::core::{Dependency, PackageId, Workspace};
use crate::sources::source::QueryKind;
use crate::sources::SourceConfigMap;
use crate::util::cache_lock::CacheLockMode;
use crate::util::{iter_join, CargoResult};
use anyhow::{bail, format_err, Context};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::task::Poll;

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
    /// A message describing suggestions for fixing the
    /// reported issues
    suggestion_message: String,
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
    /// Saves a new report returning its id
    pub fn save_report(
        mut self,
        ws: &Workspace<'_>,
        suggestion_message: String,
        per_package_reports: &[FutureIncompatReportPackage],
    ) -> u32 {
        let per_package = render_report(per_package_reports);

        if let Some(existing_report) = self
            .reports
            .iter()
            .find(|existing| existing.per_package == per_package)
        {
            return existing_report.id;
        }

        let report = OnDiskReport {
            id: self.next_id,
            suggestion_message,
            per_package,
        };

        let saved_id = report.id;
        self.next_id += 1;
        self.reports.push(report);
        if self.reports.len() > MAX_REPORTS {
            self.reports.remove(0);
        }
        let on_disk = serde_json::to_vec(&self).unwrap();
        if let Err(e) = ws
            .target_dir()
            .open_rw_exclusive_create(
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

        saved_id
    }

    /// Loads the on-disk reports.
    pub fn load(ws: &Workspace<'_>) -> CargoResult<OnDiskReports> {
        let report_file = match ws.target_dir().open_ro_shared(
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

    /// Returns an ANSI-styled report
    pub fn get_report(&self, id: u32, package: Option<&str>) -> CargoResult<String> {
        let report = self.reports.iter().find(|r| r.id == id).ok_or_else(|| {
            let available = iter_join(self.reports.iter().map(|r| r.id.to_string()), ", ");
            format_err!(
                "could not find report with ID {}\n\
                 Available IDs are: {}",
                id,
                available
            )
        })?;

        let mut to_display = report.suggestion_message.clone();
        to_display += "\n";

        let package_report = if let Some(package) = package {
            report
                .per_package
                .get(package)
                .ok_or_else(|| {
                    format_err!(
                        "could not find package with ID `{}`\n
                Available packages are: {}\n
                Omit the `--package` flag to display a report for all packages",
                        package,
                        iter_join(report.per_package.keys(), ", ")
                    )
                })?
                .to_string()
        } else {
            report
                .per_package
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        };
        to_display += &package_report;

        Ok(to_display)
    }
}

fn render_report(per_package_reports: &[FutureIncompatReportPackage]) -> BTreeMap<String, String> {
    let mut report: BTreeMap<String, String> = BTreeMap::new();
    for per_package in per_package_reports {
        let package_spec = format!(
            "{}@{}",
            per_package.package_id.name(),
            per_package.package_id.version()
        );
        let rendered = report.entry(package_spec).or_default();
        rendered.push_str(&format!(
            "The package `{}` currently triggers the following future incompatibility lints:\n",
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

/// Returns a user-readable message explaining which of
/// the packages in `package_ids` have updates available.
/// This is best-effort - if an error occurs, `None` will be returned.
fn get_updates(ws: &Workspace<'_>, package_ids: &BTreeSet<PackageId>) -> Option<String> {
    // This in general ignores all errors since this is opportunistic.
    let _lock = ws
        .config()
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)
        .ok()?;
    // Create a set of updated registry sources.
    let map = SourceConfigMap::new(ws.config()).ok()?;
    let mut package_ids: BTreeSet<_> = package_ids
        .iter()
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

    // Query the sources for new versions, mapping `package_ids` into `summaries`.
    let mut summaries = Vec::new();
    while !package_ids.is_empty() {
        package_ids.retain(|&pkg_id| {
            let Some(source) = sources.get_mut(&pkg_id.source_id()) else {
                return false;
            };
            let Ok(dep) = Dependency::parse(pkg_id.name(), None, pkg_id.source_id()) else {
                return false;
            };
            match source.query_vec(&dep, QueryKind::Exact) {
                Poll::Ready(Ok(sum)) => {
                    summaries.push((pkg_id, sum));
                    false
                }
                Poll::Ready(Err(_)) => false,
                Poll::Pending => true,
            }
        });
        for (_, source) in sources.iter_mut() {
            source.block_until_ready().ok()?;
        }
    }

    let mut updates = String::new();
    for (pkg_id, summaries) in summaries {
        let mut updated_versions: Vec<_> = summaries
            .iter()
            .map(|summary| summary.version())
            .filter(|version| *version > pkg_id.version())
            .collect();
        updated_versions.sort();

        let updated_versions = iter_join(
            updated_versions
                .into_iter()
                .map(|version| version.to_string()),
            ", ",
        );

        if !updated_versions.is_empty() {
            writeln!(
                updates,
                "{} has the following newer versions available: {}",
                pkg_id, updated_versions
            )
            .unwrap();
        }
    }
    Some(updates)
}

/// Writes a future-incompat report to disk, using the per-package
/// reports gathered during the build. If requested by the user,
/// a message is also displayed in the build output.
pub fn save_and_display_report(
    bcx: &BuildContext<'_, '_>,
    per_package_future_incompat_reports: &[FutureIncompatReportPackage],
) {
    let should_display_message = match bcx.config.future_incompat_config() {
        Ok(config) => config.should_display_message(),
        Err(e) => {
            crate::display_warning_with_error(
                "failed to read future-incompat config from disk",
                &e,
                &mut bcx.config.shell(),
            );
            true
        }
    };

    if per_package_future_incompat_reports.is_empty() {
        // Explicitly passing a command-line flag overrides
        // `should_display_message` from the config file
        if bcx.build_config.future_incompat_report {
            drop(
                bcx.config
                    .shell()
                    .note("0 dependencies had future-incompatible warnings"),
            );
        }
        return;
    }

    let current_reports = match OnDiskReports::load(bcx.ws) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(
                "saving future-incompatible reports failed to load current reports: {:?}",
                e
            );
            OnDiskReports::default()
        }
    };
    let report_id = current_reports.next_id;

    // Get a list of unique and sorted package name/versions.
    let package_ids: BTreeSet<_> = per_package_future_incompat_reports
        .iter()
        .map(|r| r.package_id)
        .collect();
    let package_vers: Vec<_> = package_ids.iter().map(|pid| pid.to_string()).collect();

    if should_display_message || bcx.build_config.future_incompat_report {
        drop(bcx.config.shell().warn(&format!(
            "the following packages contain code that will be rejected by a future \
             version of Rust: {}",
            package_vers.join(", ")
        )));
    }

    let updated_versions = get_updates(bcx.ws, &package_ids).unwrap_or(String::new());

    let update_message = if !updated_versions.is_empty() {
        format!(
            "
- Some affected dependencies have newer versions available.
You may want to consider updating them to a newer version to see if the issue has been fixed.

{updated_versions}\n",
            updated_versions = updated_versions
        )
    } else {
        String::new()
    };

    let upstream_info = package_ids
        .iter()
        .map(|package_id| {
            let manifest = bcx.packages.get_one(*package_id).unwrap().manifest();
            format!(
                "
  - {package_spec}
  - Repository: {url}
  - Detailed warning command: `cargo report future-incompatibilities --id {id} --package {package_spec}`",
                package_spec = format!("{}@{}", package_id.name(), package_id.version()),
                url = manifest
                    .metadata()
                    .repository
                    .as_deref()
                    .unwrap_or("<not found>"),
                id = report_id,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let suggestion_message = format!(
        "
To solve this problem, you can try the following approaches:

{update_message}
- If the issue is not solved by updating the dependencies, a fix has to be
implemented by those dependencies. You can help with that by notifying the
maintainers of this problem (e.g. by creating a bug report) or by proposing a
fix to the maintainers (e.g. by creating a pull request):
{upstream_info}

- If waiting for an upstream fix is not an option, you can use the `[patch]`
section in `Cargo.toml` to use your own version of the dependency. For more
information, see:
https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section
        ",
        upstream_info = upstream_info,
        update_message = update_message,
    );

    let saved_report_id = current_reports.save_report(
        bcx.ws,
        suggestion_message.clone(),
        per_package_future_incompat_reports,
    );

    if bcx.build_config.future_incompat_report {
        drop(bcx.config.shell().note(&suggestion_message));
        drop(bcx.config.shell().note(&format!(
            "this report can be shown with `cargo report \
             future-incompatibilities --id {}`",
            saved_report_id
        )));
    } else if should_display_message {
        drop(bcx.config.shell().note(&format!(
            "to see what the problems were, use the option \
             `--future-incompat-report`, or run `cargo report \
             future-incompatibilities --id {}`",
            saved_report_id
        )));
    }
}
