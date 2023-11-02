use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> Command {
    subcommand("init")
        .about("Create a new cargo package in an existing directory")
        .arg(
            Arg::new("path")
                .value_name("PATH")
                .action(ArgAction::Set)
                .default_value("."),
        )
        .arg_new_opts()
        .arg_registry("Registry to use")
        .arg_quiet()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help init</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let opts = args.new_options(config)?;
    let project_kind = ops::init(&opts, config)?;
    config
        .shell()
        .status("Created", format!("{} package", project_kind))?;
    Ok(())
}
