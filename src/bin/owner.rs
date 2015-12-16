use cargo::ops;
use cargo::util::{CliResult, Config};

#[derive(RustcDecodable)]
struct Options {
    arg_crate: Option<String>,
    flag_token: Option<String>,
    flag_add: Option<Vec<String>>,
    flag_remove: Option<Vec<String>>,
    flag_index: Option<String>,
    flag_verbose: bool,
    flag_quiet: bool,
    flag_color: Option<String>,
    flag_list: bool,
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
    -v, --verbose            Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never

This command will modify the owners for a package on the specified registry (or
default). Note that owners of a package can upload new versions, yank old
versions. Explicitly named owners can also modify the set of owners, so take
caution!

See http://doc.crates.io/crates-io.html#cargo-owner for detailed documentation
and troubleshooting.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.shell().set_verbosity(options.flag_verbose, options.flag_quiet));
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));
    let opts = ops::OwnersOptions {
        krate: options.arg_crate,
        token: options.flag_token,
        index: options.flag_index,
        to_add: options.flag_add,
        to_remove: options.flag_remove,
        list: options.flag_list,
    };
    try!(ops::modify_owners(config, &opts));
    Ok(None)
}

