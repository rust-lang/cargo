use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("generate-lockfile")
        .about("Generate the lockfile for a package")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg_manifest_path()
        .after_help("Run `cargo help generate-lockfile` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;
    ops::generate_lockfile(&ws)?;
    Ok(())
}
