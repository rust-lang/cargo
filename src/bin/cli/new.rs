use super::utils::*;

pub fn cli() -> App {
    subcommand("new")
        .about("Create a new cargo package at <path>")
        .arg(Arg::with_name("path").required(true))
        .arg_new_opts()
}
