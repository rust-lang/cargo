use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;

use crate::command_prelude::*;
use crate::util::restricted_names::is_glob_pattern;
use cargo::core::Verbosity;
use cargo::core::Workspace;
use cargo::ops::{self, CompileFilter, Packages};
use cargo::util::closest;
use cargo_util::ProcessError;
use itertools::Itertools as _;

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
        .arg_message_format()
        .arg_silent_suggestion()
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
        .arg_lockfile_path()
        .arg_ignore_rust_version()
        .arg_unit_graph()
        .arg_timings()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help run</>` for more detailed information.\n\
             To pass `--help` to the specified binary, use `<bright-cyan,bold>-- --help</>`.\n",
        ))
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let ws = args.workspace(gctx)?;

    let mut compile_opts =
        args.compile_options(gctx, UserIntent::Build, Some(&ws), ProfileChecking::Custom)?;

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

    ops::run(&ws, &compile_opts, &values_os(args, "args")).map_err(|err| to_run_error(gctx, err))
}

/// See also `util/toml/mod.rs`s `is_embedded`
pub fn is_manifest_command(arg: &str) -> bool {
    let path = Path::new(arg);
    1 < path.components().count() || path.extension() == Some(OsStr::new("rs"))
}

pub fn exec_manifest_command(gctx: &mut GlobalContext, cmd: &str, args: &[OsString]) -> CliResult {
    let manifest_path = Path::new(cmd);
    match (manifest_path.is_file(), gctx.cli_unstable().script) {
        (true, true) => {}
        (true, false) => {
            return Err(anyhow::anyhow!("running the file `{cmd}` requires `-Zscript`").into());
        }
        (false, true) => {
            let possible_commands = crate::list_commands(gctx);
            let is_dir = if manifest_path.is_dir() {
                format!(": `{cmd}` is a directory")
            } else {
                "".to_owned()
            };
            let suggested_command = if let Some(suggested_command) = possible_commands
                .keys()
                .filter(|c| cmd.starts_with(c.as_str()))
                .max_by_key(|c| c.len())
            {
                let actual_args = cmd.strip_prefix(suggested_command).unwrap();
                let args = if args.is_empty() {
                    "".to_owned()
                } else {
                    format!(
                        " {}",
                        args.into_iter().map(|os| os.to_string_lossy()).join(" ")
                    )
                };
                format!(
                    "\nhelp: there is a command with a similar name: `{suggested_command} {actual_args}{args}`"
                )
            } else {
                "".to_owned()
            };
            let suggested_script = if let Some(suggested_script) = suggested_script(cmd) {
                format!("\nhelp: there is a script with a similar name: `{suggested_script}`")
            } else {
                "".to_owned()
            };
            return Err(anyhow::anyhow!(
                "no such file or subcommand `{cmd}`{is_dir}{suggested_command}{suggested_script}"
            )
            .into());
        }
        (false, false) => {
            // HACK: duplicating the above for minor tweaks but this will all go away on
            // stabilization
            let possible_commands = crate::list_commands(gctx);
            let suggested_command = if let Some(suggested_command) = possible_commands
                .keys()
                .filter(|c| cmd.starts_with(c.as_str()))
                .max_by_key(|c| c.len())
            {
                let actual_args = cmd.strip_prefix(suggested_command).unwrap();
                let args = if args.is_empty() {
                    "".to_owned()
                } else {
                    format!(
                        " {}",
                        args.into_iter().map(|os| os.to_string_lossy()).join(" ")
                    )
                };
                format!(
                    "\nhelp: there is a command with a similar name: `{suggested_command} {actual_args}{args}`"
                )
            } else {
                "".to_owned()
            };
            let suggested_script = if let Some(suggested_script) = suggested_script(cmd) {
                format!(
                    "\nhelp: there is a script with a similar name: `{suggested_script}` (requires `-Zscript`)"
                )
            } else {
                "".to_owned()
            };
            return Err(anyhow::anyhow!(
                "no such subcommand `{cmd}`{suggested_command}{suggested_script}"
            )
            .into());
        }
    }

    let manifest_path = root_manifest(Some(manifest_path), gctx)?;

    // Reload to cargo home.
    gctx.reload_rooted_at(gctx.home().clone().into_path_unlocked())?;

    let mut ws = Workspace::new(&manifest_path, gctx)?;
    if gctx.cli_unstable().avoid_dev_deps {
        ws.set_require_optional_deps(false);
    }

    let mut compile_opts =
        cargo::ops::CompileOptions::new(gctx, cargo::core::compiler::UserIntent::Build)?;
    compile_opts.spec = cargo::ops::Packages::Default;

    cargo::ops::run(&ws, &compile_opts, args).map_err(|err| to_run_error(gctx, err))
}

fn suggested_script(cmd: &str) -> Option<String> {
    let cmd_path = Path::new(cmd);
    let mut suggestion = Path::new(".").to_owned();
    for cmd_part in cmd_path.components() {
        let exact_match = suggestion.join(cmd_part);
        suggestion = if exact_match.exists() {
            exact_match
        } else {
            let possible: Vec<_> = std::fs::read_dir(suggestion)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.to_str().is_some())
                .collect();
            if let Some(possible) = closest(
                cmd_part.as_os_str().to_str().unwrap(),
                possible.iter(),
                |p| p.file_name().unwrap().to_str().unwrap(),
            ) {
                possible.to_owned()
            } else {
                return None;
            }
        };
    }
    if suggestion.is_dir() {
        None
    } else {
        suggestion.into_os_string().into_string().ok()
    }
}

fn to_run_error(gctx: &GlobalContext, err: anyhow::Error) -> CliError {
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
    let is_quiet = gctx.shell().verbosity() == Verbosity::Quiet;
    if is_quiet {
        CliError::code(exit_code)
    } else {
        CliError::new(err, exit_code)
    }
}
