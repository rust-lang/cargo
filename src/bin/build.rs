use std::env;

use cargo::core::Workspace;
use cargo::ops::{self, CompileOptions, Packages};
use cargo::util::important_paths::{find_root_manifest_for_wd};
use cargo::util::{CliResult, Config};
pub use super::options::BuildCommandFlags as Options;
pub use super::options::BUILD_COMMAND_USAGE as USAGE;

pub fn execute(options: Options, config: &Config) -> CliResult {
    debug!("executing; cmd=cargo-build; args={:?}",
           env::args().collect::<Vec<_>>());
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked)?;

    let root = find_root_manifest_for_wd(options.flag_manifest_path, config.cwd())?;

    let spec = if options.flag_all {
        Packages::All
    } else {
        Packages::Packages(&options.flag_package)
    };

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        all_features: options.flag_all_features,
        no_default_features: options.flag_no_default_features,
        spec: spec,
        mode: ops::CompileMode::Build,
        release: options.flag_release,
        filter: ops::CompileFilter::new(options.flag_lib,
                                        &options.flag_bin,
                                        &options.flag_test,
                                        &options.flag_example,
                                        &options.flag_bench),
        message_format: options.flag_message_format,
        target_rustdoc_args: None,
        target_rustc_args: None,
    };

    let ws = Workspace::new(&root, config)?;
    ops::compile(&ws, &opts)?;
    Ok(())
}
