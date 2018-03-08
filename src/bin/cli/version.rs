use super::utils::*;

pub fn cli() -> App {
    subcommand("version")
        .about("Show version information")
}
