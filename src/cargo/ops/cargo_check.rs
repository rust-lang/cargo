use core::Workspace;
use util::important_paths::find_root_manifest_for_wd;
use util::{CliResult, Config};

pub fn with_check_ws<F>(flag_manifest_path: Option<String>,
                        config: &Config, f: F)
                        -> CliResult<Option<()>>
    where F: FnOnce(&Workspace) -> CliResult<Option<()>>
{
    let root = find_root_manifest_for_wd(flag_manifest_path, config.cwd())?;
    let ws = Workspace::new(&root, config)?;
    f(&ws)
}
