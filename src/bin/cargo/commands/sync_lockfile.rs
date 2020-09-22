use crate::command_prelude::*;

use cargo::ops;
use cargo::ops::SyncLockfileOptions;

pub fn cli() -> App {
    subcommand("sync-lockfile")
        .about("Synchronize the lockfile to the package")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
        .arg_target_triple("Sync the lockfile for the target triple")
        .after_help("Run `cargo help sync-lockfile` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;

    let opts = SyncLockfileOptions {
        config,
        targets: args.targets(),
    };
    let _ = ops::sync_lockfile(&ws, &opts)?;
    Ok(())
}
