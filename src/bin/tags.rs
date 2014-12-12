use std::os;

use cargo::core::MultiShell;
use cargo::ops::TagsOptions;
use cargo::ops;
use cargo::util::important_paths::{find_root_manifest_for_cwd};
use cargo::util::{CliResult, CliError};

#[deriving(Decodable)]
struct Options {
    flag_emacs: Option<bool>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
}

pub const USAGE: &'static str = "
Generate a TAGS file for a local package and all of its dependencies.

Usage:
    cargo tags [options]

Options:
    -h, --help               Print this message
    -e                       Generate emacs-compatible tags
    --manifest-path PATH     Path to the manifest to compile
    -v, --verbose            Use verbose output

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.
";

pub fn execute(options: Options, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-tags; args={}", os::args());
    shell.set_verbose(options.flag_verbose);

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let mut opts = TagsOptions {
        vi_tags: options.flag_emacs.unwrap_or(false),
    };

    ops::generate_tags(&root, &mut opts).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
