use command_prelude::*;

use cargo::core::{Source, SourceId};
use cargo::sources::RegistrySource;

pub fn cli() -> App {
    subcommand("update-registry")
        .about("Update the local copy of the registry from the network")
        .after_help(
            "\
Most Cargo commands will update the registry automatically when needed, so you
should not need to invoke this command directly. This command exists primarily
for use by scripts or third-party tools integrating with Cargo.
"
        )
}

pub fn exec(config: &mut Config, _args: &ArgMatches) -> CliResult {
    let crates_io = SourceId::crates_io(&config)?;
    RegistrySource::remote(&crates_io, &config).update()?;
    Ok(())
}
