//! The `cargo report rebuilds` command.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use annotate_snippets::Group;
use annotate_snippets::Level;
use anyhow::Context as _;
use cargo_util_schemas::core::PackageIdSpec;
use itertools::Itertools as _;

use crate::AlreadyPrintedError;
use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Workspace;
use crate::core::compiler::CompileMode;
use crate::core::compiler::UnitIndex;
use crate::core::compiler::fingerprint::DirtyReason;
use crate::core::compiler::fingerprint::FsStatus;
use crate::core::compiler::fingerprint::StaleItem;
use crate::ops::cargo_report::util::list_log_files;
use crate::ops::cargo_report::util::unit_target_description;
use crate::util::log_message::FingerprintStatus;
use crate::util::log_message::LogMessage;
use crate::util::log_message::Target;
use crate::util::logger::RunId;
use crate::util::style;

const DEFAULT_DISPLAY_LIMIT: usize = 5;

pub struct ReportRebuildsOptions {}

pub fn report_rebuilds(
    gctx: &GlobalContext,
    ws: Option<&Workspace<'_>>,
    _opts: ReportRebuildsOptions,
) -> CargoResult<()> {
    let Some((log, run_id)) = list_log_files(gctx, ws)?.next() else {
        let context = if let Some(ws) = ws {
            format!(" for workspace at `{}`", ws.root().display())
        } else {
            String::new()
        };
        let title = format!("no sessions found{context}");
        let note = "run command with `-Z build-analysis` to generate log files";
        let report = [Level::ERROR
            .primary_title(title)
            .element(Level::NOTE.message(note))];
        gctx.shell().print_report(&report, false)?;
        return Err(AlreadyPrintedError::new(anyhow::anyhow!("")).into());
    };

    let ctx = prepare_context(&log)
        .with_context(|| format!("failed to analyze log at `{}`", log.display()))?;
    let ws_root = ws.map(|ws| ws.root()).unwrap_or(gctx.cwd());

    display_report(gctx, ctx, &run_id, ws_root)?;

    Ok(())
}

struct Context {
    root_rebuilds: Vec<RootRebuild>,
    units: HashMap<UnitIndex, UnitInfo>,
    total_cached: usize,
    total_new: usize,
    total_rebuilt: usize,
}

struct UnitInfo {
    package_id: PackageIdSpec,
    target: Target,
    mode: CompileMode,
}

struct RootRebuild {
    unit_index: UnitIndex,
    reason: DirtyReason,
    affected_units: Vec<UnitIndex>,
}

fn prepare_context(log: &Path) -> CargoResult<Context> {
    let reader = BufReader::new(File::open(log)?);

    let mut units: HashMap<UnitIndex, UnitInfo> = HashMap::new();
    let mut dependencies: HashMap<UnitIndex, Vec<UnitIndex>> = HashMap::new();
    let mut dirty_reasons: HashMap<UnitIndex, DirtyReason> = HashMap::new();
    let mut total_cached = 0;
    let mut total_new = 0;
    let mut total_rebuilt = 0;

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
            LogMessage::UnitRegistered {
                package_id,
                target,
                mode,
                index,
                dependencies: deps,
                ..
            } => {
                units.insert(
                    index,
                    UnitInfo {
                        package_id,
                        target,
                        mode,
                    },
                );
                dependencies.insert(index, deps);
            }
            LogMessage::UnitFingerprint {
                index,
                status,
                cause,
                ..
            } => {
                if let Some(reason) = cause {
                    dirty_reasons.insert(index, reason);
                }
                match status {
                    FingerprintStatus::Fresh => {
                        total_cached += 1;
                    }
                    FingerprintStatus::Dirty => {
                        total_rebuilt += 1;
                    }
                    FingerprintStatus::New => {
                        total_new += 1;
                        dirty_reasons.insert(index, DirtyReason::FreshBuild);
                    }
                }
            }
            _ => {}
        }
    }

    // reverse dependency graph (dependents of each unit)
    let mut reverse_deps: HashMap<UnitIndex, Vec<UnitIndex>> = HashMap::new();
    for (unit_id, deps) in &dependencies {
        for dep_id in deps {
            reverse_deps.entry(*dep_id).or_default().push(*unit_id);
        }
    }

    let rebuilt_units: HashSet<UnitIndex> = dirty_reasons.keys().copied().collect();

    // Root rebuilds: units that rebuilt but none of their dependencies rebuilt
    let root_rebuilds: Vec<_> = dirty_reasons
        .iter()
        .filter(|(unit_index, _)| {
            let has_rebuilt_deps = dependencies
                .get(unit_index)
                .map(|deps| deps.iter().any(|dep| rebuilt_units.contains(dep)))
                .unwrap_or_default();
            !has_rebuilt_deps
        })
        .map(|(&unit_index, reason)| {
            let affected_units = find_cascading_rebuilds(unit_index, &reverse_deps, &rebuilt_units);
            RootRebuild {
                unit_index,
                reason: reason.clone(),
                affected_units,
            }
        })
        .sorted_by(|a, b| {
            b.affected_units
                .len()
                .cmp(&a.affected_units.len())
                .then_with(|| {
                    let a_name = units.get(&a.unit_index).map(|u| u.package_id.name());
                    let b_name = units.get(&b.unit_index).map(|u| u.package_id.name());
                    a_name.cmp(&b_name)
                })
        })
        .collect();

    Ok(Context {
        root_rebuilds,
        units,
        total_cached,
        total_new,
        total_rebuilt,
    })
}

