use cargo::core::Workspace;
use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

#[derive(Deserialize)]
pub struct Options {
    flag_target: Option<String>,
    flag_host: Option<String>,
    flag_token: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_dry_run: bool,
    flag_frozen: bool,
    flag_locked: bool,
    cmd_pass: bool,
    #[allow(dead_code)] // Pass and fail are mutually exclusive
    cmd_fail: bool,
}

pub const USAGE: &'static str = "
Upload a package's build info to the registry: whether the crate built
successfully on a particular target with a particular version of Rust.

Usage:
    cargo publish-build-info [options] (pass|fail)

Options:
    -h, --help               Print this message
    --target TRIPLE          Build for the target triple
    --host HOST              Host to upload the package to
    --token TOKEN            Token to use when uploading
    --manifest-path PATH     Path to the manifest of the package to publish
    --dry-run                Perform all checks without uploading
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date

";

pub fn execute(options: Options, config: &Config) -> CliResult {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked)?;

    let Options {
        flag_token: token,
        flag_host: host,
        flag_manifest_path,
        flag_dry_run: dry_run,
        flag_target: target,
        cmd_pass,
        ..
    } = options;

    let root = find_root_manifest_for_wd(flag_manifest_path.clone(), config.cwd())?;
    let ws = Workspace::new(&root, config)?;
    ops::publish_build_info(&ws, ops::PublishBuildInfoOpts {
        config: config,
        token: token,
        index: host,
        dry_run: dry_run,
        rust_version: config.rustc()?.version_channel_date()?.to_string(),
        target: target,
        passed: cmd_pass,
    })?;
    Ok(())
}
