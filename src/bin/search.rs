use cargo::ops;
use cargo::util::{CliResult, CliError, Config};

#[derive(RustcDecodable)]
struct Options {
    flag_host: Option<String>,
    flag_verbose: bool,
    arg_query: String
}

pub const USAGE: &'static str = "
Search packages in crates.io

Usage:
    cargo search [options] <query>
    cargo search [-h | --help]

Options:
    -h, --help              Print this message
    --host HOST             Host of a registry to search in
    -v, --verbose           Use verbose output
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.shell().set_verbose(options.flag_verbose);
    let Options {
        flag_host: host,
        arg_query: query,
        ..
    } = options;

    ops::search(&query, config, host)
        .map(|_| None)
        .map_err(|err| CliError::from_boxed(err, 101))
}