/// Finds all units that were rebuilt as a cascading effect of the given root rebuild.
fn find_cascading_rebuilds(
    root_rebuild: UnitIndex,
    dependents: &HashMap<UnitIndex, Vec<UnitIndex>>,
    rebuilt_units: &HashSet<UnitIndex>,
) -> Vec<UnitIndex> {
    let mut affected = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = vec![root_rebuild];
    visited.insert(root_rebuild);

    while let Some(unit) = queue.pop() {
        if let Some(deps) = dependents.get(&unit) {
            for &dep in deps {
                if !visited.contains(&dep) && rebuilt_units.contains(&dep) {
                    visited.insert(dep);
                    affected.push(dep);
                    queue.push(dep);
                }
            }
        }
    }

    affected.sort_unstable();
    affected
}

fn display_report(
    gctx: &GlobalContext,
    ctx: Context,
    run_id: &RunId,
    ws_root: &Path,
) -> CargoResult<()> {
    let verbose = gctx.shell().verbosity() == crate::core::shell::Verbosity::Verbose;
    let extra_verbose = gctx.extra_verbose();

    let Context {
        root_rebuilds,
        units,
        total_cached,
        total_new,
        total_rebuilt,
    } = ctx;

    let header = style::HEADER;
    let subheader = style::LITERAL;
    let mut shell = gctx.shell();
    let stderr = shell.err();

    writeln!(stderr, "{header}Session:{header:#} {run_id}")?;

    // Render summary
    let rebuilt_plural = plural(total_rebuilt);

    writeln!(
        stderr,
        "{header}Status:{header:#} {total_rebuilt} unit{rebuilt_plural} rebuilt, {total_cached} cached, {total_new} new"
    )?;
    writeln!(stderr)?;

    if total_rebuilt == 0 && total_new == 0 {
        // Don't show detailed report if all units are cached.
        return Ok(());
    }

    if total_rebuilt == 0 && total_cached == 0 {
        // Don't show detailed report if all units are new build.
        return Ok(());
    }

    // Render root rebuilds and cascading count
    let root_rebuild_count = root_rebuilds.len();
    let cascading_count: usize = root_rebuilds.iter().map(|r| r.affected_units.len()).sum();

    let root_plural = plural(root_rebuild_count);
    let cascading_plural = plural(cascading_count);

    writeln!(stderr, "{header}Rebuild impact:{header:#}",)?;
    writeln!(
        stderr,
        "  root rebuilds: {root_rebuild_count} unit{root_plural}"
    )?;
    writeln!(
        stderr,
        "  cascading:     {cascading_count} unit{cascading_plural}"
    )?;
    writeln!(stderr)?;

    // Render each root rebuilds
    let display_limit = if verbose {
        root_rebuilds.len()
    } else {
        DEFAULT_DISPLAY_LIMIT.min(root_rebuilds.len())
    };
    let truncated_count = root_rebuilds.len().saturating_sub(display_limit);

    if truncated_count > 0 {
        let count = root_rebuilds.len();
        writeln!(
            stderr,
            "{header}Root rebuilds:{header:#} (top {display_limit} of {count} by impact)",
        )?;
    } else {
        writeln!(stderr, "{header}Root rebuilds:{header:#}",)?;
    }

    for (idx, root_rebuild) in root_rebuilds.iter().take(display_limit).enumerate() {
        let unit_desc = units
            .get(&root_rebuild.unit_index)
            .map(unit_description)
            .expect("must have the unit");

        let reason_str = format_dirty_reason(&root_rebuild.reason, &units, ws_root);

        writeln!(
            stderr,
            "  {subheader}{idx}. {unit_desc}:{subheader:#} {reason_str}",
        )?;

        if root_rebuild.affected_units.is_empty() {
            writeln!(stderr, "     impact: no cascading rebuilds")?;
        } else {
            let count = root_rebuild.affected_units.len();
            let plural = plural(count);
            writeln!(
                stderr,
                "     impact: {count} dependent unit{plural} rebuilt"
            )?;

            if extra_verbose {
                for affected in &root_rebuild.affected_units {
                    if let Some(affected) = units.get(affected) {
                        let desc = unit_description(affected);
                        writeln!(stderr, "       - {desc}")?;
                    }
                }
            }
        }
    }

    // Render --verbose notes
    drop(shell);
    let has_cascading_rebuilds = root_rebuilds.iter().any(|rr| !rr.affected_units.is_empty());

    if !verbose && truncated_count > 0 {
        writeln!(gctx.shell().err())?;
        let note = "pass `--verbose` to show all root rebuilds";
        gctx.shell().print_report(
            &[Group::with_title(Level::NOTE.secondary_title(note))],
            false,
        )?;
    } else if !extra_verbose && has_cascading_rebuilds {
        writeln!(gctx.shell().err())?;
        let note = "pass `-vv` to show all affected rebuilt unit lists";
        gctx.shell().print_report(
            &[Group::with_title(Level::NOTE.secondary_title(note))],
            false,
        )?;
    }

    Ok(())
}

