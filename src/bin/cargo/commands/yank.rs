use crate::command_prelude::*;

use cargo::ops;

pub fn cli() -> App {
    subcommand("yank")
        .about("Remove a pushed crate from the index")
        .arg_quiet()
        .arg(Arg::new("crate"))
        .arg(
            opt("vers", "The version to yank or un-yank")
                .value_name("VERSION")
                .required(true),
        )
        .arg(opt(
            "undo",
            "Undo a yank, putting a version back into the index",
        ))
        .arg(opt("index", "Registry index to yank from").value_name("INDEX"))
        .arg(opt("token", "API token to use when authenticating").value_name("TOKEN"))
        .arg(opt("registry", "Registry to use").value_name("REGISTRY"))
        .after_help("Run `cargo help yank` for more detailed information.\n")
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    config.load_credentials()?;

    let registry = args.registry(config)?;

    ops::yank(
        config,
        args.value_of("crate").map(|s| s.to_string()),
        args.value_of("vers").map(|s| s.to_string()),
        args.value_of("token").map(|s| s.to_string()),
        args.value_of("index").map(|s| s.to_string()),
        args.is_present("undo"),
        registry,
    )?;
    Ok(())
}
