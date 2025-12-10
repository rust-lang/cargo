//! The `cargo report timings` command.

use std::ffi::OsStr;
use std::fs::File;
use std::path::PathBuf;

use annotate_snippets::Level;
use anyhow::Context as _;

use crate::AlreadyPrintedError;
use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Workspace;
use crate::util::BuildLogger;
use crate::util::important_paths::find_root_manifest_for_wd;
use crate::util::logger::RunId;

pub struct ReportTimingsOptions<'gctx> {
    /// Whether to attempt to open the browser after the report is generated
    pub open_result: bool,
    pub gctx: &'gctx GlobalContext,
}

pub fn report_timings(gctx: &GlobalContext, _opts: ReportTimingsOptions<'_>) -> CargoResult<()> {
    let ws = find_root_manifest_for_wd(gctx.cwd())
        .ok()
        .and_then(|manifest_path| Workspace::new(&manifest_path, gctx).ok());
    let Some((log, _run_id)) = select_log_file(gctx, ws.as_ref())? else {
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

    let _f = File::open(&log)
        .with_context(|| format!("failed to analyze log at `{}`", log.display()))?;

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
