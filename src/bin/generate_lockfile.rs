use std::env;

use cargo::core::Workspace;
use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

#[derive(Deserialize)]
pub struct Options {
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_frozen: bool,
    flag_locked: bool,
    #[serde(rename = "flag_Z")]
    flag_z: Vec<String>,
}

pub const USAGE: &'static str = "
Generate the lockfile for a project

Usage:
    cargo generate-lockfile [options]

Options:
    -h, --help               Print this message
    --manifest-path PATH     Path to the manifest to generate a lockfile for
    -v, --verbose ...        Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet              No output printed to stdout
    --color WHEN             Coloring: auto, always, never
    --frozen                 Require Cargo.lock and cache are up to date
    --locked                 Require Cargo.lock is up to date
    -Z FLAG ...              Unstable (nightly-only) flags to Cargo
";

pub fn execute(options: Options, config: &mut Config) -> CliResult {
    debug!("executing; cmd=cargo-generate-lockfile; args={:?}", env::args().collect::<Vec<_>>());
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked,
                     &options.flag_z)?;
    let root = find_root_manifest_for_wd(options.flag_manifest_path, config.cwd())?;

    let ws = Workspace::new(&root, config)?;
    ops::generate_lockfile(&ws)?;
    Ok(())
}
