use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("login")
        .about("Log in to a registry.")
        .arg_quiet()
        .arg(Arg::new("token").action(ArgAction::Set))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help login` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let registry = args.registry(config)?;
    ops::registry_login(
        config,
        args.get_one::<String>("token").map(|s| s.as_str().into()),
        registry.as_deref(),
    )?;
    Ok(())
}
