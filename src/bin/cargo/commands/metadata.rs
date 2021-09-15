use crate::command_prelude::*;
use anyhow::anyhow;
use cargo::ops::{self, BinaryDepsMode, OutputMetadataOptions};

pub fn cli() -> App {
    subcommand("metadata")
        .about(
            "Output the resolved dependencies of a package, \
             the concrete used versions including overrides, \
             in machine-readable format",
        )
        .arg(opt("quiet", "Do not print cargo log messages").short("q"))
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
        .arg(
            opt("binary-deps", "How to treat binary dependencies")
                .possible_values(&["include-if-no-library-dep", "ignore"])
                .default_value("ignore"),
        )
        .after_help("Run `cargo help metadata` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
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

    let binary_deps = {
        match args
            .value_of("binary-deps")
            .unwrap()
            .to_ascii_lowercase()
            .as_str()
        {
            "include-if-no-library-dep" => BinaryDepsMode::IncludeIfNoLibraryDep,
            "ignore" => BinaryDepsMode::Ignore,
            s => {
                return Err(CliError::new(
                    anyhow!("invalid binary-deps specifier: `{}`", s),
                    1,
                ))
            }
        }
    };

    let options = OutputMetadataOptions {
        cli_features: args.cli_features()?,
        no_deps: args.is_present("no-deps"),
        filter_platforms: args._values_of("filter-platform"),
        version,
        binary_deps,
    };

    let result = ops::output_metadata(&ws, &options)?;
    config.shell().print_json(&result)?;
    Ok(())
}
