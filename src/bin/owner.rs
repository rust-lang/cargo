use cargo::ops;
use cargo::util::{CliResult, CliError, Config};

#[derive(RustcDecodable)]
struct Options {
    arg_crate: Option<String>,
    flag_token: Option<String>,
    flag_add: Option<Vec<String>>,
    flag_remove: Option<Vec<String>>,
    flag_index: Option<String>,
    flag_verbose: bool,
    flag_list: bool,
}

pub const USAGE: &'static str = "
Manage the owners of a crate on the registry

Usage:
    cargo owner [options] [<crate>]

Options:
    -h, --help              Print this message
    -a, --add LOGIN         Login of a user to add as an owner
    -r, --remove LOGIN      Login of a user to remove as an owner
    -l, --list              List owners of a crate
    --index INDEX           Registry index to modify owners for
    --token TOKEN           API token to use when authenticating
    -v, --verbose           Use verbose output

This command will modify the owners for a package on the specified registry (or
default). Note that owners of a package can upload new versions, yank old
versions, and also modify the set of owners, so take caution!
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.shell().set_verbose(options.flag_verbose);
    let opts = ops::OwnersOptions {
        krate: options.arg_crate,
        token: options.flag_token,
        index: options.flag_index,
        to_add: options.flag_add,
        to_remove: options.flag_remove,
        list: options.flag_list,
    };
    try!(ops::modify_owners(config, &opts).map_err(|e| {
        CliError::from_boxed(e, 101)
    }));
    Ok(None)
}


