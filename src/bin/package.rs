use cargo::ops;
use cargo::core::{MultiShell};
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_root_manifest_for_cwd;

#[deriving(Decodable)]
struct Options {
    flag_verbose: bool,
    flag_manifest_path: Option<String>,
    flag_no_verify: bool,
}

pub const USAGE: &'static str = "
Assemble a the local package into a distributable tarball

Usage:
    cargo package [options]

Options:
    -h, --help              Print this message
    --manifest-path PATH    Path to the manifest to compile
    --no-verify             Don't verify the contents by building them
    -v, --verbose           Use verbose output

";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));
    ops::package(&root, shell, !options.flag_no_verify).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
