//! For opening files or URLs with the preferred application.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::CargoResult;
use crate::GlobalContext;
use crate::util::context::PathAndArgs;

/// Opens a file path using the preferred application.
///
/// 1. Try `doc.browser` config first
/// 2. Then `$BROWSER`
/// 3. Finally system default opener
pub fn open(path: &Path, gctx: &GlobalContext) -> CargoResult<()> {
    let config_browser = {
        let cfg: Option<PathAndArgs> = gctx.get("doc.browser")?;
        cfg.map(|path_args| (path_args.path.resolve_program(gctx), path_args.args))
    };

    let mut shell = gctx.shell();
    let link = shell.err_file_hyperlink(&path);
    shell.status("Opening", format!("{link}{}{link:#}", path.display()))?;

    let browser =
        config_browser.or_else(|| Some((PathBuf::from(gctx.get_env_os("BROWSER")?), Vec::new())));

    match browser {
        Some((browser, initial_args)) => {
            if let Err(e) = Command::new(&browser).args(initial_args).arg(path).status() {
                shell.warn(format!(
                    "Couldn't open docs with {}: {}",
                    browser.to_string_lossy(),
                    e
                ))?;
            }
        }
        None => {
            if let Err(e) = opener::open(&path) {
                let e = e.into();
                crate::display_warning_with_error("couldn't open docs", &e, &mut shell);
            }
        }
    };

    Ok(())
}
