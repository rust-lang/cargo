use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("init")
        .about("Create a new cargo package in an existing directory")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("path").default_value("."))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .arg_new_opts()
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let opts = args.new_options(config)?;
    ops::init(&opts, config)?;
    config
        .shell()
        .status("Created", format!("{} package", opts.kind))?;
    Ok(())
}
