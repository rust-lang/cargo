//! The `cargo report rebuilds` command.

use annotate_snippets::Level;

use crate::AlreadyPrintedError;
use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Workspace;
use crate::ops::cargo_report::util::list_log_files;

pub struct ReportRebuildsOptions {}

pub fn report_rebuilds(
    gctx: &GlobalContext,
    ws: Option<&Workspace<'_>>,
    _opts: ReportRebuildsOptions,
) -> CargoResult<()> {
    let Some((_log, _run_id)) = list_log_files(gctx, ws)?.next() else {
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

    Ok(())
}
