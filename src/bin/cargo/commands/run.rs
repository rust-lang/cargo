use crate::command_prelude::*;

use cargo::core::Verbosity;
use cargo::ops::{self, CompileFilter};

pub fn cli() -> App {
    subcommand("run")
        // subcommand aliases are handled in aliased_command()
        // .alias("r")
        .setting(AppSettings::TrailingVarArg)
        .about("Run a binary or example of the local package")
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(Arg::with_name("args").multiple(true))
        .arg_targets_bin_example(
            "Name of the bin target to run",
            "Name of the example target to run",
        )
        .arg_package("Package with the target to run")
        .arg_jobs()
        .arg_release("Build artifacts in release mode, with optimizations")
        .arg_features()
        .arg_target_triple("Build for the target triple")
        .arg_target_dir()
        .arg_manifest_path()
        .arg_message_format()
        .after_help(
            "\
If neither `--bin` nor `--example` are given, then if the package only has one
bin target it will be run. Otherwise `--bin` specifies the bin target to run,
and `--example` specifies the example target to run. At most one of `--bin` or
`--example` can be provided.

All the arguments following the two dashes (`--`) are passed to the binary to
run. If you're passing arguments to both Cargo and the binary, the ones after
`--` go to the binary, the ones before go to Cargo.
",
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let ws = args.workspace(config)?;

    let mut compile_opts = args.compile_options(config, CompileMode::Build, Some(&ws))?;

    if !args.is_present("example") && !args.is_present("bin") {
        let default_runs: Vec<_> = compile_opts
            .spec
            .get_packages(&ws)?
            .iter()
            .filter_map(|pkg| pkg.manifest().default_run())
            .collect();
        if default_runs.len() == 1 {
            compile_opts.filter = CompileFilter::from_raw_arguments(
                false,
                vec![default_runs[0].to_owned()],
                false,
                vec![],
                false,
                vec![],
                false,
                vec![],
                false,
                false,
            );
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
    match ops::run(&ws, &compile_opts, &values(args, "args"))? {
        None => Ok(()),
        Some(err) => {
            // If we never actually spawned the process then that sounds pretty
            // bad and we always want to forward that up.
            let exit = match err.exit {
                Some(exit) => exit,
                None => return Err(CliError::new(err.into(), 101)),
            };

            // If `-q` was passed then we suppress extra error information about
            // a failed process, we assume the process itself printed out enough
            // information about why it failed so we don't do so as well
            let exit_code = exit.code().unwrap_or(101);
            let is_quiet = config.shell().verbosity() == Verbosity::Quiet;
            Err(if is_quiet {
                CliError::code(exit_code)
            } else {
                CliError::new(err.into(), exit_code)
            })
        }
    }
}
