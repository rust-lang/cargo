//! Common utilities for `cargo report` commands.

use std::ffi::OsStr;
use std::path::PathBuf;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Workspace;
use crate::util::BuildLogger;
use crate::util::logger::RunId;

/// Lists log files by calling a callback for each valid log file.
///
/// * Yield log files from new to old
/// * If in a workspace, select only the log files associated with the workspace
pub fn list_log_files(
    gctx: &GlobalContext,
    ws: Option<&Workspace<'_>>,
) -> CargoResult<Box<dyn Iterator<Item = (PathBuf, RunId)>>> {
    let log_dir = gctx.home().join("log");
    let log_dir = log_dir.as_path_unlocked();

    if !log_dir.exists() {
        return Ok(Box::new(std::iter::empty()));
    }

    let walk = walkdir::WalkDir::new(log_dir)
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
            Some((path.to_path_buf(), run_id))
        });

    let ws_run_id = ws.map(BuildLogger::generate_run_id);

    let walk = walk.filter(move |(_, id)| {
        ws_run_id
            .as_ref()
            .map_or(true, |ws_id| id.same_workspace(ws_id))
    });

    Ok(Box::new(walk))
}
