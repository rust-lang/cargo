use cargo::core::Workspace;
use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;

#[derive(Deserialize)]
pub struct Options {
    flag_index: Option<String>,
    flag_host: Option<String>,
    flag_token: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_no_verify: bool,
    flag_allow_dirty: bool,
    flag_jobs: Option<u32>,
    flag_dry_run: bool,
    flag_frozen: bool,
    flag_locked: bool,
}

pub const USAGE: &'static str = "
Upload a package to the registry

Usage:
    cargo publish [options]

Options:
    -h, --help               Print this message
    --index INDEX            Host to upload the package to
    --host HOST              DEPRECATED
    --token TOKEN            Token to use when uploading
    --no-verify              Don't verify package tarball before publish
    --allow-dirty            Allow publishing with a dirty source directory
    --manifest-path PATH     Path to the manifest of the package to publish
    -j N, --jobs N           Number of parallel jobs, defaults to # of CPUs
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
        flag_index: index,
        flag_host: host,
        flag_manifest_path,
        flag_no_verify: no_verify,
        flag_allow_dirty: allow_dirty,
        flag_jobs: jobs,
        flag_dry_run: dry_run,
        ..
    } = options;


    let msg = "The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
deprecated. The flag is being renamed to 'index', as the flag
wants the location of the index to which to publish. Please
use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.";

    let root = find_root_manifest_for_wd(flag_manifest_path.clone(), config.cwd())?;
    let ws = Workspace::new(&root, config)?;
    ops::publish(&ws, &ops::PublishOpts {
        config: config,
        token: token,
        index:
            if host.clone().is_none() || host.clone().unwrap().is_empty() { index }
            else { config.shell().warn(&msg)?; host },
        verify: !no_verify,
        allow_dirty: allow_dirty,
        jobs: jobs,
        dry_run: dry_run,
    })?;
    Ok(())
}
