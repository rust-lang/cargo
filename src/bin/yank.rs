use cargo::ops;
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_root_manifest_for_cwd;

#[deriving(Decodable)]
struct Options {
    arg_crate: Option<String>,
    flag_token: Option<String>,
    flag_vers: Option<String>,
    flag_index: Option<String>,
    flag_verbose: bool,
    flag_undo: bool,
}

pub static USAGE: &'static str = "
Remove a pushed crate from the index

Usage:
    cargo yank [options] [<crate>]

Options:
    -h, --help              Print this message
    --vers VERSION          The version to yank or un-yank
    --undo                  Undo a yank, putting a version back into the index
    --index INDEX           Registry index to yank from
    --token TOKEN           API token to use when authenticating
    -v, --verbose           Use verbose output

The yank command removes a previously pushed crate's version from the server's
index. This command does not delete any data, and the crate will still be
available for download via the registry's download link.

Note that existing crates locked to a yanked version will still be able to
download the yanked version to use it. Cargo will, however, not allow any new
crates to be locked to any yanked version.
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(None));
    try!(ops::yank(&root, shell,
                   options.arg_crate,
                   options.flag_vers,
                   options.flag_token,
                   options.flag_index,
                   options.flag_undo).map_err(|e| {
        CliError::from_boxed(e, 101)
    }));
    Ok(None)
}



