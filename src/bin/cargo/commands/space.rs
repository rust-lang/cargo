use crate::command_prelude::*;

pub fn cli() -> App {
    subcommand("space")
        .about("Tell you where cargo")
        .after_help("Run `cargo help space` for more detailed information.\n")
}

pub fn exec(_config: &mut Config, _args: &ArgMatches<'_>) -> CliResult {
    return Err(anyhow::format_err!("Car no go space, cargo road").into());
}
