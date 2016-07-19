use cargo::ops;
use cargo::util::{CliResult, Config};

use std::cmp;

#[derive(RustcDecodable)]
pub struct Options {
    flag_host: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_limit: Option<u32>,
    flag_frozen: bool,
    flag_locked: bool,
    arg_query: Vec<String>,
}

pub const USAGE: &'static str = "
Search packages in crates.io

Usage:
    cargo search [options] <query>...
    cargo search [-h | --help]

Options:
    -h, --help               Print this message
    --host HOST              Host of a registry to search in
    -v, --verbose ...        Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --limit LIMIT            Limit the number of results (default: 10, max: 100)
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.configure(options.flag_verbose,
                          options.flag_quiet,
                          &options.flag_color,
                          options.flag_frozen,
                          options.flag_locked));
    let Options {
        flag_host: host,
        flag_limit: limit,
        arg_query: query,
        ..
    } = options;

    try!(ops::search(&query.join("+"), config, host, cmp::min(100, limit.unwrap_or(10)) as u8));
    Ok(None)
}
