//! The `cargo report timings` command.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

use annotate_snippets::Level;
use anyhow::Context as _;
use cargo_util::paths;
use indexmap::IndexMap;
use indexmap::map::Entry;
use itertools::Itertools as _;
use tempfile::TempDir;

use crate::AlreadyPrintedError;
use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Workspace;
use crate::core::compiler::CompileMode;
use crate::core::compiler::timings::CompilationSection;
use crate::core::compiler::timings::UnitData;
use crate::core::compiler::timings::report::RenderContext;
use crate::core::compiler::timings::report::aggregate_sections;
use crate::core::compiler::timings::report::compute_concurrency;
use crate::core::compiler::timings::report::round_to_centisecond;
use crate::core::compiler::timings::report::write_html;
use crate::util::BuildLogger;
use crate::util::important_paths::find_root_manifest_for_wd;
use crate::util::log_message::FingerprintStatus;
use crate::util::log_message::LogMessage;
use crate::util::log_message::Target;
use crate::util::logger::RunId;
use crate::util::style;

pub struct ReportTimingsOptions<'gctx> {
    /// Whether to attempt to open the browser after the report is generated
    pub open_result: bool,
    pub gctx: &'gctx GlobalContext,
}

/// Collects sections data for later post-processing through [`aggregate_sections`].
struct UnitEntry {
    target: Target,
    data: UnitData,
    sections: IndexMap<String, CompilationSection>,
    rmeta_time: Option<f64>,
}

pub fn report_timings(gctx: &GlobalContext, opts: ReportTimingsOptions<'_>) -> CargoResult<()> {
    let ws = find_root_manifest_for_wd(gctx.cwd())
        .ok()
        .and_then(|manifest_path| Workspace::new(&manifest_path, gctx).ok());
    let Some((log, run_id)) = select_log_file(gctx, ws.as_ref())? else {
        let title_extra = if let Some(ws) = ws {
            format!(" for workspace at `{}`", ws.root().display())
        } else {
            String::new()
        };
        let title = format!("no build log files found{title_extra}");
        let note = "run command with `-Z build-analysis` to generate log files";
        let report = [Level::ERROR
            .primary_title(title)
            .element(Level::NOTE.message(note))];
        gctx.shell().print_report(&report, false)?;
        return Err(AlreadyPrintedError::new(anyhow::anyhow!("")).into());
    };

    let ctx = prepare_context(&log, &run_id)
        .with_context(|| format!("failed to analyze log at `{}`", log.display()))?;

    // If we are in a workspace,
    // put timing reports in <target-dir>/cargo-timings/` as usual for easy access.
    // Otherwise in a temporary directory
    let reports_dir = if let Some(ws) = ws {
        let target_dir = ws.target_dir();
        let target_dir = target_dir.as_path_unlocked();
        paths::create_dir_all_excluded_from_backups_atomic(target_dir)?;
        let timings_dir = target_dir.join("cargo-timings");
        paths::create_dir_all(&timings_dir)?;
        timings_dir
    } else if let Ok(path) = gctx.get_env("__CARGO_TEST_REPORT_TIMINGS_TEMPDIR") {
        PathBuf::from(path.to_owned())
    } else {
        TempDir::with_prefix("cargo-timings-")?.keep()
    };

    let timing_path = reports_dir.join(format!("cargo-timing-{run_id}.html"));

    let mut out_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&timing_path)
        .with_context(|| format!("failed to open `{}`", timing_path.display()))?;

    write_html(ctx, &mut out_file)?;

    let link = gctx.shell().err_file_hyperlink(&timing_path);
    let msg = format!("report saved to {link}{}{link:#}", timing_path.display());
    gctx.shell()
        .status_with_color("Timing", msg, &style::NOTE)?;

    if opts.open_result {
        crate::util::open::open(&timing_path, gctx)?;
    }

    Ok(())
}

