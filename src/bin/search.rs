use cargo::ops;
use cargo::util::{CliResult, Config};

use std::cmp;

#[derive(Deserialize)]
pub struct Options {
    flag_index: Option<String>,
    flag_host: Option<String>,  // TODO: Depricated, remove
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_limit: Option<u32>,
    flag_frozen: bool,
    flag_locked: bool,
    arg_query: Vec<String>,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
}

pub const USAGE: &'static str = "
Search packages in crates.io

Usage:
    cargo search [options] <query>...
    cargo search [-h | --help]

Options:
    -h, --help               Print this message
    --index INDEX            Registry index to search in
    --host HOST              DEPRICATED, renamed to '--index'
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --limit LIMIT            Limit the number of results (default: 10, max: 100)
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo
";

pub fn execute(options: Options, config: &Config) -> CliResult {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;
    let Options {
        flag_index: index,
        flag_host: host,    // TODO: Depricated, remove
        flag_limit: limit,
        arg_query: query,
        ..
    } = options;

    // TODO: Depricated
    // remove once it has been decided --host can be safely removed
    // We may instead want to repurpose the host flag, as
    // mentioned in this issue
    // https://github.com/rust-lang/cargo/issues/4208

    let msg = "The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
depricated. The flag is being renamed to 'index', as the flag
wants the location of the index in which to search. Please
use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.";

    let index = if host.clone().is_none() || host.clone().unwrap().is_empty() {
        index
    } else {
        config.shell().warn(&msg)?;
        host
    };

    ops::search(&query.join("+"), config, index, cmp::min(100, limit.unwrap_or(10)) as u8)?;
    Ok(())
}
