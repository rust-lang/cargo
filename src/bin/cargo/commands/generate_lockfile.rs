use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("generate-lockfile")
        .about("Generate the lockfile for a package")
        .arg_silent_suggestion()
        .arg_manifest_path()
        .arg_lockfile_path()
        .arg_ignore_rust_version_with_help("Ignore `rust-version` specification in packages")
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help generate-lockfile</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(gctx)?;
    ops::generate_lockfile(&ws)?;
    Ok(())
}