/// Selects the appropriate log file.
///
/// Currently look at the newest log file for the workspace.
/// If not in a workspace, pick the newest log file in the log directory.
fn select_log_file(
    gctx: &GlobalContext,
    ws: Option<&Workspace<'_>>,
) -> CargoResult<Option<(PathBuf, RunId)>> {
    let log_dir = gctx.home().join("log");
    let log_dir = log_dir.as_path_unlocked();

    if !log_dir.exists() {
        return Ok(None);
    }

    // Gets the latest log files in the log directory
    let mut walk = walkdir::WalkDir::new(log_dir)
        .follow_links(true)
        .sort_by(|a, b| a.file_name().cmp(b.file_name()).reverse())
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            // We only accept JSONL/NDJSON files.
            if !entry.file_type().is_file() {
                return None;
            }
            if entry.path().extension() != Some(OsStr::new("jsonl")) {
                return None;
            }

            // ...and the file name must follow RunId format
            let run_id = path.file_stem()?.to_str()?.parse::<RunId>().ok()?;
            Some((entry, run_id))
        });

    let item = if let Some(ws) = ws {
        // If we are under a workspace, find only that workspace's log files.
        let ws_run_id = BuildLogger::generate_run_id(ws);
        walk.skip_while(|(_, run_id)| !run_id.same_workspace(&ws_run_id))
            .next()
    } else {
        walk.next()
    };

    Ok(item.map(|(entry, run_id)| (entry.into_path(), run_id)))
}

