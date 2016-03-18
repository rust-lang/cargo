use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

#[derive(RustcDecodable)]
pub struct Options {
    flag_host: Option<String>,
    flag_token: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: Option<bool>,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_no_verify: bool,
}

pub const USAGE: &'static str = "
Upload a package to the registry

Usage:
    cargo publish [options]

Options:
    -h, --help               Print this message
    --host HOST              Host to upload the package to
    --token TOKEN            Token to use when uploading
    --no-verify              Don't verify package tarball before publish
    --manifest-path PATH     Path to the manifest of the package to publish
    -v, --verbose            Use verbose output
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never

";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    try!(config.configure_shell(options.flag_verbose,
                                options.flag_quiet,
                                &options.flag_color));
    let Options {
        flag_token: token,
        flag_host: host,
        flag_manifest_path,
        flag_no_verify: no_verify,
        ..
    } = options;

    let root = find_root_manifest_for_wd(flag_manifest_path.clone(), config.cwd())?;
    ops::publish(&root, config, token, host, !no_verify)?;
    Ok(None)
}
