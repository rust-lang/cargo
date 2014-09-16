use docopt;

use cargo::ops;
use cargo::core::{MultiShell};
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_root_manifest_for_cwd;

docopt!(Options, "
Upload a package to the registry

Usage:
    cargo upload [options]

Options:
    -h, --help              Print this message
    --host HOST             Host to upload the package to
    --token TOKEN           Token to use when uploading
    --manifest-path PATH    Path to the manifest to compile
    -v, --verbose           Use verbose output

", flag_host: Option<String>, flag_token: Option<String>,
   flag_manifest_path: Option<String>)

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    shell.set_verbose(options.flag_verbose);
    let Options {
        flag_token: token,
        flag_host: host,
        flag_manifest_path,
        ..
    } = options;

    let root = try!(find_root_manifest_for_cwd(flag_manifest_path.clone()));
    ops::upload(&root, shell, token, host).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
