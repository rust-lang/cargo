use cargo::ops;
use cargo::util::{CliResult, Config};

#[derive(Deserialize)]
pub struct Options {
    flag_index: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_frozen: bool,
    flag_locked: bool,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
}

pub const USAGE: &'static str = "
Check if an api token exists locally and who it belongs to

Usage:
    cargo whoami [options] [<token>]

Options:
    -h, --help               Print this message
    --index INDEX            Registry index to search in
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo

";

pub fn execute(options: Options, config: &mut Config) -> CliResult {
    config.configure(
        options.flag_verbose,
        options.flag_quiet,
        &options.flag_color,
        options.flag_frozen,
        options.flag_locked,
        &options.flag_z,
    )?;

    let Options {
        flag_index: index,
        ..
    } = options;

    ops::registry_whoami(config, index)?;
    Ok(())
}