fn prepare_context(log: &Path, run_id: &RunId) -> CargoResult<RenderContext<'static>> {
    let reader = BufReader::new(File::open(&log)?);

    let mut ctx = RenderContext {
        start_str: run_id.timestamp().to_string(),
        root_units: Default::default(),
        profile: Default::default(),
        total_fresh: Default::default(),
        total_dirty: Default::default(),
        unit_data: Default::default(),
        concurrency: Default::default(),
        cpu_usage: Default::default(),
        rustc_version: Default::default(),
        host: Default::default(),
        requested_targets: Default::default(),
        jobs: 0,
        num_cpus: None,
        error: &None,
    };
    let mut units: IndexMap<_, UnitEntry> = IndexMap::new();

    let mut platform_targets = HashSet::new();

    let mut requested_units = HashSet::new();

    for (log_index, result) in serde_json::Deserializer::from_reader(reader)
        .into_iter::<LogMessage>()
        .enumerate()
    {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                tracing::warn!("failed to parse log message at index {log_index}: {e}");
                continue;
            }
        };

        match msg {
            LogMessage::BuildStarted {
                cwd: _,
                host,
                jobs,
                num_cpus,
                profile,
                rustc_version,
                rustc_version_verbose,
                target_dir: _,
                workspace_root: _,
            } => {
                let rustc_version = rustc_version_verbose
                    .lines()
                    .next()
                    .map(ToOwned::to_owned)
                    .unwrap_or(rustc_version);
                ctx.host = host;
                ctx.jobs = jobs;
                ctx.num_cpus = num_cpus;
                ctx.profile = profile;
                ctx.rustc_version = rustc_version;
            }
            LogMessage::UnitRegistered {
                package_id,
                target,
                mode,
                platform,
                index,
                features,
                requested,
            } => {
                if requested {
                    requested_units.insert(index);
                }
                platform_targets.insert(platform);

                let version = package_id
                    .version()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "N/A".into());

                // This is pretty similar to how the current `core::compiler::timings`
                // renders `core::manifest::Target`. However, our target is
                // a simplified type so we cannot reuse the same logic here.
                let mut target_str = if target.kind == "lib" && mode == CompileMode::Build {
                    // Special case for brevity, since most dependencies hit this path.
                    "".to_string()
                } else if target.kind == "build-script" {
                    " build-script".to_string()
                } else {
                    format!(r#" {} "{}""#, target.name, target.kind)
                };

                match mode {
                    CompileMode::Test => target_str.push_str(" (test)"),
                    CompileMode::Build => {}
                    CompileMode::Check { test: true } => target_str.push_str(" (check-test)"),
                    CompileMode::Check { test: false } => target_str.push_str(" (check)"),
                    CompileMode::Doc { .. } => target_str.push_str(" (doc)"),
                    CompileMode::Doctest => target_str.push_str(" (doc test)"),
                    CompileMode::Docscrape => target_str.push_str(" (doc scrape)"),
                    CompileMode::RunCustomBuild => target_str.push_str(" (run)"),
                }

                let mode_str = if mode.is_run_custom_build() {
                    "run-custom-build"
                } else {
                    "todo"
                };

                let data = UnitData {
                    i: index,
                    name: package_id.name().to_string(),
                    version,
                    mode: mode_str.to_owned(),
                    target: target_str,
                    features,
                    start: 0.0,
                    duration: 0.0,
                    unblocked_units: Vec::new(),
                    unblocked_rmeta_units: Vec::new(),
                    sections: None,
                };

                units.insert(
                    index,
                    UnitEntry {
                        target,
                        data,
                        sections: IndexMap::new(),
                        rmeta_time: None,
                    },
                );
            }
            LogMessage::UnitFingerprint { status, .. } => match status {
                FingerprintStatus::New => ctx.total_dirty += 1,
                FingerprintStatus::Dirty => ctx.total_dirty += 1,
                FingerprintStatus::Fresh => ctx.total_fresh += 1,
            },
            LogMessage::UnitStarted { index, elapsed } => {
                units
                    .entry(index)
                    .and_modify(|unit| unit.data.start = elapsed)
                    .or_insert_with(|| {
                        unreachable!("unit {index} must have been registered first")
                    });
            }
            LogMessage::UnitRmetaFinished {
                index,
                elapsed,
                unblocked,
            } => match units.entry(index) {
                Entry::Occupied(mut e) => {
                    let elapsed = f64::max(elapsed - e.get().data.start, 0.0);
                    e.get_mut().data.unblocked_rmeta_units = unblocked;
                    e.get_mut().data.duration = elapsed;
                    e.get_mut().rmeta_time = Some(elapsed);
                }
                Entry::Vacant(_) => {
                    tracing::warn!(
                        "section `frontend` ended, but unit {index} has no start recorded"
                    )
                }
            },
            LogMessage::UnitSectionStarted {
                index,
                elapsed,
                section,
            } => match units.entry(index) {
                Entry::Occupied(mut e) => {
                    let elapsed = f64::max(elapsed - e.get().data.start, 0.0);
                    if e.get_mut()
                        .sections
                        .insert(
                            section.clone(),
                            CompilationSection {
                                start: elapsed,
                                end: None,
                            },
                        )
                        .is_some()
                    {
                        tracing::warn!(
                            "section `{section}` for unit {index} started more than once",
                        );
                    }
                }
                Entry::Vacant(_) => {
                    tracing::warn!(
                        "section `{section}` started, but unit {index} has no start recorded"
                    )
                }
            },
            LogMessage::UnitSectionFinished {
                index,
                elapsed,
                section,
            } => match units.entry(index) {
                Entry::Occupied(mut e) => {
                    let elapsed = f64::max(elapsed - e.get().data.start, 0.0);
                    if let Some(section) = e.get_mut().sections.get_mut(&section) {
                        section.end = Some(elapsed);
                    } else {
                        tracing::warn!(
                            "section `{section}` for unit {index} ended, but section `{section}` has no start recorded"
                        );
                    }
                }
                Entry::Vacant(_) => {
                    tracing::warn!(
                        "section `{section}` ended, but unit {index} has no start recorded"
                    )
                }
            },
            LogMessage::UnitFinished {
                index,
                elapsed,
                unblocked,
            } => match units.entry(index) {
                Entry::Occupied(mut e) => {
                    let elapsed = f64::max(elapsed - e.get().data.start, 0.0);
                    e.get_mut().data.duration = elapsed;
                    e.get_mut().data.unblocked_units = unblocked;
                }
                Entry::Vacant(_) => {
                    tracing::warn!("unit {index} ended, but it has no start recorded");
                }
            },
            _ => {} // skip non-timing logs
        }
    }

    ctx.root_units = {
        let mut root_map: IndexMap<_, Vec<_>> = IndexMap::new();
        for index in requested_units {
            let unit = &units[&index];
            // Pretty much like `core::Target::description_named`
            let target_desc = if unit.target.kind == "lib" {
                "lib".to_owned()
            } else if unit.target.kind == "build-script" {
                "build script".to_owned()
            } else {
                format!(r#" {} "{}""#, unit.target.name, unit.target.kind)
            };
            root_map.entry(index).or_default().push(target_desc);
        }
        root_map
            .into_iter()
            .sorted_by_key(|(i, _)| *i)
            .map(|(index, targets)| {
                let unit = &units[&index];
                let pkg_desc = format!("{} {}", unit.data.name, unit.data.version);
                (pkg_desc, targets)
            })
            .collect()
    };

    let unit_data: Vec<_> = units
        .into_values()
        .map(
            |UnitEntry {
                 target: _,
                 mut data,
                 sections,
                 rmeta_time,
             }| {
                // Post-processing for compilation sections we've collected so far.
                data.sections = aggregate_sections(sections, data.duration, rmeta_time);
                data.start = round_to_centisecond(data.start);
                data.duration = round_to_centisecond(data.duration);
                data
            },
        )
        .sorted_unstable_by(|a, b| a.start.partial_cmp(&b.start).unwrap())
        .collect();

    if unit_data.is_empty() {
        anyhow::bail!("no timing data found in log");
    }

    ctx.unit_data = unit_data;
    ctx.concurrency = compute_concurrency(&ctx.unit_data);
    ctx.requested_targets = platform_targets.into_iter().sorted_unstable().collect();

    Ok(ctx)
}
