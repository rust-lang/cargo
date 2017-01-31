use cargo::util::{CliResult, CliError, Config, human};

#[derive(RustcDecodable)]
pub struct Options;

pub const USAGE: &'static str = "
Get some help with a cargo command.

Usage:
    cargo help <command>
    cargo help -h | --help

Options:
    -h, --help          Print this message
";

pub fn execute(_: Options, _: &Config) -> CliResult {
    // This is a dummy command just so that `cargo help help` works.
    // The actual delegation of help flag to subcommands is handled by the
    // cargo command.
    Err(CliError::new(human("help command should not be executed directly"), 101))
}
