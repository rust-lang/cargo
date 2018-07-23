use command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("generate-lockfile")
        .about("Generate the lockfile for a project")
        .arg_manifest_path()
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    ops::generate_lockfile(&ws)?;
    Ok(())
}
