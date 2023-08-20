use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("login")
        .about("Log in to a registry.")
        .arg(Arg::new("token").action(ArgAction::Set))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .arg(
            Arg::new("args")
                .help("Arguments for the credential provider (unstable)")
                .num_args(0..)
                .last(true),
        )
        .arg_quiet()
        .after_help("Run `cargo help login` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let registry = args.registry(config)?;
    let extra_args = args
        .get_many::<String>("args")
        .unwrap_or_default()
        .map(String::as_str)
        .collect::<Vec<_>>();
    ops::registry_login(
        config,
        args.get_one::<String>("token").map(|s| s.as_str().into()),
        registry.as_deref(),
        &extra_args,
    )?;
    Ok(())
}
