use core::Workspace;
use ops::{self, CompileOptions, MessageFormat};
use util::important_paths::{find_root_manifest_for_wd};
use util::{CliResult, Config};

#[derive(RustcDecodable)]
pub struct Options {
    flag_package: Vec<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_all_features: bool,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_message_format: MessageFormat,
    flag_release: bool,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_example: Vec<String>,
    flag_test: Vec<String>,
    flag_bench: Vec<String>,
    flag_locked: bool,
    flag_frozen: bool,
}

impl Options {
    pub fn default() -> Options {
        Options {
            flag_package: vec![],
            flag_jobs: None,
            flag_features: vec![],
            flag_all_features: false,
            flag_no_default_features: false,
            flag_target: None,
            flag_manifest_path: None,
            flag_verbose: 0,
            flag_quiet: None,
            flag_color: None,
            flag_message_format: MessageFormat::Human,
            flag_release: false,
            flag_lib: false,
            flag_bin: vec![],
            flag_example: vec![],
            flag_test: vec![],
            flag_bench: vec![],
            flag_locked: false,
            flag_frozen: false,
        }
    }
}


pub fn with_check_env<F>(options: Options, config: &Config, f: F) -> CliResult<Option<()>>
    where F: FnOnce(&Workspace, &CompileOptions) -> CliResult<Option<()>>
{
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked)?;

    let root = find_root_manifest_for_wd(options.flag_manifest_path, config.cwd())?;

    let opts = CompileOptions {
        config: config,
        jobs: options.flag_jobs,
        target: options.flag_target.as_ref().map(|t| &t[..]),
        features: &options.flag_features,
        all_features: options.flag_all_features,
        no_default_features: options.flag_no_default_features,
        spec: ops::Packages::Packages(&options.flag_package),
        mode: ops::CompileMode::Check,
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
    f(&ws, &opts)
}
