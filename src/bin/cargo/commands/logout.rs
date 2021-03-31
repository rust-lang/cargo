use crate::command_prelude::*;
use cargo::ops;

pub fn cli() -> App {
    subcommand("logout")
        .about("Remove an API token from the registry locally")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help logout` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    if !config.cli_unstable().credential_process {
        config
            .cli_unstable()
            .fail_if_stable_command(config, "logout", 8933)?;
    }
    config.load_credentials()?;
    ops::registry_logout(config, args.value_of("registry").map(String::from))?;
    Ok(())
}
