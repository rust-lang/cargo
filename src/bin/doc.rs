use cargo::core::Workspace;
use cargo::ops::{self, MessageFormat, Packages};
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::{find_root_manifest_for_wd};

#[derive(RustcDecodable)]
pub struct Options {
    flag_target: Option<String>,
    flag_features: Vec<String>,
    flag_all_features: bool,
    flag_jobs: Option<u32>,
    flag_manifest_path: Option<String>,
    flag_no_default_features: bool,
    flag_no_deps: bool,
    flag_open: bool,
    flag_release: bool,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_message_format: MessageFormat,
    flag_package: Vec<String>,
    flag_lib: bool,
    flag_bin: Vec<String>,
    flag_frozen: bool,
    flag_locked: bool,
}

pub const USAGE: &'static str = "
Build a package's documentation

Usage:
    cargo doc [options]

Options:
    -h, --help                   Print this message
    --open                       Opens the docs in a browser after the operation
    -p SPEC, --package SPEC ...  Package to document
    --no-deps                    Don't build documentation for dependencies
    -j N, --jobs N               Number of parallel jobs, defaults to # of CPUs
    --lib                        Document only this package's library
    --bin NAME                   Document only the specified binary
    --release                    Build artifacts in release mode, with optimizations
    --features FEATURES          Space-separated list of features to also build
    --all-features               Build all available features
    --no-default-features        Do not build the `default` feature
    --target TRIPLE              Build for the target triple
    --manifest-path PATH         Path to the manifest to document
    -v, --verbose ...            Use verbose output (-vv very verbose/build.rs output)
    -q, --quiet                  No output printed to stdout
    --color WHEN                 Coloring: auto, always, never
    --message-format FMT         Error format: human, json [default: human]
    --frozen                     Require Cargo.lock and cache are up to date
    --locked                     Require Cargo.lock is up to date

By default the documentation for the local package and all dependencies is
built. The output is all placed in `target/doc` in rustdoc's usual format.

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be documented. If it is not given, then the
current package is documented. For more information on SPEC and its format, see
the `cargo help pkgid` command.
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    config.configure(options.flag_verbose,
                     options.flag_quiet,
                     &options.flag_color,
                     options.flag_frozen,
                     options.flag_locked)?;

    let root = find_root_manifest_for_wd(options.flag_manifest_path, config.cwd())?;

    let empty = Vec::new();
    let doc_opts = ops::DocOptions {
        open_result: options.flag_open,
        compile_opts: ops::CompileOptions {
            config: config,
            jobs: options.flag_jobs,
            target: options.flag_target.as_ref().map(|t| &t[..]),
            features: &options.flag_features,
            all_features: options.flag_all_features,
            no_default_features: options.flag_no_default_features,
            spec: Packages::Packages(&options.flag_package),
            filter: ops::CompileFilter::new(options.flag_lib,
                                            &options.flag_bin,
                                            &empty,
                                            &empty,
                                            &empty),
            message_format: options.flag_message_format,
            release: options.flag_release,
            mode: ops::CompileMode::Doc {
                deps: !options.flag_no_deps,
            },
            target_rustc_args: None,
            target_rustdoc_args: None,
        },
    };

    let ws = Workspace::new(&root, config)?;
    ops::doc(&ws, &doc_opts)?;
    Ok(None)
}
