use cargo::core::Workspace;
use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

#[derive(Deserialize)]
pub struct Options {
    flag_target: Option<String>,
    flag_index: Option<String>,
    flag_token: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_dry_run: bool,
    flag_frozen: bool,
    flag_locked: bool,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
    flag_registry: Option<String>,
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
    --index INDEX            Registry index to publish build info to
    --token TOKEN            Token to use when uploading
    --manifest-path PATH     Path to the manifest of the package to publish
    --dry-run                Perform all checks without uploading
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo
    --registry REGISTRY      Registry to use

";

pub fn execute(options: Options, config: &mut Config) -> CliResult {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;

    let Options {
        flag_token: token,
        flag_index: index,
        flag_manifest_path,
        flag_dry_run: dry_run,
        flag_target: target,
        flag_registry: registry,
        cmd_pass,
        ..
    } = options;

    let root = find_root_manifest_for_wd(flag_manifest_path.clone(), config.cwd())?;
    let ws = Workspace::new(&root, config)?;
    ops::publish_build_info(&ws, ops::PublishBuildInfoOpts {
        config,
        token,
        index,
        dry_run,
        rust_version: config.rustc()?.version_channel_date()?.to_string(),
        registry,
        target,
        passed: cmd_pass,
    })?;
    Ok(())
}
