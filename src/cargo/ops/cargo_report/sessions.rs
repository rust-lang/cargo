//! The `cargo report sessions` command.

use annotate_snippets::Level;

use crate::CargoResult;
use crate::GlobalContext;
use crate::ops::cargo_report::util::infer_workspace;
use crate::ops::cargo_report::util::list_log_files;
use crate::util::BuildLogger;

pub struct ReportSessionsOptions {
    pub limit: usize,
}

pub fn report_sessions(gctx: &GlobalContext, opts: ReportSessionsOptions) -> CargoResult<()> {
    let ws = infer_workspace(gctx);

    let ws_run_id = ws.as_ref().map(BuildLogger::generate_run_id);

    let mut sessions = list_log_files(gctx, None)?
        .filter(|(_, id)| {
            ws_run_id
                .as_ref()
                .map(|ws_run_id| ws_run_id.same_workspace(id))
                .unwrap_or(true)
        })
        .take(opts.limit)
        .peekable();

    if sessions.peek().is_none() {
        let context = if let Some(ws) = ws {
            format!(" for workspace at `{}`", ws.root().display())
        } else {
            String::new()
        };
        let title = format!("no build sessions found{context}");
        let note = "run build commands with `-Z build-analysis` to generate log files";

        let report = [Level::ERROR
            .primary_title(title)
            .element(Level::NOTE.message(note))];
        gctx.shell().print_report(&report, false)?;
        return Err(crate::AlreadyPrintedError::new(anyhow::anyhow!("")).into());
    }

    let mut shell = gctx.shell();
    let stderr = shell.err();

    if let Some(ws) = ws {
        writeln!(
            stderr,
            "Session IDs for `{}` (showing up to {}):",
            ws.root().display(),
            opts.limit,
        )?;
    } else {
        writeln!(stderr, "Session IDs (showing up to {}):", opts.limit)?
    };
    writeln!(stderr)?;

    for (_path, run_id) in sessions {
        writeln!(stderr, " - {run_id}")?;
    }

    Ok(())
}
