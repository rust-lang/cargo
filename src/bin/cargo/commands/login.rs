use cargo::ops;
use cargo::ops::RegistryOrIndex;

use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("login")
        .about("Log in to a registry.")
        .arg(
            Arg::new("token")
                .value_name("TOKEN")
                .action(ArgAction::Set)
                .hide(true),
        )
        .arg_registry("Registry to use")
        .arg(
            Arg::new("args")
                .help("Additional arguments for the credential provider")
                .num_args(0..)
                .last(true),
        )
        .arg_silent_suggestion()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help login</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let reg = args.registry_or_index(gctx)?;
    assert!(
        !matches!(reg, Some(RegistryOrIndex::Index(..))),
        "must not be index URL"
    );

    let token = args.get_one::<String>("token").map(|s| s.as_str().into());
    if token.is_some() {
        let _ = gctx
            .shell()
            .warn("`cargo login <token>` is deprecated in favor of reading `<token>` from stdin");
    }

    let extra_args = args
        .get_many::<String>("args")
        .unwrap_or_default()
        .map(String::as_str)
        .collect::<Vec<_>>();
    ops::registry_login(gctx, token, reg.as_ref(), &extra_args)?;
    Ok(())
}
