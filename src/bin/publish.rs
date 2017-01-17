use cargo::core::Workspace;
use cargo::ops;
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::find_root_manifest_for_wd;
pub use super::options::PublishCommandFlags as Options;
pub use super::options::PUBLISH_COMMAND_USAGE as USAGE;

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked)?;
    let Options {
        flag_token: token,
        flag_host: host,
        flag_manifest_path,
        flag_no_verify: no_verify,
        flag_allow_dirty: allow_dirty,
        flag_jobs: jobs,
        flag_dry_run: dry_run,
        ..
    } = options;

    let root = find_root_manifest_for_wd(flag_manifest_path.clone(), config.cwd())?;
    let ws = Workspace::new(&root, config)?;
    ops::publish(&ws, &ops::PublishOpts {
        config: config,
        token: token,
        index: host,
        verify: !no_verify,
        allow_dirty: allow_dirty,
        jobs: jobs,
        dry_run: dry_run,
    })?;
    Ok(None)
}
