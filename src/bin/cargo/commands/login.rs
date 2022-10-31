use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("login")
        .about(
            "Save an api token from the registry locally. \
             If token is not specified, it will be read from stdin.",
        )
        .arg_quiet()
        .arg(Arg::new("token").action(ArgAction::Set))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help login` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    ops::registry_login(
        config,
        args.get_one("token").map(String::as_str),
        args.get_one("registry").map(String::as_str),
    )?;
    Ok(())
}
