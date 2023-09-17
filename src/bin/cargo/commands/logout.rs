use cargo::ops;
use cargo::ops::RegistryOrIndex;

use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("logout")
        .about("Remove an API token from the registry locally")
        .arg_registry("Registry to use")
        .arg_quiet()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help logout</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let reg = args.registry_or_index(config)?;
    assert!(
        !matches!(reg, Some(RegistryOrIndex::Index(..))),
        "must not be index URL"
    );

    ops::registry_logout(config, reg)?;
    Ok(())
}
