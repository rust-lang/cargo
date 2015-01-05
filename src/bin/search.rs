use cargo::ops;
use cargo::core::{MultiShell};
use cargo::util::{CliResult, CliError};

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

Options:
    -h, --help              Print this message
    --host HOST             Host of a registry to search in
    -v, --verbose           Use verbose output
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let Options {
        flag_host: host,
        arg_query: query,
        ..
    } = options;

    ops::search(query.as_slice(), shell, host)
        .map(|_| None)
        .map_err(|err| CliError::from_boxed(err, 101))
}
