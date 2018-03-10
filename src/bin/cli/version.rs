use command_prelude::*;

pub fn cli() -> App {
    subcommand("version")
        .about("Show version information")
}
