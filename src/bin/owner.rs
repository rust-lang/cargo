use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_root_manifest_for_cwd;

#[deriving(Decodable)]
struct Options {
    arg_crate: Option<String>,
    flag_token: Option<String>,
    flag_add: Option<Vec<String>>,
    flag_remove: Option<Vec<String>>,
    flag_index: Option<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Manage the owners of a crate on the registry

Usage:
    cargo owner [options] [<crate>]

Options:
    -h, --help              Print this message
    -a, --add LOGIN         Login of a user to add as an owner
    -r, --remove LOGIN      Login of a user to remove as an owner
    --index INDEX           Registry index to modify owners for
    --token TOKEN           API token to use when authenticating
    -v, --verbose           Use verbose output

This command will modify the owners for a package on the specified registry (or
default). Note that owners of a package can upload new versions, yank old
versions, and also modify the set of owners, so take caution!
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(None));
    try!(ops::modify_owners(&root, shell,
                            options.arg_crate,
                            options.flag_token,
                            options.flag_index,
                            options.flag_add,
                            options.flag_remove).map_err(|e| {
        CliError::from_boxed(e, 101)
    }));
    Ok(None)
}


