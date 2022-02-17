use crate::command_prelude::*;
use crate::util::restricted_names::is_glob_pattern;
use cargo::core::Verbosity;
use cargo::ops::{self, CompileFilter, Packages};
use cargo_util::ProcessError;

pub fn cli() -> App {
    subcommand("run")
        // subcommand aliases are handled in aliased_command()
        // .alias("r")
        .trailing_var_arg(true)
        .about("Run a binary or example of the local package")
        .arg_quiet()
        .arg(
            Arg::new("args")
                .allow_invalid_utf8(true)
                .multiple_values(true),
        )
        .arg_targets_bin_example(
            "Name of the bin target to run",
            "Name of the example target to run",
        )
        .arg_package("Package with the target to run")
        .arg_jobs()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_profile("Build artifacts with the specified profile")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .arg_unit_graph()
        .arg_ignore_rust_version()
        .arg_timings()
        .after_help("Run `cargo help run` for more detailed information.\n")
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

    if !args.is_present("example") && !args.is_present("bin") {
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

    ops::run(&ws, &compile_opts, &values_os(args, "args")).map_err(|err| {
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
    })
}
