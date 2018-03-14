use command_prelude::*;

use cargo::ops::{self, OutputMetadataOptions};
use cargo::print_json;

pub fn cli() -> App {
    subcommand("metadata")
        .about(
            "Output the resolved dependencies of a project, \
             the concrete used versions including overrides, \
             in machine-readable format",
        )
        .arg_features()
        .arg(opt(
            "no-deps",
            "Output information only about the root package \
             and don't fetch dependencies",
        ))
        .arg_manifest_path()
        .arg(
            opt("format-version", "Format version")
                .value_name("VERSION")
                .possible_value("1"),
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;

    let version = match args.value_of("format-version") {
        None => {
            config.shell().warn(
                "\
                 please specify `--format-version` flag explicitly \
                 to avoid compatibility problems",
            )?;
            1
        }
        Some(version) => version.parse().unwrap(),
    };

    let options = OutputMetadataOptions {
        features: values(args, "features"),
        all_features: args.is_present("all-features"),
        no_default_features: args.is_present("no-default-features"),
        no_deps: args.is_present("no-deps"),
        version,
    };

    let result = ops::output_metadata(&ws, &options)?;
    print_json(&result);
    Ok(())
}
