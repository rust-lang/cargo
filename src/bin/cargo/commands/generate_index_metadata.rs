use cargo::ops;
use cargo::print_json;
use crate::command_prelude::*;

pub fn cli() -> App {
    subcommand("generate-index-metadata")
        .about(
            "Output index file metadata, \
             required by registry",
        )
        .arg_manifest_path()
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;

    let result = ops::generate_index_metadata(&ws)?;

    print_json(&result);

    Ok(())
}