fn unit_description(unit: &UnitInfo) -> String {
    let name = unit.package_id.name();
    let version = unit
        .package_id
        .version()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "<n/a>".into());
    let target = unit_target_description(&unit.target, unit.mode);

    let literal = style::LITERAL;
    let nop = style::NOP;

    format!("{literal}{name}@{version}{literal:#}{nop}{target}{nop:#}")
}

fn plural(len: usize) -> &'static str {
    if len == 1 { "" } else { "s" }
}

fn format_dirty_reason(
    reason: &DirtyReason,
    units: &HashMap<UnitIndex, UnitInfo>,
    ws_root: &Path,
) -> String {
    match reason {
        DirtyReason::RustcChanged => "toolchain changed".to_string(),
        DirtyReason::FeaturesChanged { old, new } => {
            format!("activated features changed: {old} -> {new}")
        }
        DirtyReason::DeclaredFeaturesChanged { old, new } => {
            format!("declared features changed: {old} -> {new}")
        }
        DirtyReason::TargetConfigurationChanged => "target configuration changed".to_string(),
        DirtyReason::PathToSourceChanged => "path to source changed".to_string(),
        DirtyReason::ProfileConfigurationChanged => "profile configuration changed".to_string(),
        DirtyReason::RustflagsChanged { old, new } => {
            let old = old.join(", ");
            let new = new.join(", ");
            format!("rustflags changed: {old} -> {new}")
        }
        DirtyReason::ConfigSettingsChanged => "config settings changed".to_string(),
        DirtyReason::CompileKindChanged => "compile target changed".to_string(),
        DirtyReason::FsStatusOutdated(status) => match status {
            FsStatus::Stale => "filesystem status stale".to_string(),
            FsStatus::StaleItem(item) => match item {
                StaleItem::MissingFile { path } => {
                    let path = path.strip_prefix(ws_root).unwrap_or(path).display();
                    format!("file missing: {path}")
                }
                StaleItem::UnableToReadFile { path } => {
                    let path = path.strip_prefix(ws_root).unwrap_or(path).display();
                    format!("unable to read file: {path}")
                }
                StaleItem::FailedToReadMetadata { path } => {
                    let path = path.strip_prefix(ws_root).unwrap_or(path).display();
                    format!("failed to read file metadata: {path}")
                }
                StaleItem::FileSizeChanged {
                    path,
                    old_size: old,
                    new_size: new,
                } => {
                    let path = path.strip_prefix(ws_root).unwrap_or(path).display();
                    format!("file size changed: {path} ({old} -> {new} bytes)")
                }
                StaleItem::ChangedFile { stale, .. } => {
                    let path = stale.strip_prefix(ws_root).unwrap_or(stale).display();
                    format!("file modified: {path}")
                }
                StaleItem::ChangedChecksum {
                    source,
                    stored_checksum: old,
                    new_checksum: new,
                } => {
                    let path = source.strip_prefix(ws_root).unwrap_or(source).display();
                    format!("file checksum changed: {path} ({old} -> {new})")
                }
                StaleItem::MissingChecksum { path } => {
                    let path = path.strip_prefix(ws_root).unwrap_or(path).display();
                    format!("checksum missing: {path}")
                }
                StaleItem::ChangedEnv {
                    var,
                    previous,
                    current,
                } => {
                    let old = previous.as_deref().unwrap_or("<unset>");
                    let new = current.as_deref().unwrap_or("<unset>");
                    format!("environment variable changed ({var}): {old} -> {new}")
                }
            },
            FsStatus::StaleDepFingerprint { unit } => units
                .get(unit)
                .map(|u| format!("dependency rebuilt: {}", unit_description(u)))
                .unwrap_or_else(|| format!("dependency rebuilt: unit {unit}")),
            FsStatus::StaleDependency { unit, .. } => units
                .get(unit)
                .map(|u| format!("dependency rebuilt: {}", unit_description(u)))
                .unwrap_or_else(|| format!("dependency rebuilt: unit {unit}")),
            FsStatus::UpToDate { .. } => "up to date".to_string(),
        },
        DirtyReason::EnvVarChanged {
            name,
            old_value,
            new_value,
        } => {
            let old = old_value.as_deref().unwrap_or("<unset>");
            let new = new_value.as_deref().unwrap_or("<unset>");
            format!("environment variable changed ({name}): {old} -> {new}")
        }
        DirtyReason::EnvVarsChanged { old, new } => {
            format!("environment variables changed: {old} -> {new}")
        }
        DirtyReason::LocalFingerprintTypeChanged { old, new } => {
            format!("local fingerprint type changed: {old} -> {new}")
        }
        DirtyReason::NumberOfDependenciesChanged { old, new } => {
            format!("number of dependencies changed: {old} -> {new}")
        }
        DirtyReason::UnitDependencyNameChanged { old, new } => {
            format!("dependency name changed: {old} -> {new}")
        }
        DirtyReason::UnitDependencyInfoChanged { unit } => units
            .get(unit)
            .map(|u| format!("dependency info changed: {}", unit_description(u)))
            .unwrap_or_else(|| "dependency info changed".to_string()),
        DirtyReason::LocalLengthsChanged => "local lengths changed".to_string(),
        DirtyReason::PrecalculatedComponentsChanged { old, new } => {
            format!("precalculated components changed: {old} -> {new}")
        }
        DirtyReason::ChecksumUseChanged { old } => {
            if *old {
                "checksum use changed: enabled -> disabled".to_string()
            } else {
                "checksum use changed: disabled -> enabled".to_string()
            }
        }
        DirtyReason::DepInfoOutputChanged { old, new } => {
            let old = old.strip_prefix(ws_root).unwrap_or(old).display();
            let new = new.strip_prefix(ws_root).unwrap_or(new).display();
            format!("dependency info output changed: {old} -> {new}")
        }
        DirtyReason::RerunIfChangedOutputFileChanged { old, new } => {
            let old = old.strip_prefix(ws_root).unwrap_or(old).display();
            let new = new.strip_prefix(ws_root).unwrap_or(new).display();
            format!("rerun-if-changed output file changed: {old} -> {new}")
        }
        DirtyReason::RerunIfChangedOutputPathsChanged { old, new } => {
            let old = old.len();
            let new = new.len();
            format!("rerun-if-changed paths changed: {old} path(s) -> {new} path(s)",)
        }
        DirtyReason::NothingObvious => "nothing obvious".to_string(),
        DirtyReason::Forced => "forced rebuild".to_string(),
        DirtyReason::FreshBuild => "fresh build".to_string(),
    }
}
