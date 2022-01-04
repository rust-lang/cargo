use crate::command_prelude::*;
use cargo::ops::{self, OutputMetadataOptions};

pub fn cli() -> App {
    subcommand("metadata")
        .about(
            "Output the resolved dependencies of a package, \
             the concrete used versions including overrides, \
             in machine-readable format",
        )
        .arg_quiet()
        .arg_features()
        .arg(multi_opt(
            "filter-platform",
            "TRIPLE",
            "Only include resolve dependencies matching the given target-triple",
        ))
        .arg(opt(
            "no-deps",
            "Output information only about the workspace members \
             and don't fetch dependencies",
        ))
        .arg_manifest_path()
        .arg(
            opt("format-version", "Format version")
                .value_name("VERSION")
                .possible_value("1"),
        )
        .after_help("Run `cargo help metadata` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;

    let version = match args.value_of("format-version") {
        None => {
            config.shell().warn(
                "please specify `--format-version` flag explicitly \
                 to avoid compatibility problems",
            )?;
            1
        }
        Some(version) => version.parse().unwrap(),
    };

    let options = OutputMetadataOptions {
        cli_features: args.cli_features()?,
        no_deps: args.is_present("no-deps"),
        filter_platforms: args._values_of("filter-platform"),
        version,
    };

    let result = ops::output_metadata(&ws, &options)?;
    config.shell().print_json(&result)?;
    Ok(())
}
