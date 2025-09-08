use cargo::ops;
use cargo::ops::RegistryOrIndex;

use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("logout")
        .about("Remove an API token from the registry locally")
        .arg_registry("Registry to use")
        .arg_silent_suggestion()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help logout</>` for more detailed information.\n"
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let reg = args.registry_or_index(gctx)?;
    assert!(
        !matches!(reg, Some(RegistryOrIndex::Index(..))),
        "must not be index URL"
    );

    ops::registry_logout(gctx, reg)?;
    Ok(())
}
