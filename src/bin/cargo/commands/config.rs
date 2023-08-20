use crate::command_prelude::*;
use cargo::ops::cargo_config;

pub fn cli() -> Command {
    subcommand("config")
        .about("Inspect configuration values")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            subcommand("get")
                .arg(
                    Arg::new("key")
                        .action(ArgAction::Set)
                        .help("The config key to display"),
                )
                .arg(
                    opt("format", "Display format")
                        .value_parser(cargo_config::ConfigFormat::POSSIBLE_VALUES)
                        .default_value("toml"),
                )
                .arg(flag(
                    "show-origin",
                    "Display where the config value is defined",
                ))
                .arg(
                    opt("merged", "Whether or not to merge config values")
                        .value_parser(["yes", "no"])
                        .default_value("yes"),
                ),
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    config
        .cli_unstable()
        .fail_if_stable_command(config, "config", 9301)?;
    match args.subcommand() {
        Some(("get", args)) => {
            let opts = cargo_config::GetOptions {
                key: args.get_one::<String>("key").map(String::as_str),
                format: args.get_one::<String>("format").unwrap().parse()?,
                show_origin: args.flag("show-origin"),
                merged: args.get_one::<String>("merged").map(String::as_str) == Some("yes"),
            };
            cargo_config::get(config, &opts)?;
        }
        Some((cmd, _)) => {
            unreachable!("unexpected command {}", cmd)
        }
        None => {
            unreachable!("unexpected command")
        }
    }
    Ok(())
}
