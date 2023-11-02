use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;

use crate::command_prelude::*;
use crate::util::restricted_names::is_glob_pattern;
use cargo::core::Verbosity;
use cargo::core::Workspace;
use cargo::ops::{self, CompileFilter, Packages};
use cargo_util::ProcessError;

pub fn cli() -> Command {
    subcommand("run")
        // subcommand aliases are handled in aliased_command()
        // .alias("r")
        .about("Run a binary or example of the local package")
        .arg(
            Arg::new("args")
                .value_name("ARGS")
                .help("Arguments for the binary or example to run")
                .value_parser(value_parser!(OsString))
                .num_args(0..)
                .trailing_var_arg(true),
        )
        .arg_ignore_rust_version()
        .arg_message_format()
        .arg_quiet()
        .arg_package("Package with the target to run")
        .arg_targets_bin_example(
            "Name of the bin target to run",
            "Name of the example target to run",
        )
        .arg_features()
        .arg_parallel()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_unit_graph()
        .arg_timings()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help run</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(config)?;

    let mut compile_opts = args.compile_options(
        config,
        CompileMode::Build,
        Some(&ws),
        ProfileChecking::Custom,
    )?;

    // Disallow `spec` to be an glob pattern
    if let Packages::Packages(opt_in) = &compile_opts.spec {
        if let Some(pattern) = opt_in.iter().find(|s| is_glob_pattern(s)) {
            return Err(anyhow::anyhow!(
                "`cargo run` does not support glob pattern `{}` on package selection",
                pattern,
            )
            .into());
        }
    }

    if !args.contains_id("example") && !args.contains_id("bin") {
        let default_runs: Vec<_> = compile_opts
            .spec
            .get_packages(&ws)?
            .iter()
            .filter_map(|pkg| pkg.manifest().default_run())
            .collect();
        if let [bin] = &default_runs[..] {
            compile_opts.filter = CompileFilter::single_bin(bin.to_string());
        } else {
            // ops::run will take care of errors if len pkgs != 1.
            compile_opts.filter = CompileFilter::Default {
                // Force this to false because the code in ops::run is not
                // able to pre-check features before compilation starts to
                // enforce that only 1 binary is built.
                required_features_filterable: false,
            };
        }
    };

    ops::run(&ws, &compile_opts, &values_os(args, "args")).map_err(|err| to_run_error(config, err))
}

/// See also `util/toml/mod.rs`s `is_embedded`
pub fn is_manifest_command(arg: &str) -> bool {
    let path = Path::new(arg);
    1 < path.components().count()
        || path.extension() == Some(OsStr::new("rs"))
        || path.file_name() == Some(OsStr::new("Cargo.toml"))
}

pub fn exec_manifest_command(config: &mut Config, cmd: &str, args: &[OsString]) -> CliResult {
    if !config.cli_unstable().script {
        return Err(anyhow::anyhow!("running `{cmd}` requires `-Zscript`").into());
    }

    let manifest_path = Path::new(cmd);
    let manifest_path = root_manifest(Some(manifest_path), config)?;

    // Treat `cargo foo.rs` like `cargo install --path foo` and re-evaluate the config based on the
    // location where the script resides, rather than the environment from where it's being run.
    let parent_path = manifest_path
        .parent()
        .expect("a file should always have a parent");
    config.reload_rooted_at(parent_path)?;

    let mut ws = Workspace::new(&manifest_path, config)?;
    if config.cli_unstable().avoid_dev_deps {
        ws.set_require_optional_deps(false);
    }

    let mut compile_opts =
        cargo::ops::CompileOptions::new(config, cargo::core::compiler::CompileMode::Build)?;
    compile_opts.spec = cargo::ops::Packages::Default;

    cargo::ops::run(&ws, &compile_opts, args).map_err(|err| to_run_error(config, err))
}

fn to_run_error(config: &cargo::util::Config, err: anyhow::Error) -> CliError {
    let proc_err = match err.downcast_ref::<ProcessError>() {
        Some(e) => e,
        None => return CliError::new(err, 101),
    };

    // If we never actually spawned the process then that sounds pretty
    // bad and we always want to forward that up.
    let exit_code = match proc_err.code {
        Some(exit) => exit,
        None => return CliError::new(err, 101),
    };

    // If `-q` was passed then we suppress extra error information about
    // a failed process, we assume the process itself printed out enough
    // information about why it failed so we don't do so as well
    let is_quiet = config.shell().verbosity() == Verbosity::Quiet;
    if is_quiet {
        CliError::code(exit_code)
    } else {
        CliError::new(err, exit_code)
    }
}
