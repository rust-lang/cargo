//! The `cargo report sessions` command.

use annotate_snippets::Level;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Workspace;
use crate::ops::cargo_report::util::list_log_files;
use crate::util::BuildLogger;

pub struct ReportSessionsOptions {
    pub limit: usize,
}

pub fn report_sessions(
    gctx: &GlobalContext,
    ws: Option<&Workspace<'_>>,
    opts: ReportSessionsOptions,
) -> CargoResult<()> {
    let ws_run_id = ws.map(BuildLogger::generate_run_id);

    // Take limit + 1 to detect if there are more sessions
    let sessions: Vec<_> = list_log_files(gctx, None)?
        .filter(|(_, id)| {
            ws_run_id
                .as_ref()
                .map(|ws_run_id| ws_run_id.same_workspace(id))
                .unwrap_or(true)
        })
        .take(opts.limit + 1)
        .collect();

    if sessions.is_empty() {
        let context = if let Some(ws) = ws {
            format!(" for workspace at `{}`", ws.root().display())
        } else {
            String::new()
        };
        let title = format!("no sessions found{context}");
        let note = "run build commands with `-Z build-analysis` to generate log files";

        let report = [Level::ERROR
            .primary_title(title)
            .element(Level::NOTE.message(note))];
        gctx.shell().print_report(&report, false)?;
        return Err(crate::AlreadyPrintedError::new(anyhow::anyhow!("")).into());
    }

    let truncated = sessions.len() > opts.limit;
    let display_count = opts.limit.min(sessions.len());

    let mut shell = gctx.shell();
    let stderr = shell.err();

    if let Some(ws) = ws {
        writeln!(
            stderr,
            "Session IDs for `{}` (most recent first):",
            ws.root().display(),
        )?;
    } else {
        writeln!(stderr, "Session IDs (most recent first):",)?
    };
    writeln!(stderr)?;

    for (_path, run_id) in sessions.iter().take(display_count) {
        writeln!(stderr, " - {run_id}")?;
    }

    if truncated {
        writeln!(stderr)?;
        writeln!(stderr, "... and more (use --limit N to see more)")?;
    }

    Ok(())
}
