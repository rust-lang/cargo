use command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("generate-lockfile")
        .about("Generate the lockfile for a project")
        .arg_manifest_path()
        .after_help(
            "\
If a lockfile is available, this command will ensure that all of the git
dependencies and/or registries dependencies are downloaded and locally
available. The network is never touched after a `cargo fetch` unless
the lockfile changes.

If the lockfile is not available, then this is the equivalent of
`cargo generate-lockfile`. A lockfile is generated and dependencies are also
all updated.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;
    ops::generate_lockfile(&ws)?;
    Ok(())
}
