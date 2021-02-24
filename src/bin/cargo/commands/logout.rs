use crate::command_prelude::*;
use anyhow::format_err;
use cargo::core::features;
use cargo::ops;

pub fn cli() -> App {
    subcommand("logout")
        .about("Remove an API token from the registry locally")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help logout` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let unstable = config.cli_unstable();
    if !(unstable.credential_process || unstable.unstable_options) {
        const SEE: &str = "See https://github.com/rust-lang/cargo/issues/8933 for more \
        information about the `cargo logout` command.";
        if config.nightly_features_allowed {
            return Err(format_err!(
                "the `cargo logout` command is unstable, pass `-Z unstable-options` to enable it\n\
                {}",
                SEE
            )
            .into());
        } else {
            return Err(format_err!(
                "the `cargo logout` command is unstable, and only available on the \
                 nightly channel of Cargo, but this is the `{}` channel\n\
                 {}\n\
                 {}",
                features::channel(),
                features::SEE_CHANNELS,
                SEE
            )
            .into());
        }
    }
    config.load_credentials()?;
    ops::registry_logout(config, args.value_of("registry").map(String::from))?;
    Ok(())
}
