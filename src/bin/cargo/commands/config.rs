use crate::command_prelude::*;
use cargo::ops::cargo_config;

pub fn cli() -> App {
    subcommand("config")
        .about("Inspect configuration values")
        .after_help("Run `cargo help config` for more detailed information.\n")
        .setting(clap::AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            subcommand("get")
                .arg(Arg::with_name("key").help("The config key to display"))
                .arg(
                    opt("format", "Display format")
                        .possible_values(cargo_config::ConfigFormat::POSSIBLE_VALUES)
                        .default_value("toml"),
                )
                .arg(opt(
                    "show-origin",
                    "Display where the config value is defined",
                ))
                .arg(
                    opt("merged", "Whether or not to merge config values")
                        .possible_values(&["yes", "no"])
                        .default_value("yes"),
                ),
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    config
        .cli_unstable()
        .fail_if_stable_command(config, "config", 9301)?;
    match args.subcommand() {
        ("get", Some(args)) => {
            let opts = cargo_config::GetOptions {
                key: args.value_of("key"),
                format: args.value_of("format").unwrap().parse()?,
                show_origin: args.is_present("show-origin"),
                merged: args.value_of("merged") == Some("yes"),
            };
            cargo_config::get(config, &opts)?;
        }
        (cmd, _) => {
            panic!("unexpected command `{}`", cmd)
        }
    }
    Ok(())
}
