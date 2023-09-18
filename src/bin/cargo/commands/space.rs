use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("space")
        .about("Easter Egg")
        .arg_quiet()
        .after_help(color_print::cstr!(
            "This is an `<cyan,bold>Easter egg</>`!\n"
        ))
}

pub fn exec(config: &mut Config, _args: &ArgMatches) -> CliResult {
    cargo::drop_print!(config, color_print::cstr!("<red>error<white>: car no go space: `car no fly`\n"));
    Ok(())
}
