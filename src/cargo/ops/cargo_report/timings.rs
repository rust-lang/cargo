//! The `cargo report timings` command.

use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
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
use crate::core::compiler::UnitIndex;
use crate::core::compiler::timings::CompilationSection;
use crate::core::compiler::timings::UnitData;
use crate::core::compiler::timings::report::RenderContext;
use crate::core::compiler::timings::report::aggregate_sections;
use crate::core::compiler::timings::report::compute_concurrency;
use crate::core::compiler::timings::report::round_to_centisecond;
use crate::core::compiler::timings::report::write_html;
use crate::ops::cargo_report::util::find_log_file;
use crate::ops::cargo_report::util::unit_target_description;
use crate::util::log_message::FingerprintStatus;
use crate::util::log_message::LogMessage;
use crate::util::log_message::Target;
use crate::util::logger::RunId;
use crate::util::style;

pub struct ReportTimingsOptions<'gctx> {
    /// Whether to attempt to open the browser after the report is generated
    pub open_result: bool,
    pub gctx: &'gctx GlobalContext,
    pub id: Option<RunId>,
}

/// Collects sections data for later post-processing through [`aggregate_sections`].
struct UnitEntry {
    target: Target,
    data: UnitData,
    sections: IndexMap<String, CompilationSection>,
    rmeta_time: Option<f64>,
}

pub fn report_timings(
    gctx: &GlobalContext,
    ws: Option<&Workspace<'_>>,
    opts: ReportTimingsOptions<'_>,
) -> CargoResult<()> {
    let Some((log, run_id)) = find_log_file(gctx, ws, opts.id.as_ref())? else {
        let context = if let Some(ws) = ws {
            format!(" for workspace at `{}`", ws.root().display())
        } else {
            String::new()
        };
        let (title, note) = if let Some(id) = &opts.id {
            (
                format!("session `{id}` not found{context}"),
                "run `cargo report sessions` to list available sessions",
            )
        } else {
            (
                format!("no sessions found{context}"),
                "run command with `-Z build-analysis` to generate log files",
            )
        };
        let report = [Level::ERROR
            .primary_title(title)
            .element(Level::NOTE.message(note))];
        gctx.shell().print_report(&report, false)?;
        return Err(AlreadyPrintedError::new(anyhow::anyhow!("")).into());
    };

    let reader = BufReader::new(File::open(&log)?);
    let iter = serde_json::Deserializer::from_reader(reader)
        .into_iter::<LogMessage>()
        .enumerate()
        .filter_map(|(idx, msg)| match msg {
            Ok(msg) => Some(msg),
            Err(e) => {
                tracing::warn!("failed to parse log message at index {idx}: {e}");
                None
            }
        });
    let ctx = prepare_context(iter, &run_id)
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

pub(crate) fn prepare_context<I>(log: I, run_id: &RunId) -> CargoResult<RenderContext<'_>>
where
    I: Iterator<Item = LogMessage>,
{
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

    let mut requested_units: HashSet<UnitIndex> = HashSet::new();

    for msg in log {
        match msg {
            LogMessage::BuildStarted {
                command: _,
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
                dependencies: _,
            } => {
                if requested {
                    requested_units.insert(index);
                }
                platform_targets.insert(platform);

                let version = package_id
                    .version()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "N/A".into());

                let target_str = unit_target_description(&target, mode);

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
