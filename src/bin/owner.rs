use cargo::ops;
use cargo::util::{CliResult, Config};

#[derive(Deserialize)]
pub struct Options {
    arg_crate: Option<String>,
    flag_token: Option<String>,
    flag_add: Option<Vec<String>>,
    flag_remove: Option<Vec<String>>,
    flag_index: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_list: bool,
    flag_frozen: bool,
    flag_locked: bool,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
}

pub const USAGE: &'static str = "
Manage the owners of a crate on the registry

Usage:
    cargo owner [options] [<crate>]

Options:
    -h, --help               Print this message
    -a, --add LOGIN          Name of a user or team to add as an owner
    -r, --remove LOGIN       Name of a user or team to remove as an owner
    -l, --list               List owners of a crate
    --index INDEX            Registry index to modify owners for
    --token TOKEN            API token to use when authenticating
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo

This command will modify the owners for a package on the specified registry (or
default). Note that owners of a package can upload new versions, yank old
versions. Explicitly named owners can also modify the set of owners, so take
caution!

See http://doc.crates.io/crates-io.html#cargo-owner for detailed documentation
and troubleshooting.
";

pub fn execute(options: Options, config: &Config) -> CliResult {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;
    let opts = ops::OwnersOptions {
        krate: options.arg_crate,
        token: options.flag_token,
        index: options.flag_index,
        to_add: options.flag_add,
        to_remove: options.flag_remove,
        list: options.flag_list,
    };
    ops::modify_owners(config, &opts)?;
    Ok(())
}

