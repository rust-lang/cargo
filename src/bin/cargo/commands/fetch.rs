use crate::command_prelude::*;

use cargo::ops;
use cargo::ops::FetchOptions;

pub fn cli() -> Command {
    subcommand("fetch")
        .about("Fetch dependencies of a package from the network")
        .arg_silent_suggestion()
        .arg_target_triple("Fetch dependencies for the target triple")
        .arg_manifest_path()
        .arg_lockfile_path()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help fetch</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(gctx)?;

    let opts = FetchOptions {
        gctx,
        targets: args.targets()?,
    };
    let _ = ops::fetch(&ws, &opts)?;
    Ok(())
}
