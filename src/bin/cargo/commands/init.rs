use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("init")
        .about("Create a new cargo package in an existing directory")
        .arg_quiet()
        .arg(Arg::new("path").default_value("."))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .arg_new_opts()
        .after_help("Run `cargo help init` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let opts = args.new_options(config)?;
    let project_kind = ops::init(&opts, config)?;
    config
        .shell()
        .status("Created", format!("{} package", project_kind))?;
    Ok(())
}
